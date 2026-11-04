//! Orchestrator for CareerOS — Full Guardrailed `/apply` Workflow.
//!
//! Sequential execution pipeline:
//!   0. Create application_records row (PENDING_REVIEW)
//!   1. Scrape JD URL to Markdown
//!   2. Gap analysis against Claims Ledger
//!   3. Guardrailed skill execution (resume-tailor, cover-letter)
//!   4. Fact-Check Pass
//!   5. Typst PDF compilation + visual QA
//!   6. Update application_records, sync CSV, Git commit

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::State;
use rfd::FileDialog;
use crate::AppState;
use crate::db::get_db_connection;
use crate::claims::CreateClaimRequest;
use crate::resume_parser::{extract_resume_text, ResumeFormat};
use crate::setup_wizard::{parse_claims_from_resume, update_setup_step};
use uuid::Uuid;
use chrono::Utc;

/// Input for the /apply workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyInput {
    pub jd_url: String,
    pub employer: Option<String>,
    pub role: Option<String>,
    pub req_id: Option<String>,
}

/// Result of the full /apply workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyResult {
    pub application_id: String,
    pub jd_scraped: bool,
    pub jd_path: Option<String>,
    pub gap_analysis: Option<crate::gap_analyzer::AnalysisResult>,
    pub resume_tailored: Option<crate::skills::SkillExecutionResult>,
    pub cover_letter: Option<crate::skills::SkillExecutionResult>,
    pub fact_check_report: Option<crate::fact_check::FactCheckReport>,
    pub pdf_compiled: Option<crate::typst_engine::CompileResult>,
    pub pages_rasterized: Option<crate::typst_engine::RasterizeResult>,
    pub visual_flags: Option<crate::typst_engine::VisualFlags>,
    pub git_commit: Option<String>,
    pub success: bool,
    pub error_message: Option<String>,
}

/// Execute the full guardrailed /apply workflow.
#[tauri::command]
pub async fn execute_apply_workflow(
    input: ApplyInput,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<ApplyResult, String> {
    let mut result = ApplyResult {
        application_id: String::new(),
        jd_scraped: false,
        jd_path: None,
        gap_analysis: None,
        resume_tailored: None,
        cover_letter: None,
        fact_check_report: None,
        pdf_compiled: None,
        pages_rasterized: None,
        visual_flags: None,
        git_commit: None,
        success: false,
        error_message: None,
    };

    // 0. Get DB path
    let db_path_str = {
        let db_path_opt = state.db_path.lock().unwrap();
        db_path_opt.as_ref()
            .ok_or("Database not initialized")?
            .clone()
    };
    let conn = get_db_connection(&db_path_str)?;

    // Create application_records row with PENDING_REVIEW status
    let docs_dir = dirs::document_dir().ok_or("Failed to locate Documents folder")?;
    let workspace_root = docs_dir.join("CareerOS");
    let app_id = Uuid::new_v4().to_string();

    let now = Utc::now().format("%Y-%m-%d").to_string();
    let employer = input.employer.clone().unwrap_or_else(|| "Unknown".to_string());
    let role = input.role.clone().unwrap_or_else(|| "Unknown".to_string());

    conn.execute(
        "INSERT INTO application_records (id, employer, role, req_id, date, base_resume_version, materials_version, claims_used, approved_wording, gaps_named, salary_entered, status, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'PENDING_REVIEW', ?12)",
        [
            app_id.as_str(),
            employer.as_str(),
            role.as_str(),
            input.req_id.as_deref().unwrap_or(""),
            now.as_str(),
            "master_v1",
            "master_v1",
            "",
            "",
            "",
            "",
            format!("JD URL: {}", input.jd_url).as_str(),
        ],
    ).map_err(|e| format!("Failed to create application record: {}", e))?;

    result.application_id = app_id.clone();

    // 1. Scrape JD URL to Markdown
    let scrape_result = crate::scraper::scrape_job_page(input.jd_url.clone()).await;
    match &scrape_result {
        Ok(scrape) => {
            result.jd_scraped = true;
            let app_dir = workspace_root.join("applications").join(&app_id);
            fs::create_dir_all(&app_dir).map_err(|e| format!("Failed to create app dir: {}", e))?;
            let jd_path = app_dir.join("job-description.md");
            fs::write(&jd_path, &scrape.description_md).map_err(|e| format!("Failed to write JD file: {}", e))?;
            result.jd_path = Some(jd_path.to_string_lossy().into_owned());

            // Update application record with scraped info
            conn.execute(
                "UPDATE application_records SET notes = ?1 WHERE id = ?2",
                [format!("JD URL: {}\nJD scraped: {}", input.jd_url, now).as_str(), app_id.as_str()],
            ).map_err(|e| format!("Failed to update application record: {}", e))?;
        }
        Err(e) => {
            result.error_message = Some(format!("JD scrape failed: {}", e));
            return Ok(result);
        }
    }

    // 2. Execute gap analysis
    let gap_result = crate::gap_analyzer::analyze_job_gaps(
        scrape_result.as_ref().unwrap().description_md.clone(),
        app_id.clone(),
        state.clone(),
    ).await;

    match gap_result {
        Ok(gaps) => {
            result.gap_analysis = Some(gaps.clone());
        }
        Err(e) => {
            result.error_message = Some(format!("Gap analysis failed: {}", e));
            return Ok(result);
        }
    }

    // 3. Execute guardrailed skills (resume-tailor and cover-letter)
    // Use the JD text for context
    let jd_text = scrape_result.as_ref().unwrap().description_md.clone();

    let resume_result = crate::skills::execute_skill(
        crate::skills::SkillExecutionInput {
            skill_id: "resume-tailor".to_string(),
            application_id: Some(app_id.clone()),
            jd_text: Some(jd_text.clone()),
            use_deep_model: true,
        },
        state.clone(),
    ).await;

    match resume_result {
        Ok(r) => result.resume_tailored = Some(r),
        Err(e) => {
            result.error_message = Some(format!("Resume tailor failed: {}", e));
            return Ok(result);
        }
    }

    let cover_result = crate::skills::execute_skill(
        crate::skills::SkillExecutionInput {
            skill_id: "cover-letter".to_string(),
            application_id: Some(app_id.clone()),
            jd_text: Some(jd_text.clone()),
            use_deep_model: true,
        },
        state.clone(),
    ).await;

    match cover_result {
        Ok(r) => result.cover_letter = Some(r),
        Err(e) => {
            result.error_message = Some(format!("Cover letter failed: {}", e));
            return Ok(result);
        }
    }

    // 4. Run Fact-Check Pass on the cover letter
    let cover_md = result.cover_letter.as_ref()
        .map(|r| r.markdown_output.clone())
        .unwrap_or_default();

    if !cover_md.is_empty() {
        // Get confirmed claim IDs from gap analysis
        let confirmed_claim_ids: Vec<String> = result.gap_analysis.as_ref()
            .map(|g| g.supported_claim_ids.clone())
            .unwrap_or_default();

        let fact_result = crate::fact_check::run_fact_check_pass(
            cover_md.clone(),
            confirmed_claim_ids,
            vec![], // never_claim_subjects - populated from rules in fact_check internally
            Some(employer.clone()),
            Some(role.clone()),
            state.clone(),
        );

        match fact_result {
            Ok(fr) => result.fact_check_report = Some(fr),
            Err(e) => {
                result.error_message = Some(format!("Fact check failed: {}", e));
                return Ok(result);
            }
        }
    }

    // 5. Compile PDF via Typst (using cover letter markdown as .typ content)
    if let Some(cover_result) = &result.cover_letter {
        let app_dir = workspace_root.join("applications").join(&app_id);
        let typ_path = app_dir.join("cover-letter.typ");
        fs::write(
            &typ_path,
            crate::typst_engine::with_document_font_defaults(&cover_result.markdown_output),
        ).map_err(|e| format!("Failed to write Typst file: {}", e))?;

        let pdf_result = crate::typst_engine::compile_typst_pdf(
            typ_path.to_string_lossy().into_owned(),
            app.clone(),
        );
        match pdf_result {
            Ok(pr) => {
                result.pdf_compiled = Some(pr.clone());
                if pr.success {
                    // 6. Rasterize pages
                    let raster_result = crate::typst_engine::rasterize_pdf_pages(
                        pr.pdf_path.clone(),
                        app_dir.to_string_lossy().into_owned(),
                        app.clone(),
                    );
                    match raster_result {
                        Ok(rr) => {
                            result.pages_rasterized = Some(rr.clone());

                            // 7. Analyze visual QA
                            let vqa_result = crate::typst_engine::analyze_visual_qa(
                                pr.pdf_path,
                                rr.png_paths,
                                Some(employer.clone()),
                                Some(role.clone()),
                                app.clone(),
                            );
                            match vqa_result {
                                Ok(vf) => result.visual_flags = Some(vf),
                                Err(e) => {
                                    result.error_message = Some(format!("Visual QA failed: {}", e));
                                    return Ok(result);
                                }
                            }
                        }
                        Err(e) => {
                            result.error_message = Some(format!("Rasterize failed: {}", e));
                            return Ok(result);
                        }
                    }
                } else {
                    result.error_message = Some(format!("Typst compile failed: {:?}", pr.error_message));
                    return Ok(result);
                }
            }
            Err(e) => {
                result.error_message = Some(format!("Typst compile error: {}", e));
                return Ok(result);
            }
        }
    }

    // 8. Update application_records with results
    let claims_used = result.gap_analysis.as_ref()
        .map(|g| g.supported_claim_ids.join(","))
        .unwrap_or_default();
    let gaps_named = result.gap_analysis.as_ref()
        .map(|g| g.identified_gaps.len().to_string())
        .unwrap_or_else(|| "0".to_string());
    let approved_wording = result.cover_letter.as_ref()
        .map(|r| r.markdown_output.clone())
        .unwrap_or_default();

    conn.execute(
        "UPDATE application_records SET claims_used = ?1, gaps_named = ?2, approved_wording = ?3, materials_version = ?4 WHERE id = ?5",
        [claims_used.as_str(), gaps_named.as_str(), approved_wording.as_str(), "master_v1", app_id.as_str()],
    ).map_err(|e| format!("Failed to update application record: {}", e))?;

    // 9. Sync CSV mirror
    let csv_path = workspace_root.join("applications.csv");
    let csv_entry = format!("{},{},{},{},{},{},{}\n",
        app_id, now, employer, role, result.jd_scraped, input.jd_url, app_dir_to_str(&workspace_root, &app_id));

    if !csv_path.exists() {
        fs::write(&csv_path, "id,date,employer,role,jd_scraped,job_url,folder_path\n")
            .map_err(|e| format!("Failed to create CSV: {}", e))?;
    }
    {
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&csv_path)
            .map_err(|e| format!("Failed to open CSV for append: {}", e))?;
        std::io::Write::write_all(&mut file, csv_entry.as_bytes())
            .map_err(|e| format!("Failed to append CSV: {}", e))?;
    }

    // 10. Trigger Git commit
    let commit_result = crate::git_engine::commit_changes(
        state.clone(),
        format!("Apply: {} - {} ({})", employer, role, now),
    );
    match commit_result {
        Ok(oid) => result.git_commit = Some(oid),
        Err(e) => {
            result.error_message = Some(format!("Git commit failed: {}", e));
            return Ok(result);
        }
    }

    result.success = true;
    Ok(result)
}

/// Helper to build application directory path string.
fn app_dir_to_str(workspace_root: &PathBuf, app_id: &str) -> String {
    workspace_root.join("applications").join(app_id).to_string_lossy().into_owned()
}

/// Run company research (/prep command).
#[tauri::command]
pub async fn run_company_research(company: String, state: State<'_, AppState>) -> Result<String, String> {
    let ollama_config = crate::ollama::get_ollama_config(state);
    let config = ollama_config.unwrap_or_default();
    let model = config.deep_model.clone().or_else(|| config.fast_model.clone()).unwrap_or_else(|| "llama3.2:latest".to_string());

    let prompt = format!(
        "Research the company {} and provide a briefing for a job application. Include: company overview, recent news, products/services, culture, interview tips, and key talking points for a cover letter.",
        company
    );

    let messages = vec![
        crate::ollama::OllamaChatMessage {
            role: "system".to_string(),
            content: "You are a company researcher helping a job applicant prepare applications and interviews.".to_string(),
        },
        crate::ollama::OllamaChatMessage {
            role: "user".to_string(),
            content: prompt,
        },
    ];

    let response = crate::ollama::chat_with_ollama(
        model,
        messages,
        Some(0.3),
        Some(1000),
        None,
    ).map_err(|e| format!("Company research failed: {}", e))?;

    Ok(response.message.content)
}

/// Run interview simulator (/mock command).
#[tauri::command]
pub async fn start_interview_simulator(role: String, state: State<'_, AppState>) -> Result<String, String> {
    let ollama_config = crate::ollama::get_ollama_config(state);
    let config = ollama_config.unwrap_or_default();
    let model = config.deep_model.clone().or_else(|| config.fast_model.clone()).unwrap_or_else(|| "llama3.2:latest".to_string());

    let prompt = format!(
        "You are a hiring manager interviewing a candidate for a {} role. Conduct a STAR-method behavioral interview. Ask one question at a time, wait for the user's response, then provide feedback and ask the next question. Start with an introduction and the first question.",
        role
    );

    let messages = vec![
        crate::ollama::OllamaChatMessage {
            role: "system".to_string(),
            content: "You are an experienced hiring manager conducting behavioral interviews using the STAR method.".to_string(),
        },
        crate::ollama::OllamaChatMessage {
            role: "user".to_string(),
            content: prompt,
        },
    ];

    let response = crate::ollama::chat_with_ollama(
        model,
        messages,
        Some(0.5),
        Some(800),
        None,
    ).map_err(|e| format!("Interview simulator failed: {}", e))?;

    Ok(response.message.content)
}

/// Draft follow-up emails for stale applications (/followup command).
#[tauri::command]
pub async fn draft_followups(state: State<'_, AppState>) -> Result<String, String> {
    let db_path_str = {
        let db_path_opt = state.db_path.lock().unwrap();
        db_path_opt.as_ref()
            .ok_or("Database not initialized")?
            .clone()
    };
    let conn = get_db_connection(&db_path_str)?;

    // Get applications that are SUBMITTED or INTERVIEWING without recent follow-up
    let mut stmt = conn.prepare("SELECT id, employer, role, date FROM application_records WHERE status IN ('SUBMITTED', 'INTERVIEWING')")
        .map_err(|e| format!("Failed to prepare query: {}", e))?;
    let apps: Vec<(String, String, String, String)> = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    }).map_err(|e| format!("Failed to query apps: {}", e))?
    .filter_map(|r| r.ok())
    .collect();

    if apps.is_empty() {
        return Ok("No pending applications requiring follow-up.".to_string());
    }

    let ollama_config = crate::ollama::get_ollama_config(state);
    let config = ollama_config.unwrap_or_default();
    let _model = config.fast_model.clone().or_else(|| config.deep_model.clone()).unwrap_or_else(|| "llama3.2:latest".to_string());

    let mut followups = String::from("## Follow-up Drafts\n\n");
    for (id, employer, role, date) in apps {
        followups.push_str(&format!("### {}\n- Employer: {}\n- Role: {}\n- Applied: {}\n\n", id, employer, role, date));
    }

    Ok(followups)
}

/// Generate weekly report (/weekly command).
#[tauri::command]
pub async fn generate_weekly_report(state: State<'_, AppState>) -> Result<String, String> {
    let db_path_str = {
        let db_path_opt = state.db_path.lock().unwrap();
        db_path_opt.as_ref()
            .ok_or("Database not initialized")?
            .clone()
    };
    let conn = get_db_connection(&db_path_str)?;

    let total: i64 = conn.query_row(
        "SELECT COUNT(*) FROM application_records",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    let submitted: i64 = conn.query_row(
        "SELECT COUNT(*) FROM application_records WHERE status = 'SUBMITTED'",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    let interviewing: i64 = conn.query_row(
        "SELECT COUNT(*) FROM application_records WHERE status = 'INTERVIEWING'",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    let rejected: i64 = conn.query_row(
        "SELECT COUNT(*) FROM application_records WHERE status = 'REJECTED'",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    let offer: i64 = conn.query_row(
        "SELECT COUNT(*) FROM application_records WHERE status = 'OFFER'",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    Ok(format!(
        "## Weekly Application Report\n\n- Total applications: {}\n- Submitted: {}\n- Interviewing: {}\n- Rejected: {}\n- Offers: {}\n",
        total, submitted, interviewing, rejected, offer
    ))
}

/// Log analysis — query daily-log.md and ledger for submission counts.
#[tauri::command]
pub async fn analyze_logs(state: State<'_, AppState>) -> Result<String, String> {
    let docs_dir = dirs::document_dir().ok_or("Failed to locate Documents folder")?;
    let workspace_root = docs_dir.join("CareerOS");
    let log_path = workspace_root.join("daily-log.md");

    let log_content = if log_path.exists() {
        fs::read_to_string(&log_path).map_err(|e| format!("Failed to read daily log: {}", e))?
    } else {
        "No daily log found.".to_string()
    };

    let db_path_str = {
        let db_path_opt = state.db_path.lock().unwrap();
        db_path_opt.as_ref()
            .ok_or("Database not initialized")?
            .clone()
    };
    let conn = get_db_connection(&db_path_str)?;

    let today_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM application_records WHERE date = ?1",
        [Utc::now().format("%Y-%m-%d").to_string().as_str()],
        |row| row.get(0),
    ).unwrap_or(0);

    let week_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM application_records WHERE date >= date('now', '-7 days')",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    Ok(format!(
        "## Log Analysis\n\n### daily-log.md\n{}\n\n### Application Counts\n- Today: {}\n- Past 7 days: {}\n",
        log_content, today_count, week_count
    ))
}

/// Get all application records for the ledger dashboard.
#[tauri::command]
pub fn get_application_records(state: State<'_, AppState>) -> Result<Vec<ApplicationRecord>, String> {
    let db_path_str = {
        let db_path_opt = state.db_path.lock().unwrap();
        db_path_opt.as_ref()
            .ok_or("Database not initialized")?
            .clone()
    };
    let conn = get_db_connection(&db_path_str)?;

    let mut stmt = conn.prepare("SELECT id, employer, role, req_id, date, base_resume_version, materials_version, claims_used, gaps_named, approved_wording, salary_entered, status, notes FROM application_records ORDER BY date DESC")
        .map_err(|e| format!("Failed to prepare query: {}", e))?;
    let records: Vec<ApplicationRecord> = stmt.query_map([], |row| {
        Ok(ApplicationRecord {
            id: row.get(0)?,
            employer: row.get(1)?,
            role: row.get(2)?,
            req_id: row.get(3)?,
            date: row.get(4)?,
            base_resume_version: row.get(5)?,
            materials_version: row.get(6)?,
            claims_used: row.get(7)?,
            gaps_named: row.get(8)?,
            approved_wording: row.get(9)?,
            salary_entered: row.get(10)?,
            status: row.get(11)?,
            notes: row.get(12)?,
        })
    }).map_err(|e| format!("Failed to query records: {}", e))?
    .filter_map(|r| r.ok())
    .collect();

    Ok(records)
}

/// Application record for the ledger dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationRecord {
    pub id: String,
    pub employer: String,
    pub role: String,
    pub req_id: String,
    pub date: String,
    pub base_resume_version: String,
    pub materials_version: String,
    pub claims_used: String,
    pub gaps_named: String,
    pub approved_wording: String,
    pub salary_entered: Option<String>,
    pub status: String,
    pub notes: String,
}

/// Update application record status.
#[tauri::command]
pub fn update_application_status(
    id: String,
    status: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let db_path_str = {
        let db_path_opt = state.db_path.lock().unwrap();
        db_path_opt.as_ref()
            .ok_or("Database not initialized")?
            .clone()
    };
    let conn = get_db_connection(&db_path_str)?;

    conn.execute(
        "UPDATE application_records SET status = ?1 WHERE id = ?2",
        [status.as_str(), id.as_str()],
    ).map_err(|e| format!("Failed to update status: {}", e))?;

    Ok(true)
}

/// Open a resume file via dialog or path, extract claims, and save to the ledger.
#[tauri::command]
pub async fn open_resume_file(
    file_path: Option<String>,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<ResumeImportResult, String> {
    let (format, content) = match file_path {
        Some(path) => {
            extract_resume_text(&path, &app)
                .map_err(|e| format!("Failed to read resume file: {}", e))?
        }
        None => {
            // Open file dialog using rfd
            let selected = FileDialog::new()
                .add_filter("Resume files", &["pdf", "docx", "odt", "txt", "md"])
                .pick_file();
            match selected {
                Some(path_buf) => {
                    let path_str = path_buf.to_string_lossy().to_string();
                    extract_resume_text(&path_str, &app)
                        .map_err(|e| format!("Failed to read resume file: {}", e))?
                }
                None => {
                    return Ok(ResumeImportResult {
                        success: false,
                        claims_imported: 0,
                        format: None,
                        error_message: Some("No file selected".to_string()),
                    });
                }
            }
        }
    };

    // Save resume content to app_settings and master resume.md
    let db_path_str = {
        let db_path_opt = state.db_path.lock().unwrap();
        db_path_opt.as_ref()
            .ok_or("Database not initialized")?
            .clone()
    };
    let conn = get_db_connection(&db_path_str)?;

    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('master_resume', ?1)",
        [content.clone()],
    ).map_err(|e| format!("Failed to save resume: {}", e))?;

    let workspace_root = PathBuf::from(&db_path_str).parent().unwrap().to_path_buf();
    let master_resume_path = workspace_root.join("master").join("resume.md");
    fs::write(&master_resume_path, &content).map_err(|e| format!("Failed to write master resume: {}", e))?;

    // Extract claims from resume text
    let suggestions = parse_claims_from_resume(&content);

    // Convert suggestions to CreateClaimRequest and save
    let mut count = 0;
    for suggestion in &suggestions {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        let inserted = conn.execute(
            "INSERT INTO claims (id, statement, evidence_source, confirmed_wording, scope, category, status, last_confirmed_at)
             SELECT ?1, ?2, ?3, ?4, ?5, ?6, 'confirmed', ?7
             WHERE NOT EXISTS (
                 SELECT 1 FROM claims
                 WHERE status = 'confirmed'
                   AND statement = ?2
                   AND evidence_source = ?3
                   AND confirmed_wording = ?4
                   AND scope = ?5
                   AND category = ?6
             )",
            [
                id.as_str(),
                suggestion.statement.as_str(),
                suggestion.evidence_source.as_str(),
                suggestion.confirmed_wording.as_str(),
                suggestion.scope.as_str(),
                suggestion.category.as_str(),
                now.as_str(),
            ],
        ).map_err(|e| format!("Failed to save claim: {}", e))?;
        if inserted > 0 {
            count += 1;
        }
    }

    crate::setup_wizard::update_setup_step(&conn, "resume")?;
    crate::setup_wizard::update_setup_step(&conn, "claims")?;

    Ok(ResumeImportResult {
        success: true,
        claims_imported: count,
        error_message: None,
        format: Some(format),
    })
}

/// Result of resume import operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeImportResult {
    pub success: bool,
    pub claims_imported: u32,
    pub format: Option<ResumeFormat>,
    pub error_message: Option<String>,
}

/// Export an ATS-optimized resume to PDF, DOCX, or ODT format.
#[tauri::command]
pub async fn export_ats_resume(
    markdown: String,
    format: String,
    output_dir: Option<String>,
    app: tauri::AppHandle,
) -> Result<crate::resume_exporter::ExportResult, String> {
    let export_format = match format.to_lowercase().as_str() {
        "pdf" => crate::resume_exporter::ExportFormat::Pdf,
        "docx" => crate::resume_exporter::ExportFormat::Docx,
        "odt" => crate::resume_exporter::ExportFormat::Odt,
        _ => {
            return Ok(crate::resume_exporter::ExportResult {
                success: false,
                output_path: None,
                format: None,
                error_message: Some(format!("Unsupported format: {}. Use pdf, docx, or odt.", format)),
            });
        }
    };

    let out_dir = output_dir.unwrap_or_else(|| {
        std::env::temp_dir().to_string_lossy().into_owned()
    });

    crate::resume_exporter::export_ats_resume(&markdown, export_format, &out_dir, &app)
}

/// Contact information for resume header.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactInfoInput {
    pub name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub linkedin: Option<String>,
}

/// Generate ATS-optimized resume markdown from claims and JD text.
#[tauri::command]
pub async fn generate_ats_resume_markdown(
    claims: Vec<crate::claims::Claim>,
    jd_text: Option<String>,
    contact_info: Option<ContactInfoInput>,
) -> Result<String, String> {
    let contact = contact_info.map(|c| crate::ats_engine::ContactInfo {
        name: c.name,
        phone: c.phone,
        email: c.email,
        linkedin: c.linkedin,
    }).unwrap_or_default();

    Ok(crate::ats_engine::ATSRules::generate_ats_resume(
        &claims,
        jd_text.as_deref(),
        Some(&contact),
    ))
}

/// Run ATS compliance check on resume markdown.
#[tauri::command]
pub async fn check_ats_compliance(
    markdown: String,
    jd_text: Option<String>,
) -> Result<crate::ats_engine::ATSReport, String> {
    Ok(crate::ats_engine::ATSRules::analyze(
        &markdown,
        jd_text.as_deref(),
    ))
}