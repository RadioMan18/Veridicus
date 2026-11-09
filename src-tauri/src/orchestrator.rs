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

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{Emitter, State};
use std::sync::atomic::{AtomicBool, Ordering};
use rfd::FileDialog;
use crate::AppState;
use crate::db::get_db_connection;
use crate::claims::CreateClaimRequest;
use crate::resume_parser::{extract_resume_text, ResumeFormat};
use crate::setup_wizard::parse_claims_from_resume;
use uuid::Uuid;
use chrono::Utc;

/// Progress event emitted while the /apply pipeline runs, so the UI can show a timeline.
#[derive(Debug, Clone, Serialize)]
pub struct ApplyProgress {
    pub stage: String,
    pub label: String,
    pub status: String,
    pub detail: Option<String>,
}

/// Set by `cancel_apply`; checked between pipeline stages.
static APPLY_CANCELLED: AtomicBool = AtomicBool::new(false);

fn emit_stage(
    app: &tauri::AppHandle,
    stage: &str,
    label: &str,
    status: &str,
    detail: Option<String>,
) {
    let _ = app.emit(
        "apply-progress",
        ApplyProgress {
            stage: stage.to_string(),
            label: label.to_string(),
            status: status.to_string(),
            detail,
        },
    );
}

/// Marks the current stage failed when the user cancelled, so the caller can return early.
fn cancelled(result: &mut ApplyResult, app: &tauri::AppHandle, stage: &str, label: &str) -> bool {
    if APPLY_CANCELLED.load(Ordering::SeqCst) {
        emit_stage(app, stage, label, "failed", Some("Cancelled by user".to_string()));
        result.error_message = Some("Cancelled by user".to_string());
        return true;
    }
    false
}

#[tauri::command]
pub fn cancel_apply() -> bool {
    APPLY_CANCELLED.store(true, Ordering::SeqCst);
    true
}

/// Commit an application only after the user approves its materials in the review gate.
/// Blocking guardrail findings stop the commit unless the user records an override.
#[tauri::command]
pub fn commit_application(
    application_id: String,
    override_reason: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let db_path_str = {
        let db_path_opt = state.db_path.lock().unwrap();
        db_path_opt
            .as_ref()
            .ok_or("Database not initialized")?
            .clone()
    };
    let conn = get_db_connection(&db_path_str)?;
    let (employer, role, date): (String, String, String) = conn
        .query_row(
            "SELECT employer, role, date FROM application_records WHERE id = ?1",
            [application_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|e| format!("Application not found: {}", e))?;

    let docs_dir = dirs::document_dir().ok_or("Failed to locate Documents folder")?;
    let app_dir = docs_dir
        .join("CareerOS")
        .join("applications")
        .join(&application_id);
    let resume_md = fs::read_to_string(app_dir.join("resume.md")).unwrap_or_default();
    let letter_md = fs::read_to_string(app_dir.join("cover-letter.md")).unwrap_or_default();

    if resume_md.trim().is_empty() && letter_md.trim().is_empty() {
        return Err(
            "No generated materials found for this application. Run /apply first.".to_string(),
        );
    }

    if !crate::compliance::has_output_disclosure(&app_dir) {
        return Err(
            "Acknowledge the AI-assisted disclosure for this application before committing."
                .to_string(),
        );
    }

    let claims = crate::guardrails::fetch_confirmed_claims(&conn)?;
    let rules_resume = crate::guardrails::load_active_rules(&conn, crate::guardrails::RESUME)?;
    let rules_letter = crate::guardrails::load_active_rules(&conn, crate::guardrails::COVER_LETTER)?;
    let gaps = crate::guardrails::required_gaps_for(&conn, &application_id)?;

    let mut reports = Vec::new();
    if !resume_md.trim().is_empty() {
        reports.push(crate::guardrails::evaluate_document(
            crate::guardrails::RESUME,
            &resume_md,
            &claims,
            &rules_resume,
            Some(employer.as_str()),
            Some(role.as_str()),
            &[],
        ));
    }
    if !letter_md.trim().is_empty() {
        reports.push(crate::guardrails::evaluate_document(
            crate::guardrails::COVER_LETTER,
            &letter_md,
            &claims,
            &rules_letter,
            Some(employer.as_str()),
            Some(role.as_str()),
            &gaps,
        ));
    }
    let mut report = crate::guardrails::merge(reports);
    let mut cited = Vec::new();
    if !resume_md.trim().is_empty() {
        cited.extend(crate::guardrails::cited_claim_ids(&resume_md, &claims));
    }
    if !letter_md.trim().is_empty() {
        cited.extend(crate::guardrails::cited_claim_ids(&letter_md, &claims));
    }
    cited.sort();
    cited.dedup();
    report
        .findings
        .extend(crate::guardrails::weak_provenance_findings(&claims, &cited));
    report.blocking = report.findings.iter().any(|finding| finding.blocking);
    report.passed = !report.blocking;

    fs::write(
        app_dir.join("guardrail-report-commit.json"),
        serde_json::to_string_pretty(&report)
            .map_err(|error| format!("Failed to serialize guardrail report: {}", error))?,
    )
    .map_err(|error| format!("Failed to write guardrail report: {}", error))?;

    let mut override_used = false;
    if report.blocking {
        let summary = report
            .findings
            .iter()
            .filter(|finding| finding.blocking)
            .map(|finding| format!("{}: {}", finding.code, finding.message))
            .collect::<Vec<_>>()
            .join("\n- ");

        match override_reason
            .as_deref()
            .map(str::trim)
            .filter(|reason| !reason.is_empty())
        {
            None => {
                return Err(format!(
                    "Blocked by honesty guardrails:\n- {}\n\nFix the documents, or commit with a written override.",
                    summary
                ));
            }
            Some(reason) => {
                override_used = true;
                let record = format!(
                    "# Guardrail override\n\nDate: {}\nReason: {}\n\n## Findings\n- {}\n",
                    Utc::now().to_rfc3339(),
                    reason,
                    summary
                );
                fs::write(app_dir.join("guardrail-override.md"), record)
                    .map_err(|error| format!("Failed to write override record: {}", error))?;

                let note = format!(
                    "\nGuardrail override ({}): {} — {}",
                    Utc::now().format("%Y-%m-%d"),
                    reason,
                    summary.replace('\n', " ")
                );
                conn.execute(
                    "UPDATE application_records SET notes = notes || ?1 WHERE id = ?2",
                    [note.as_str(), application_id.as_str()],
                )
                .map_err(|error| format!("Failed to record override: {}", error))?;
            }
        }
    }

    let last_generation = crate::compliance::last_generation(&app_dir);
    let model = last_generation
        .as_ref()
        .and_then(|event| event.model.clone());
    let model_digest = last_generation
        .as_ref()
        .and_then(|event| event.model_digest.clone());
    let model_digest_at_commit = model
        .as_deref()
        .and_then(|name| crate::ollama::model_identity(name).digest);
    let mut all_rules = rules_resume.clone();
    all_rules.extend(rules_letter.iter().cloned());
    let (rule_ids, rules_sha256) = crate::compliance::rules_fingerprint(&all_rules);

    let mut audit = crate::compliance::new_event(
        &application_id,
        if override_used { "override" } else { "approved" },
    );
    audit.model = model;
    audit.model_digest = model_digest;
    audit.model_digest_at_commit = model_digest_at_commit;
    audit.template_version = crate::skills::TEMPLATE_VERSION;
    audit.rule_ids = rule_ids;
    audit.rules_sha256 = rules_sha256;
    audit.documents = crate::compliance::document_digests(&app_dir);
    audit.blocking = report.blocking;
    audit.findings = report.findings.clone();
    audit.advisory_count = report
        .findings
        .iter()
        .filter(|finding| !finding.blocking)
        .count();
    audit.override_reason = if override_used {
        override_reason.clone()
    } else {
        None
    };
    crate::compliance::append_event(&app_dir, &audit)
        .map_err(|error| format!("Could not write the audit record; commit blocked: {}", error))?;

    crate::git_engine::commit_changes(
        state.clone(),
        format!("Apply: {} - {} ({})", employer, role, date),
    )
}

/// Save text through a native save dialog. Done in Rust so the webview needs no
/// dialog or filesystem plugin permissions.
#[tauri::command]
pub fn save_text_via_dialog(default_name: String, contents: String) -> Result<Option<String>, String> {
    let selected = FileDialog::new().set_file_name(&default_name).save_file();
    match selected {
        Some(path) => {
            fs::write(&path, contents).map_err(|e| format!("Failed to write file: {}", e))?;
            Ok(Some(path.to_string_lossy().into_owned()))
        }
        None => Ok(None),
    }
}

/// Absolute folder holding one application's artifacts.
#[tauri::command]
pub fn get_application_folder(application_id: String) -> Result<String, String> {
    let docs_dir = dirs::document_dir().ok_or("Failed to locate Documents folder")?;
    Ok(docs_dir
        .join("CareerOS")
        .join("applications")
        .join(application_id)
        .to_string_lossy()
        .into_owned())
}

/// Reveal an application folder in the OS file manager.
#[tauri::command]
pub fn open_application_folder(path: String) -> Result<(), String> {
    if !std::path::Path::new(&path).exists() {
        return Err(format!("Folder not found: {}", path));
    }

    #[cfg(target_os = "windows")]
    let mut command = std::process::Command::new("explorer");
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = std::process::Command::new("xdg-open");

    command
        .arg(&path)
        .spawn()
        .map_err(|e| format!("Could not open folder: {}", e))?;
    Ok(())
}

/// Read a rasterized page as a data URL, so the review screen can show documents
/// without enabling the asset protocol or widening the CSP.
#[tauri::command]
pub fn get_artifact_preview(path: String) -> Result<String, String> {
    let metadata = fs::metadata(&path).map_err(|e| format!("Cannot read {}: {}", path, e))?;
    if metadata.len() > 6_000_000 {
        return Err("This artifact is too large to preview".to_string());
    }
    let bytes = fs::read(&path).map_err(|e| format!("Cannot read {}: {}", path, e))?;
    use base64::Engine;
    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

/// Input for the /apply workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyInput {
    pub jd_url: String,
    pub employer: Option<String>,
    pub role: Option<String>,
    pub req_id: Option<String>,
    /// Leave the git commit to the review gate instead of committing automatically.
    #[serde(default)]
    pub defer_commit: bool,
}

/// Result of the full /apply workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyResult {
    pub application_id: String,
    pub employer: String,
    pub role: String,
    pub jd_scraped: bool,
    pub jd_path: Option<String>,
    pub gap_analysis: Option<crate::gap_analyzer::AnalysisResult>,
    pub resume_tailored: Option<crate::skills::SkillExecutionResult>,
    pub cover_letter: Option<crate::skills::SkillExecutionResult>,
    pub fact_check_report: Option<crate::fact_check::FactCheckReport>,
    pub resume_fact_check_report: Option<crate::fact_check::FactCheckReport>,
    pub guardrail_report: Option<crate::guardrails::GuardrailReport>,
    pub recruiter_review: Option<crate::recruiter_review::RecruiterReview>,
    pub resume_pdf_compiled: Option<crate::typst_engine::CompileResult>,
    pub resume_visual_flags: Option<crate::typst_engine::VisualFlags>,
    pub resume_pages_rasterized: Option<crate::typst_engine::RasterizeResult>,
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
        employer: "Unknown".to_string(),
        role: "Unknown".to_string(),
        jd_scraped: false,
        jd_path: None,
        gap_analysis: None,
        resume_tailored: None,
        cover_letter: None,
        fact_check_report: None,
        resume_fact_check_report: None,
        guardrail_report: None,
        recruiter_review: None,
        resume_pdf_compiled: None,
        resume_visual_flags: None,
        resume_pages_rasterized: None,
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

    APPLY_CANCELLED.store(false, Ordering::SeqCst);
    emit_stage(&app, "scrape", "Scrape job posting", "active", None);

    // Create application_records row with PENDING_REVIEW status
    let docs_dir = dirs::document_dir().ok_or("Failed to locate Documents folder")?;
    let workspace_root = docs_dir.join("CareerOS");
    let app_id = Uuid::new_v4().to_string();

    let now = Utc::now().format("%Y-%m-%d").to_string();
    let mut employer = input.employer.clone().unwrap_or_else(|| "Unknown".to_string());
    let mut role = input.role.clone().unwrap_or_else(|| "Unknown".to_string());
    let app_dir = workspace_root.join("applications").join(&app_id);
    let mut notes = format!("JD URL: {}", input.jd_url);

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
            notes.as_str(),
        ],
    ).map_err(|e| format!("Failed to create application record: {}", e))?;

    result.application_id = app_id.clone();

    // 1. Scrape JD URL to Markdown
    let scrape_result = crate::scraper::scrape_job_page(input.jd_url.clone()).await;
    match &scrape_result {
        Ok(scrape) => {
            result.jd_scraped = true;
            fs::create_dir_all(&app_dir).map_err(|e| format!("Failed to create app dir: {}", e))?;
            let jd_path = app_dir.join("job-description.md");
            fs::write(&jd_path, &scrape.description_md).map_err(|e| format!("Failed to write JD file: {}", e))?;
            result.jd_path = Some(jd_path.to_string_lossy().into_owned());

            // Prefer metadata supplied on the command line; fall back to the scraped posting.
            employer = resolve_metadata(&input.employer, &scrape.company, "Unknown Company");
            role = resolve_metadata(&input.role, &scrape.title, "Job Posting");
            result.employer = employer.clone();
            result.role = role.clone();
            let scrape_detail = if scrape.requirements_md.trim().is_empty() {
                format!("{} — {} (no requirements section found)", employer, role)
            } else {
                format!("{} — {}", employer, role)
            };
            emit_stage(&app, "scrape", "Scrape job posting", "done", Some(scrape_detail));
            notes = format!("JD URL: {}\nJD scraped: {}\nEmployer: {}\nRole: {}", input.jd_url, now, employer, role);
            if !scrape.requirements_md.trim().is_empty() && !scrape.requirements_md.contains("not found") {
                fs::write(app_dir.join("job-requirements.md"), &scrape.requirements_md)
                    .map_err(|e| format!("Failed to write JD requirements file: {}", e))?;
            }

            conn.execute(
                "UPDATE application_records SET employer = ?1, role = ?2, notes = ?3 WHERE id = ?4",
                [employer.as_str(), role.as_str(), notes.as_str(), app_id.as_str()],
            ).map_err(|e| format!("Failed to update application record: {}", e))?;
        }
        Err(e) => {
            let message = format!("JD scrape failed: {}", e);
            emit_stage(&app, "scrape", "Scrape job posting", "failed", Some(message.clone()));
            result.error_message = Some(message);
            return Ok(result);
        }
    }

    // 2. Execute gap analysis
    if cancelled(&mut result, &app, "gap", "Gap analysis against claims ledger") {
        return Ok(result);
    }
    emit_stage(&app, "gap", "Gap analysis against claims ledger", "active", None);
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
            emit_stage(&app, "gap", "Gap analysis against claims ledger", "failed", Some(e.clone()));
            result.error_message = Some(format!("Gap analysis failed: {}", e));
            return Ok(result);
        }
    }

    let mut required_gaps: Vec<crate::guardrails::RequiredGap> = Vec::new();
    if let Some(gaps) = result.gap_analysis.as_ref() {
        match crate::guardrails::persist_gaps(&conn, &app_id, gaps) {
            Ok(persisted) => {
                // A gap the user accepted as unaddressed stops being required in
                // the letter, but `accepted_gap_requirements` reports it below.
                required_gaps = persisted
                    .into_iter()
                    .filter(|gap| {
                        (gap.requirement_tier == "must_have" || gap.requirement_tier == "gate")
                            && gap.status != "accepted_by_user"
                    })
                    .collect();
                // The matrix is what the review screen renders; store the exact
                // analysis that produced this application's documents.
                let _ = fs::write(
                    app_dir.join("fit-matrix.json"),
                    serde_json::to_string_pretty(gaps).unwrap_or_default(),
                );
            }
            Err(error) => {
                emit_stage(&app, "gap", "Gap analysis against claims ledger", "failed", Some(error.clone()));
                result.error_message = Some(format!("Gap persistence failed: {}", error));
                return Ok(result);
            }
        }
    }

    // Posting terms nothing in the ledger covers yet. The resume skill is asked to
    // cover them only where a claim already supports the statement.
    let coverage_terms: Vec<String> = result
        .gap_analysis
        .as_ref()
        .map(|gaps| {
            gaps.ats_keywords
                .iter()
                .filter(|entry| !entry.in_resume && !entry.in_claims)
                .map(|entry| entry.term.clone())
                .take(12)
                .collect()
        })
        .unwrap_or_default();

    // 3. Execute guardrailed skills (resume-tailor and cover-letter)
    // Pass the extracted requirements alongside the description so tailoring can target them.
    let scraped = scrape_result.as_ref().unwrap();
    let jd_text = if scraped.requirements_md.trim().is_empty() {
        scraped.description_md.clone()
    } else {
        format!(
            "{}\n\n## Requirements\n{}",
            scraped.description_md, scraped.requirements_md
        )
    };

    if cancelled(&mut result, &app, "resume", "Tailor resume") {
        return Ok(result);
    }
    emit_stage(&app, "resume", "Tailor resume", "active", None);

    let resume_result = crate::skills::execute_skill(
        crate::skills::SkillExecutionInput {
            skill_id: "resume-tailor".to_string(),
            application_id: None,
            jd_text: Some(jd_text.clone()),
            use_deep_model: true,
            required_gaps: Vec::new(),
            coverage_terms: coverage_terms.clone(),
            employer: Some(employer.clone()),
            role: Some(role.clone()),
        },
        state.clone(),
    ).await;

    match resume_result {
        Ok(r) => result.resume_tailored = Some(r),
        Err(e) => {
            emit_stage(&app, "resume", "Tailor resume", "failed", Some(e.clone()));
            result.error_message = Some(format!("Resume tailor failed: {}", e));
            return Ok(result);
        }
    }

    if cancelled(&mut result, &app, "letter", "Write cover letter") {
        return Ok(result);
    }
    emit_stage(&app, "resume", "Tailor resume", "done", None);
    emit_stage(&app, "letter", "Write cover letter", "active", None);

    let cover_result = crate::skills::execute_skill(
        crate::skills::SkillExecutionInput {
            skill_id: "cover-letter".to_string(),
            application_id: None,
            jd_text: Some(jd_text.clone()),
            use_deep_model: true,
            required_gaps: required_gaps.clone(),
            coverage_terms: Vec::new(),
            employer: Some(employer.clone()),
            role: Some(role.clone()),
        },
        state.clone(),
    ).await;

    match cover_result {
        Ok(r) => result.cover_letter = Some(r),
        Err(e) => {
            emit_stage(&app, "letter", "Write cover letter", "failed", Some(e.clone()));
            result.error_message = Some(format!("Cover letter failed: {}", e));
            return Ok(result);
        }
    }

    if cancelled(&mut result, &app, "fact-check", "Fact-check both documents") {
        return Ok(result);
    }
    emit_stage(&app, "letter", "Write cover letter", "done", None);
    emit_stage(&app, "fact-check", "Fact-check both documents", "active", None);

    // 4. Run the fact-check pass on both generated documents
    let confirmed_claim_ids: Vec<String> = result.gap_analysis.as_ref()
        .map(|g| g.supported_claim_ids.clone())
        .unwrap_or_default();

    // A local fn, not a closure: a closure would capture `conn`, and a `Connection`
    // is not `Sync`, which would make this workflow future non-`Send` across the
    // advisory model call below.
    fn never_claim_subjects(conn: &rusqlite::Connection, kind: &str) -> Vec<String> {
        crate::guardrails::load_active_rules(conn, kind)
            .map(|rules| {
                rules
                    .into_iter()
                    .filter(|rule| rule.kind == "never_claim")
                    .map(|rule| rule.subject)
                    .collect()
            })
            .unwrap_or_default()
    }
    let resume_never_claims = never_claim_subjects(&conn, crate::guardrails::RESUME);
    let letter_never_claims = never_claim_subjects(&conn, crate::guardrails::COVER_LETTER);

    if let Some(resume_md) = result.resume_tailored.as_ref().map(|r| r.markdown_output.clone()) {
        match crate::fact_check::run_fact_check_pass(
            resume_md,
            confirmed_claim_ids.clone(),
            resume_never_claims.clone(),
            Some(employer.clone()),
            Some(role.clone()),
            state.clone(),
        ) {
            Ok(report) => result.resume_fact_check_report = Some(report),
            Err(e) => {
                emit_stage(&app, "fact-check", "Fact-check both documents", "failed", Some(e.clone()));
                result.error_message = Some(format!("Resume fact check failed: {}", e));
                return Ok(result);
            }
        }
    }

    if let Some(cover_md) = result.cover_letter.as_ref().map(|r| r.markdown_output.clone()) {
        match crate::fact_check::run_fact_check_pass(
            cover_md,
            confirmed_claim_ids,
            letter_never_claims,
            Some(employer.clone()),
            Some(role.clone()),
            state.clone(),
        ) {
            Ok(report) => result.fact_check_report = Some(report),
            Err(e) => {
                emit_stage(&app, "fact-check", "Fact-check both documents", "failed", Some(e.clone()));
                result.error_message = Some(format!("Cover letter fact check failed: {}", e));
                return Ok(result);
            }
        }
    }

    emit_stage(
        &app,
        "fact-check",
        "Fact-check both documents",
        "active",
        Some("Advisory scope review".to_string()),
    );

    // 4b. Honesty guardrails: model proposes, the record decides.
    if let Some(letter) = result.cover_letter.as_ref() {
        for gap in required_gaps.iter() {
            if crate::guardrails::gap_named(&letter.markdown_output, &gap.requirement) {
                let _ = conn.execute(
                    "UPDATE gaps SET status = 'named_in_letter' WHERE id = ?1",
                    [gap.id.as_str()],
                );
            }
        }
    }

    let mut cited_claim_ids: Vec<String> = Vec::new();
    match crate::guardrails::fetch_confirmed_claims(&conn) {
        Ok(claims) => {
            let rules_resume = crate::guardrails::load_active_rules(&conn, crate::guardrails::RESUME).unwrap_or_default();
            let rules_letter = crate::guardrails::load_active_rules(&conn, crate::guardrails::COVER_LETTER).unwrap_or_default();
            let mut reports = Vec::new();
            if let Some(resume) = result.resume_tailored.as_ref() {
                reports.push(crate::guardrails::evaluate_document(
                    crate::guardrails::RESUME,
                    &resume.markdown_output,
                    &claims,
                    &rules_resume,
                    Some(employer.as_str()),
                    Some(role.as_str()),
                    &[],
                ));
            }
            if let Some(letter) = result.cover_letter.as_ref() {
                reports.push(crate::guardrails::evaluate_document(
                    crate::guardrails::COVER_LETTER,
                    &letter.markdown_output,
                    &claims,
                    &rules_letter,
                    Some(employer.as_str()),
                    Some(role.as_str()),
                    &required_gaps,
                ));
            }
            // Advisory model pass: looks for scope inflation the lexicon misses.
            // It never blocks, and every observation must cite a real claim and
            // quote a sentence that actually appears in the document.
            let scope_config = crate::ollama::get_ollama_config(state.clone()).unwrap_or_default();
            let scope_model = scope_config
                .fast_model
                .clone()
                .or_else(|| scope_config.deep_model.clone())
                .unwrap_or_else(|| "llama3.2:latest".to_string());
            let mut advisory = crate::guardrails::GuardrailReport::default();
            let mut review_error: Option<String> = None;
            for (kind, draft) in [
                (
                    crate::guardrails::RESUME,
                    result.resume_tailored.as_ref().map(|resume| resume.markdown_output.as_str()),
                ),
                (
                    crate::guardrails::COVER_LETTER,
                    result.cover_letter.as_ref().map(|letter| letter.markdown_output.as_str()),
                ),
            ] {
                let Some(draft) = draft else { continue };
                let _ = kind;
                let outcome =
                    crate::scope_review::review(draft, &claims, &scope_model, scope_config.num_ctx)
                        .await;
                if let Some(error) = outcome.error {
                    review_error.get_or_insert(error);
                    continue;
                }
                if outcome.ran {
                    advisory.checks_run += 1;
                }
                advisory.findings.extend(crate::scope_review::advisory_findings(&outcome.findings));
            }
            if let Some(error) = review_error {
                advisory.findings.push(crate::guardrails::GuardrailFinding {
                    code: "scope_review_unavailable".to_string(),
                    message: format!("the advisory model scope review did not run: {}", error),
                    blocking: false,
                });
            }
            reports.push(advisory);

            let mut cited = Vec::new();
            if let Some(resume) = result.resume_tailored.as_ref() {
                cited.extend(crate::guardrails::cited_claim_ids(&resume.markdown_output, &claims));
            }
            if let Some(letter) = result.cover_letter.as_ref() {
                cited.extend(crate::guardrails::cited_claim_ids(&letter.markdown_output, &claims));
            }
            cited.sort();
            cited.dedup();
            cited_claim_ids = cited;

            let mut report = crate::guardrails::merge(reports);
            report
                .findings
                .extend(crate::guardrails::weak_provenance_findings(&claims, &cited_claim_ids));
            let injection_warning = result
                .resume_tailored
                .as_ref()
                .and_then(|skill| skill.untrusted_input_warning.clone())
                .or_else(|| {
                    result
                        .cover_letter
                        .as_ref()
                        .and_then(|skill| skill.untrusted_input_warning.clone())
                });
            if let Some(warning) = injection_warning.as_ref() {
                report.findings.push(crate::guardrails::GuardrailFinding {
                    code: "untrusted_input_instruction".to_string(),
                    message: warning.clone(),
                    blocking: false,
                });
            }
            let posting_truncated = result
                .resume_tailored
                .as_ref()
                .map(|skill| skill.jd_truncated)
                .unwrap_or(false)
                || result
                    .cover_letter
                    .as_ref()
                    .map(|skill| skill.jd_truncated)
                    .unwrap_or(false);
            if posting_truncated {
                report.findings.push(crate::guardrails::GuardrailFinding {
                    code: "posting_truncated".to_string(),
                    message: format!(
                        "the posting exceeded the {} byte prompt budget; its tail was omitted from generation",
                        crate::skills::MAX_JD_PROMPT_BYTES
                    ),
                    blocking: false,
                });
            }
            // A must-have the user chose to leave unaddressed is a decision, not a
            // silence: it is reported here and carried into the audit record.
            for requirement in crate::guardrails::accepted_gap_requirements(&conn, &app_id) {
                report.findings.push(crate::guardrails::GuardrailFinding {
                    code: "gap_intentionally_unaddressed".to_string(),
                    message: format!(
                        "you chose to leave this requirement unaddressed: {}",
                        requirement
                    ),
                    blocking: false,
                });
            }
            report.blocking = report.findings.iter().any(|finding| finding.blocking);
            report.passed = !report.blocking;
            let _ = fs::write(
                app_dir.join("guardrail-report.json"),
                serde_json::to_string_pretty(&report).unwrap_or_default(),
            );

            let mut all_rules = rules_resume.clone();
            all_rules.extend(rules_letter.iter().cloned());
            let (rule_ids, rules_sha256) = crate::compliance::rules_fingerprint(&all_rules);
            let generator = result
                .resume_tailored
                .as_ref()
                .or(result.cover_letter.as_ref());
            let mut audit = crate::compliance::new_event(&app_id, "generated");
            audit.model = generator.and_then(|skill| skill.model.clone());
            audit.model_digest = generator.and_then(|skill| skill.model_digest.clone());
            audit.template_version = crate::skills::TEMPLATE_VERSION;
            audit.rule_ids = rule_ids;
            audit.rules_sha256 = rules_sha256;
            if let Some(resume) = result.resume_tailored.as_ref() {
                audit.documents.push(crate::compliance::digest_text(
                    "resume.md",
                    &resume.markdown_output,
                ));
            }
            if let Some(letter) = result.cover_letter.as_ref() {
                audit.documents.push(crate::compliance::digest_text(
                    "cover-letter.md",
                    &letter.markdown_output,
                ));
            }
            // Prompt and reply tokens per document: a reply that stopped at the
            // budget, or came close to it, is then visible without a rerun.
            audit.generation_usage = [
                result.resume_tailored.as_ref(),
                result.cover_letter.as_ref(),
            ]
            .into_iter()
            .flatten()
            .filter_map(|skill| skill.usage.clone())
            .collect();
            audit.blocking = report.blocking;
            audit.findings = report.findings.clone();
            audit.advisory_count = report
                .findings
                .iter()
                .filter(|finding| !finding.blocking)
                .count();
            if injection_warning.is_some() {
                audit.note = Some(
                    "Untrusted input warning: the posting contained instruction-like text."
                        .to_string(),
                );
            }
            if let Err(error) = crate::compliance::append_event(&app_dir, &audit) {
                append_warning(
                    &mut result.error_message,
                    format!("Audit record not written: {}", error),
                );
            }
            result.guardrail_report = Some(report);

            // Recruiter-style verdict over the documents just written. Evidence is
            // built before the call so the model can only cite real ledger items.
            let resume_md = result
                .resume_tailored
                .as_ref()
                .map(|resume| resume.markdown_output.clone())
                .unwrap_or_default();
            let letter_md = result
                .cover_letter
                .as_ref()
                .map(|letter| letter.markdown_output.clone())
                .unwrap_or_default();
            let review_context = crate::recruiter_review::build_context(&conn, &app_id, &claims);
            // The deep model handles the JSON contract; the fast model tends to echo
            // the schema itself instead of choosing a verdict.
            let review_model = scope_config
                .deep_model
                .clone()
                .or_else(|| scope_config.fast_model.clone())
                .unwrap_or_else(|| "llama3.2:latest".to_string());
            match crate::recruiter_review::review_documents(
                &employer,
                &role,
                &jd_text,
                &resume_md,
                &letter_md,
                &claims,
                &review_context,
                &review_model,
                scope_config.num_ctx,
            )
            .await
            {
                Ok(review) => {
                    let _ = fs::write(app_dir.join("recruiter-review.md"), &review.markdown);
                    let _ = fs::write(
                        app_dir.join("recruiter-review.json"),
                        serde_json::to_string_pretty(&review).unwrap_or_default(),
                    );
                    result.recruiter_review = Some(review);
                }
                Err(error) => append_warning(
                    &mut result.error_message,
                    format!("Recruiter review skipped: {}", error),
                ),
            }
        }
        Err(error) => {
            result.error_message = Some(format!("Guardrail evaluation failed: {}", error));
        }
    }

    emit_stage(&app, "fact-check", "Fact-check both documents", "done", None);

    if cancelled(&mut result, &app, "render", "Compile PDFs and run visual QA") {
        return Ok(result);
    }
    emit_stage(&app, "render", "Compile PDFs and run visual QA", "active", None);

    // 5. Compile both documents to PDF
    if let Some(resume_md) = result.resume_tailored.as_ref().map(|r| r.markdown_output.clone()) {
        match compile_document(&resume_md, "resume", &app_dir, &app) {
            Ok(pdf) => result.resume_pdf_compiled = Some(pdf),
            Err(e) => {
                emit_stage(&app, "render", "Compile PDFs and run visual QA", "failed", Some(e.clone()));
                result.error_message = Some(format!("Resume PDF failed: {}", e));
                return Ok(result);
            }
        }
    }

    if let Some(pdf) = result.resume_pdf_compiled.as_ref().map(|pdf| pdf.pdf_path.clone()) {
        match run_visual_qa("resume", &pdf, &app_dir, &employer, &role, &app) {
            Ok((raster, flags)) => {
                result.resume_pages_rasterized = Some(raster);
                result.resume_visual_flags = Some(flags);
            }
            Err(e) => append_warning(
                &mut result.error_message,
                format!("Resume visual QA skipped: {}", e),
            ),
        }
    }

    if let Some(cover_md) = result.cover_letter.as_ref().map(|r| r.markdown_output.clone()) {
        match compile_document(&cover_md, "cover-letter", &app_dir, &app) {
            Ok(pr) => {
                let pdf_path = pr.pdf_path.clone();
                result.pdf_compiled = Some(pr);

                // 6. Rasterize and analyze layout. QA is advisory: generated documents are kept.
                match run_visual_qa("cover-letter", &pdf_path, &app_dir, &employer, &role, &app) {
                    Ok((raster, flags)) => {
                        result.pages_rasterized = Some(raster);
                        result.visual_flags = Some(flags);
                    }
                    Err(e) => append_warning(
                        &mut result.error_message,
                        format!("Cover letter visual QA skipped: {}", e),
                    ),
                }
            }
            Err(e) => {
                emit_stage(&app, "render", "Compile PDFs and run visual QA", "failed", Some(e.clone()));
                result.error_message = Some(format!("Cover letter PDF failed: {}", e));
                return Ok(result);
            }
        }
    }

    emit_stage(&app, "render", "Compile PDFs and run visual QA", "done", None);
    emit_stage(&app, "save", "Save, sync ledger and commit", "active", None);

    // 8. Update application_records with results
    let claims_used = cited_claim_ids.join(",");
    let gaps_named: String = conn.query_row(
        "SELECT COUNT(*) FROM gaps WHERE application_id = ?1 AND status = 'named_in_letter'",
        [app_id.as_str()],
        |row| row.get::<_, i64>(0),
    ).unwrap_or(0).to_string();
    // This column records confirmed wording approved for reuse, not a generated document.
    // It is populated by the approval UI, so commit must preserve it instead of
    // overwriting it with an empty string (D8).
    let approved_wording: String = conn
        .query_row(
            "SELECT approved_wording FROM application_records WHERE id = ?1",
            [app_id.as_str()],
            |row| row.get(0),
        )
        .unwrap_or_default();
    let materials_version = crate::compliance::materials_version(&app_dir);

    conn.execute(
        "UPDATE application_records SET claims_used = ?1, gaps_named = ?2, approved_wording = ?3, materials_version = ?4 WHERE id = ?5",
        [claims_used.as_str(), gaps_named.as_str(), approved_wording.as_str(), materials_version.as_str(), app_id.as_str()],
    ).map_err(|e| format!("Failed to update application record: {}", e))?;

    // 9. Sync CSV mirror using the same 15-column header the workspace seeds.
    let csv_path = workspace_root.join("applications.csv");
    let folder_path = app_dir.to_string_lossy().into_owned();
    let csv_header = "id,date,employer,role,status,job_url,folder_path,req_id,base_resume_version,materials_version,claims_used,approved_wording,gaps_named,salary_entered,notes\n";
    let csv_entry = format!(
        "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
        csv_field(&app_id),
        csv_field(&now),
        csv_field(&employer),
        csv_field(&role),
        csv_field("PENDING_REVIEW"),
        csv_field(&input.jd_url),
        csv_field(&folder_path),
        csv_field(input.req_id.as_deref().unwrap_or("")),
        csv_field("master_v1"),
        csv_field(&materials_version),
        csv_field(&claims_used),
        csv_field(&approved_wording),
        csv_field(&gaps_named),
        csv_field(""),
        csv_field(&notes),
    );

    if !csv_path.exists() {
        fs::write(&csv_path, csv_header)
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

    // 10. Append to the daily log (non-fatal)
    if let Err(error) = crate::workspace::append_daily_log(&format!("Applied: {} - {}", employer, role)) {
        result.error_message = Some(format!(
            "Application saved, but the daily log could not be updated: {}", error
        ));
    }

    // Approval is always a deliberate human action. The generation pipeline never
    // commits, even if an older caller sets defer_commit = false. The review gate
    // also requires the per-application disclosure acknowledgement.
    emit_stage(
        &app,
        "save",
        "Save, sync ledger and commit",
        "awaiting",
        Some("Review the materials, acknowledge the disclosure, then approve to commit".to_string()),
    );
    result.success = true;
    Ok(result)
}

/// Resolve employer/role from the command line, falling back to the scraped posting.
fn resolve_metadata(provided: &Option<String>, scraped: &str, placeholder: &str) -> String {
    if let Some(value) = provided.as_ref().map(|value| value.trim()).filter(|value| !value.is_empty()) {
        return value.to_string();
    }
    let scraped = scraped.trim();
    if scraped.is_empty() || scraped == placeholder {
        "Unknown".to_string()
    } else {
        scraped.to_string()
    }
}

/// Quote a CSV field so commas, quotes and newlines cannot corrupt the mirror.
fn csv_field(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

/// Append a non-fatal problem so the user still sees the result that was produced.
fn append_warning(slot: &mut Option<String>, message: String) {
    let combined = match slot.take() {
        Some(existing) => format!("{}\n{}", existing, message),
        None => message,
    };
    *slot = Some(combined);
}

/// Rasterize a compiled PDF into its own QA folder and analyze the pages.
fn run_visual_qa(
    stem: &str,
    pdf_path: &str,
    app_dir: &PathBuf,
    employer: &str,
    role: &str,
    app: &tauri::AppHandle,
) -> Result<(
    crate::typst_engine::RasterizeResult,
    crate::typst_engine::VisualFlags,
), String> {
    let qa_dir = app_dir.join(format!("qa-{}", stem));
    let raster = crate::typst_engine::rasterize_pdf_pages(
        pdf_path.to_string(),
        qa_dir.to_string_lossy().into_owned(),
        app.clone(),
    )?;
    let flags = crate::typst_engine::analyze_visual_qa(
        pdf_path.to_string(),
        raster.png_paths.clone(),
        Some(employer.to_string()),
        Some(role.to_string()),
        app.clone(),
    )?;
    Ok((raster, flags))
}

/// Write a generated document as Markdown and Typst, then compile it to PDF.
fn compile_document(
    markdown: &str,
    stem: &str,
    app_dir: &PathBuf,
    app: &tauri::AppHandle,
) -> Result<crate::typst_engine::CompileResult, String> {
    let markdown = if stem == "resume" {
        crate::typst_engine::polish_resume_markdown(markdown)
    } else {
        markdown.to_string()
    };
    fs::write(app_dir.join(format!("{}.md", stem)), &markdown)
        .map_err(|e| format!("Failed to write {}.md: {}", stem, e))?;
    let typ_path = app_dir.join(format!("{}.typ", stem));
    fs::write(
        &typ_path,
        crate::typst_engine::with_document_font_defaults(&markdown),
    ).map_err(|e| format!("Failed to write {}.typ: {}", stem, e))?;

    let compiled = crate::typst_engine::compile_typst_pdf(
        typ_path.to_string_lossy().into_owned(),
        app.clone(),
    )?;
    if compiled.success {
        Ok(compiled)
    } else {
        Err(compiled
            .error_message
            .unwrap_or_else(|| "Typst compilation failed".to_string()))
    }
}

/// Default export location: the workspace exports folder.
fn default_export_dir() -> String {
    dirs::document_dir()
        .map(|docs| docs.join("CareerOS").join("exports"))
        .unwrap_or_else(std::env::temp_dir)
        .to_string_lossy()
        .into_owned()
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

    let response = crate::ollama::chat_with_ollama_async(
        model,
        messages,
        Some(0.3),
        Some(1000),
        None,
        config.num_ctx,
    )
    .await
    .map_err(|e| format!("Company research failed: {}", e))?;

    Ok(response.message.content)
}

/// Draft follow-up emails for stale applications (/followup command).
pub const DEFAULT_FOLLOWUP_DAYS: u32 = 21;

/// One application that could be followed up.
#[derive(Debug, Clone, Serialize)]
pub struct FollowupCandidate {
    pub id: String,
    pub employer: String,
    pub role: String,
    pub date: String,
    pub days_ago: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FollowupCandidates {
    pub days: u32,
    pub candidates: Vec<FollowupCandidate>,
    /// Set when the window holds nothing, so the caller can say why.
    pub note: Option<String>,
}

fn parse_application_date(value: &str) -> Option<chrono::NaiveDate> {
    let text = value.trim();
    chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .ok()
        .or_else(|| {
            chrono::DateTime::parse_from_rfc3339(text)
                .ok()
                .map(|stamp| stamp.date_naive())
        })
}

fn followup_rows(conn: &rusqlite::Connection) -> Result<Vec<(String, String, String, String)>, String> {
    let mut statement = conn
        .prepare("SELECT id, employer, role, date FROM application_records WHERE status IN ('SUBMITTED', 'INTERVIEWING') ORDER BY date DESC")
        .map_err(|error| format!("Failed to prepare query: {}", error))?;
    let rows: Vec<(String, String, String, String)> = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|error| format!("Failed to query apps: {}", error))?
        .filter_map(|row| row.ok())
        .collect();
    Ok(rows)
}

/// Keep applications inside the window, newest first. An empty window says so and
/// names the most recent application instead of returning a silent empty list.
pub(crate) fn select_followup_candidates(
    rows: Vec<(String, String, String, String)>,
    days: u32,
    today: chrono::NaiveDate,
) -> FollowupCandidates {
    let mut all: Vec<FollowupCandidate> = rows
        .into_iter()
        .map(|(id, employer, role, date)| {
            let days_ago = parse_application_date(&date).map(|applied| (today - applied).num_days());
            FollowupCandidate {
                id,
                employer,
                role,
                date,
                days_ago,
            }
        })
        .collect();
    all.sort_by_key(|candidate| candidate.days_ago.unwrap_or(i64::MAX));

    let candidates: Vec<FollowupCandidate> = all
        .iter()
        .filter(|candidate| candidate.days_ago.map_or(true, |ago| ago <= days as i64))
        .cloned()
        .collect();
    if !candidates.is_empty() {
        return FollowupCandidates {
            days,
            candidates,
            note: None,
        };
    }

    let note = match all
        .iter()
        .find_map(|candidate| candidate.days_ago.map(|ago| (candidate, ago)))
    {
        Some((candidate, ago)) => Some(format!(
            "No application applied in the last {} days is waiting on a follow-up. The most recent is {} — {}, applied {} days ago. Run `/followup {}` to widen the window.",
            days,
            candidate.employer,
            candidate.role,
            ago,
            (ago + 1).max(days as i64 + 1)
        )),
        None => Some(
            "No application is marked SUBMITTED or INTERVIEWING yet. Set that status in the ledger, then run /followup again."
                .to_string(),
        ),
    };

    FollowupCandidates {
        days,
        candidates: Vec::new(),
        note,
    }
}

/// List who could be followed up, without spending a model call.
#[tauri::command]
pub fn list_followup_candidates(
    days: Option<u32>,
    state: State<'_, AppState>,
) -> Result<FollowupCandidates, String> {
    let db_path = {
        let guard = state.db_path.lock().unwrap();
        guard
            .as_ref()
            .ok_or("Database not initialized")?
            .clone()
    };
    let conn = get_db_connection(&db_path)?;
    let rows = followup_rows(&conn)?;
    Ok(select_followup_candidates(
        rows,
        days.unwrap_or(DEFAULT_FOLLOWUP_DAYS),
        chrono::Local::now().date_naive(),
    ))
}

#[tauri::command]
pub async fn draft_followups(
    application_id: Option<String>,
    days: Option<u32>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let db_path_str = {
        let db_path_opt = state.db_path.lock().unwrap();
        db_path_opt.as_ref()
            .ok_or("Database not initialized")?
            .clone()
    };
    let conn = get_db_connection(&db_path_str)?;

    // Every application still waiting on a reply, newest first.
    let rows = followup_rows(&conn)?;
    let (apps, header) = match application_id.as_deref() {
        Some(id) => {
            let found = rows
                .into_iter()
                .find(|(row_id, _, _, _)| row_id == id)
                .ok_or_else(|| format!("Application not found: {}", id))?;
            let header = format!("## Follow-up — {}, {}\n\n", found.1, found.2);
            (vec![found], header)
        }
        None => {
            let selection = select_followup_candidates(
                rows,
                days.unwrap_or(DEFAULT_FOLLOWUP_DAYS),
                chrono::Local::now().date_naive(),
            );
            if selection.candidates.is_empty() {
                return Ok(selection
                    .note
                    .unwrap_or_else(|| "No applications awaiting follow-up.".to_string()));
            }
            let header = format!(
                "## Follow-up drafts\n\n_Submitted or interviewing, applied in the last {} days._\n\n",
                selection.days
            );
            let apps = selection
                .candidates
                .into_iter()
                .map(|candidate| (candidate.id, candidate.employer, candidate.role, candidate.date))
                .collect::<Vec<_>>();
            (apps, header)
        }
    };

    let config = crate::ollama::get_ollama_config(state).unwrap_or_default();
    let model = config.fast_model.clone()
        .or_else(|| config.deep_model.clone())
        .unwrap_or_else(|| "llama3.2:latest".to_string());

    const MAX_DRAFTS: usize = 10;
    let mut drafts = header;
    let mut drafted = 0usize;

    for (_, employer, role, date) in apps.iter().take(MAX_DRAFTS) {
        let messages = vec![
            crate::ollama::OllamaChatMessage {
                role: "system".to_string(),
                content: "You write brief, professional job-application follow-up emails. Return only the email body, under 150 words, with no subject line and no bracketed placeholders.".to_string(),
            },
            crate::ollama::OllamaChatMessage {
                role: "user".to_string(),
                content: format!(
                    "Draft a follow-up email to {} about the {} role I applied for on {}. Reiterate interest, state one concrete value I bring, and ask about next steps. Do not invent facts beyond the employer, role, and application date.",
                    employer, role, date
                ),
            },
        ];

        match crate::ollama::chat_with_ollama_async(
            model.clone(),
            messages,
            Some(0.4),
            Some(400),
            None,
            config.num_ctx,
        )
        .await
        {
            Ok(response) => {
                drafts.push_str(&format!(
                    "### {} - {}\n_Applied {}_\n\n{}\n\n",
                    employer, role, date, response.message.content.trim()
                ));
                drafted += 1;
            }
            Err(error) => {
                drafts.push_str(&format!(
                    "### {} - {}\n\n_Drafting failed: {}_\n\n",
                    employer, role, error
                ));
            }
        }
    }

    if apps.len() > MAX_DRAFTS {
        drafts.push_str(&format!(
            "_{} more application(s) were not drafted._\n",
            apps.len() - MAX_DRAFTS
        ));
    }
    let _ = crate::workspace::append_daily_log(&format!("Drafted {} follow-up email(s)", drafted));

    Ok(drafts)
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

/// Aggregate honesty evidence plus the review cadence.
#[tauri::command]
pub fn get_honesty_stats(
    _state: State<'_, AppState>,
) -> Result<crate::compliance::HonestyStats, String> {
    let docs_dir = dirs::document_dir().ok_or("Failed to locate Documents folder")?;
    let workspace_root = docs_dir.join("CareerOS");
    Ok(crate::compliance::stats(&workspace_root))
}

const PROBLEM_CATEGORIES: &[&str] = &[
    "false_block",
    "missed_false_claim",
    "scope_error",
    "wrong_evidence",
    "other",
];

/// Record a structured incident. This is the recourse path the standards expect;
/// it writes a separate append-only file, not a database edit.
#[tauri::command]
pub fn report_output_problem(
    application_id: String,
    category: String,
    note: String,
    _state: State<'_, AppState>,
) -> Result<crate::compliance::ProblemReport, String> {
    let docs_dir = dirs::document_dir().ok_or("Failed to locate Documents folder")?;
    let app_dir = docs_dir
        .join("CareerOS")
        .join("applications")
        .join(&application_id);
    let category = if PROBLEM_CATEGORIES.contains(&category.as_str()) {
        category
    } else {
        "other".to_string()
    };
    let report = crate::compliance::ProblemReport {
        incident_id: Uuid::new_v4().to_string(),
        timestamp: Utc::now().to_rfc3339(),
        application_id,
        category,
        note,
        status: "open".to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
    };
    crate::compliance::append_problem(&app_dir, &report)?;
    Ok(report)
}

/// Export every structured incident row as CSV. Returns the chosen path.
#[tauri::command]
pub fn export_incident_log() -> Result<Option<String>, String> {
    let docs_dir = dirs::document_dir().ok_or("Failed to locate Documents folder")?;
    let workspace_root = docs_dir.join("CareerOS");
    let reports = crate::compliance::all_problem_reports(&workspace_root);
    let mut csv = String::from(
        "incident_id,timestamp,application_id,category,status,note,app_version\n",
    );
    for report in reports {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            csv_field(&report.incident_id),
            csv_field(&report.timestamp),
            csv_field(&report.application_id),
            csv_field(&report.category),
            csv_field(&report.status),
            csv_field(&report.note),
            csv_field(&report.app_version),
        ));
    }

    let selected = FileDialog::new()
        .set_file_name("careeros-incident-log.csv")
        .add_filter("CSV", &["csv"])
        .save_file();
    match selected {
        Some(path) => {
            fs::write(&path, csv).map_err(|error| format!("Failed to write incident log: {}", error))?;
            Ok(Some(path.to_string_lossy().into_owned()))
        }
        None => Ok(None),
    }
}

/// Record a review of the honesty metrics. The next due date is derived from this row.
#[tauri::command]
pub fn record_honesty_review(
    reviewer: String,
    note: String,
    state: State<'_, AppState>,
) -> Result<crate::compliance::HonestyStats, String> {
    let db_path_str = {
        let db_path_opt = state.db_path.lock().unwrap();
        db_path_opt
            .as_ref()
            .ok_or("Database not initialized")?
            .clone()
    };
    let workspace_root = PathBuf::from(&db_path_str)
        .parent()
        .ok_or("Invalid database path")?
        .to_path_buf();
    let stats = crate::compliance::stats(&workspace_root);
    let reviewer = if reviewer.trim().is_empty() {
        "local-user".to_string()
    } else {
        reviewer.trim().to_string()
    };
    let record = crate::compliance::ReviewRecord {
        review_id: Uuid::new_v4().to_string(),
        timestamp: Utc::now().to_rfc3339(),
        reviewer,
        note: note.trim().to_string(),
        applications_covered: stats.applications,
        blocking_findings: stats.blocked_findings,
        overrides: stats.overrides,
        false_block_reports: stats.false_block_reports,
    };
    crate::compliance::append_review(&workspace_root, &record)?;
    Ok(crate::compliance::stats(&workspace_root))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiDisclosureState {
    pub acknowledged: bool,
    pub version: String,
    pub text: String,
}

#[tauri::command]
pub fn get_ai_disclosure_state(
    state: State<'_, AppState>,
) -> Result<AiDisclosureState, String> {
    let db_path_str = {
        let db_path_opt = state.db_path.lock().unwrap();
        db_path_opt
            .as_ref()
            .ok_or("Database not initialized")?
            .clone()
    };
    let conn = get_db_connection(&db_path_str)?;
    let acknowledged: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'ai_disclosure_ack_version'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("Failed to read disclosure state: {}", error))?;
    Ok(AiDisclosureState {
        acknowledged: acknowledged.as_deref() == Some(crate::compliance::DISCLOSURE_VERSION),
        version: crate::compliance::DISCLOSURE_VERSION.to_string(),
        text: crate::compliance::DISCLOSURE_TEXT.to_string(),
    })
}

#[tauri::command]
pub fn acknowledge_ai_disclosure(
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let db_path_str = {
        let db_path_opt = state.db_path.lock().unwrap();
        db_path_opt
            .as_ref()
            .ok_or("Database not initialized")?
            .clone()
    };
    let conn = get_db_connection(&db_path_str)?;
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('ai_disclosure_ack_version', ?1)",
        [crate::compliance::DISCLOSURE_VERSION],
    )
    .map_err(|error| format!("Failed to save disclosure acknowledgement: {}", error))?;
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('ai_disclosure_ack_at', ?1)",
        [now.as_str()],
    )
    .map_err(|error| format!("Failed to save disclosure timestamp: {}", error))?;

    let workspace_root = PathBuf::from(&db_path_str)
        .parent()
        .ok_or("Invalid database path")?
        .to_path_buf();
    crate::compliance::append_disclosure(&workspace_root, None, "first-run")?;
    Ok(true)
}

/// Per-application acknowledgement. It is written after the generation it covers,
/// so regenerating requires a fresh acknowledgement.
#[tauri::command]
pub fn acknowledge_output_disclosure(
    application_id: String,
    _state: State<'_, AppState>,
) -> Result<bool, String> {
    let docs_dir = dirs::document_dir().ok_or("Failed to locate Documents folder")?;
    let workspace_root = docs_dir.join("CareerOS");
    let app_dir = workspace_root.join("applications").join(&application_id);

    let mut event = crate::compliance::new_event(&application_id, "disclosure_acknowledged");
    event.note = Some(crate::compliance::DISCLOSURE_VERSION.to_string());
    event.documents = crate::compliance::document_digests(&app_dir);
    crate::compliance::append_event(&app_dir, &event)?;
    crate::compliance::append_disclosure(&workspace_root, Some(&application_id), "per-application")?;
    Ok(true)
}

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
            "INSERT INTO claims (id, statement, evidence_source, confirmed_wording, scope, category, status, last_confirmed_at, evidence_type, evidence_sha256, verified_by)
             SELECT ?1, ?2, ?3, ?4, ?5, ?6, 'needs-confirmation', ?7, ?8, ?9, ''
             WHERE NOT EXISTS (
                 SELECT 1 FROM claims
                 WHERE statement = ?2
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
                "resume",
                crate::claims::claim_evidence_sha256(
                    &suggestion.statement,
                    &suggestion.evidence_source,
                    "resume",
                    &suggestion.scope,
                    &suggestion.category,
                )
                .as_str(),
            ],
        ).map_err(|e| format!("Failed to save claim: {}", e))?;
        if inserted > 0 {
            count += 1;
        }
    }

    crate::setup_wizard::update_setup_step(&conn, "resume")?;
    // Imported claims stay staged until the user confirms them in the setup wizard.

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

    let out_dir = output_dir
        .filter(|dir| !dir.trim().is_empty())
        .unwrap_or_else(default_export_dir);

    crate::resume_exporter::export_ats_resume(&markdown, export_format, &out_dir, &app)
}

/// The fit matrix as it stood when this application was analysed.
#[tauri::command]
pub fn get_fit_matrix(
    application_id: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let _ = state;
    let docs = dirs::document_dir().ok_or("Failed to locate Documents folder")?;
    let path = docs
        .join("CareerOS")
        .join("applications")
        .join(&application_id)
        .join("fit-matrix.json");
    let text = fs::read_to_string(&path)
        .map_err(|_| "No fit matrix has been saved for this application yet.".to_string())?;
    serde_json::from_str(&text).map_err(|error| format!("Failed to read the fit matrix: {}", error))
}

#[tauri::command]
pub fn get_master_resume(state: State<'_, AppState>) -> Result<String, String> {
    let db_path_str = {
        let db_path_opt = state.db_path.lock().unwrap();
        db_path_opt.as_ref()
            .ok_or("Database not initialized")?
            .clone()
    };
    let conn = get_db_connection(&db_path_str)?;

    conn.query_row(
        "SELECT value FROM app_settings WHERE key = 'master_resume'",
        [],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|e| format!("Failed to load master resume: {}", e))?
    .filter(|value| !value.trim().is_empty())
    .ok_or_else(|| "No master resume found. Import one with /resume first.".to_string())
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
#[cfg(test)]
mod followup_tests {
    use super::select_followup_candidates;

    fn row(id: &str, employer: &str, date: &str) -> (String, String, String, String) {
        (
            id.to_string(),
            employer.to_string(),
            "Learning Lead".to_string(),
            date.to_string(),
        )
    }

    #[test]
    fn the_window_keeps_recent_applications_and_orders_them_newest_first() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 11, 8).unwrap();
        let selection = select_followup_candidates(
            vec![row("a", "Old", "2026-09-01"), row("b", "Recent", "2026-11-05")],
            21,
            today,
        );
        assert_eq!(selection.days, 21);
        let employers: Vec<&str> = selection
            .candidates
            .iter()
            .map(|candidate| candidate.employer.as_str())
            .collect();
        assert_eq!(employers, vec!["Recent"]);
        assert_eq!(selection.candidates[0].days_ago, Some(3));
    }

    #[test]
    fn an_empty_window_names_the_most_recent_application() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 11, 8).unwrap();
        let selection = select_followup_candidates(vec![row("a", "Old", "2026-10-05")], 21, today);
        assert!(selection.candidates.is_empty());
        let note = selection.note.expect("note");
        assert!(note.contains("Old"), "{note}");
        assert!(note.contains("34 days"), "{note}");
        assert!(note.contains("/followup"), "{note}");
    }
}
