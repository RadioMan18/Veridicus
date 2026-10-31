//! Skill Execution Engine for CareerOS.
//!
//! Loads system prompts, master resume, and skill templates; assembles
//! guardrailed prompts using only confirmed claims; invokes the deep Ollama
//! model; extracts Markdown output mapped to `.typ` Typst format.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::State;
use crate::AppState;
use crate::db::get_db_connection;
use crate::ollama::{chat_with_ollama, OllamaChatMessage};
use crate::claims::Claim;

/// Represents a skill template that defines how to generate documents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub template_path: String,
    pub system_prompt: String,
    pub applies_to: Vec<String>, // e.g. ["resume", "cover_letter"]
}

/// Skill execution result containing generated Markdown and Typst output paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillExecutionResult {
    pub skill_id: String,
    pub markdown_output: String,
    pub typst_path: Option<String>,
    pub generation_time_ms: u64,
    pub success: bool,
    pub error_message: Option<String>,
}

/// Input parameters for executing a guardrailed skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillExecutionInput {
    pub skill_id: String,
    pub application_id: Option<String>,
    pub jd_text: Option<String>,
    pub use_deep_model: bool,
}

/// Load a skill template from the workspace `.careeros/skills/` directory.
fn load_skill_template(skill_id: &str, workspace_root: &PathBuf) -> Result<SkillTemplate, String> {
    let template_path = workspace_root.join(".careeros").join("skills").join(format!("{}.md", skill_id));
    if !template_path.exists() {
        return Err(format!("Skill template not found: {}", skill_id));
    }

    let content = fs::read_to_string(&template_path)
        .map_err(|e| format!("Failed to read skill template: {}", e))?;

    // Parse frontmatter-style metadata or use defaults
    let name = skill_id.replace('-', " ").replace('_', " ");
    let description = format!("{}: generate career document", name);

    Ok(SkillTemplate {
        id: skill_id.to_string(),
        name,
        description,
        template_path: template_path.to_string_lossy().into_owned(),
        system_prompt: content.clone(),
        applies_to: vec!["resume".to_string(), "cover_letter".to_string()],
    })
}

/// Fetch confirmed claims from the SQLCipher Claims Ledger.
fn fetch_confirmed_claims(conn: &rusqlite::Connection) -> Result<Vec<Claim>, String> {
    let mut stmt = conn.prepare("SELECT id, statement, evidence_source, confirmed_wording, scope, category, status, last_confirmed_at FROM claims WHERE status = 'confirmed'")
        .map_err(|e| format!("Failed to prepare claims query: {}", e))?;
    let claims: Vec<Claim> = stmt.query_map([], |row| {
        Ok(Claim {
            id: row.get(0)?,
            statement: row.get(1)?,
            evidence_source: row.get(2)?,
            confirmed_wording: row.get(3)?,
            scope: row.get(4)?,
            category: row.get(5)?,
            status: row.get(6)?,
            last_confirmed_at: row.get(7)?,
        })
    }).map_err(|e| format!("Failed to query claims: {}", e))?
    .filter_map(|r| r.ok())
    .collect();

    Ok(claims)
}

/// Fetch active rules from the rules table (never_claim, exact_wording, presentation).
fn fetch_active_rules(conn: &rusqlite::Connection) -> Result<Vec<String>, String> {
    let mut never_claims: Vec<String> = Vec::new();
    let mut exact_wordings: Vec<String> = Vec::new();
    let mut presentation_rules: Vec<String> = Vec::new();

    let mut rows = conn.prepare(
        "SELECT kind, rule_text FROM rules WHERE active = 1"
    ).map_err(|e| format!("Failed to prepare rules query: {}", e))?;

    let rule_iter = rows.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }).map_err(|e| format!("Failed to query rules: {}", e))?;

    for rule in rule_iter.flatten() {
        match rule.0.as_str() {
            "never_claim" => never_claims.push(rule.1),
            "exact_wording" => exact_wordings.push(rule.1),
            "presentation" => presentation_rules.push(rule.1),
            _ => {}
        }
    }

    // Combine all rules into a single string for injection into the prompt
    let mut all_rules = String::new();
    if !never_claims.is_empty() {
        all_rules.push_str("## Never-Claim Rules\n");
        for rule in &never_claims {
            all_rules.push_str(&format!("- NEVER CLAIM: {}\n", rule));
        }
    }
    if !exact_wordings.is_empty() {
        all_rules.push_str("\n## Exact Wording Rules\n");
        for rule in &exact_wordings {
            all_rules.push_str(&format!("- Exact wording required: {}\n", rule));
        }
    }
    if !presentation_rules.is_empty() {
        all_rules.push_str("\n## Presentation Rules\n");
        for rule in &presentation_rules {
            all_rules.push_str(&format!("- {}\n", rule));
        }
    }

    Ok(vec![all_rules])
}

/// Assemble the guardrailed prompt from skill template, claims, and rules.
fn assemble_guardrailed_prompt(
    skill: &SkillTemplate,
    claims: &[Claim],
    rules: &[String],
    jd_text: Option<&str>,
) -> String {
    let mut prompt = String::new();

    // System instruction from skill template
    prompt.push_str(&skill.system_prompt);
    prompt.push_str("\n\n");

    // Confirmed claims section
    prompt.push_str("## Confirmed Claims Ledger\n");
    prompt.push_str("Use ONLY these verified claims. Do not invent or hallucinate any metrics, skills, or experience:\n\n");
    for claim in claims {
        prompt.push_str(&format!("- [{}] {} (evidence: {}, scope: {}, category: {})\n",
            claim.id, claim.confirmed_wording, claim.evidence_source, claim.scope, claim.category));
    }

    // Rules section
    for rule_block in rules {
        prompt.push_str(&format!("\n{}", rule_block));
    }

    // JD context if provided
    if let Some(jd) = jd_text {
        prompt.push_str(&format!("\n## Target Job Description\n{}\n", jd));
    }

    prompt.push_str("\n## Output\nGenerate the document content in Markdown format using only confirmed claims above.\n");

    prompt
}

/// Execute a guardrailed skill: assemble prompt, call deep Ollama model, return Markdown.
#[tauri::command]
pub async fn execute_skill(
    input: SkillExecutionInput,
    state: State<'_, AppState>,
) -> Result<SkillExecutionResult, String> {
    let start = std::time::Instant::now();

    // 1. Get DB path and open connection - clone path out of lock scope
    let db_path_str = {
        let db_path_opt = state.db_path.lock().unwrap();
        db_path_opt.as_ref()
            .ok_or("Database not initialized")?
            .clone()
    };

    let conn = get_db_connection(&db_path_str)?;

    // 2. Fetch confirmed claims
    let claims = fetch_confirmed_claims(&conn)?;

    if claims.is_empty() {
        return Ok(SkillExecutionResult {
            skill_id: input.skill_id.clone(),
            markdown_output: String::new(),
            typst_path: None,
            generation_time_ms: start.elapsed().as_millis() as u64,
            success: false,
            error_message: Some("No confirmed claims found in ledger. Complete setup wizard first.".to_string()),
        });
    }

    // 3. Fetch active rules
    let rules = fetch_active_rules(&conn)?;

    // 4. Load skill template
    let docs_dir = dirs::document_dir().ok_or("Failed to locate Documents folder")?;
    let workspace_root = docs_dir.join("CareerOS");
    let skill = load_skill_template(&input.skill_id, &workspace_root)?;

    // 5. Determine model: deep model if requested
    let ollama_config = crate::ollama::get_ollama_config(state);
    let config = ollama_config.unwrap_or_default();
    let model = if input.use_deep_model {
        config.deep_model.clone().unwrap_or_else(|| "qwen2.5:7b".to_string())
    } else {
        config.fast_model.clone().unwrap_or_else(|| "llama3.2:3b".to_string())
    };

    // 6. Assemble guardrailed prompt
    let full_prompt = assemble_guardrailed_prompt(&skill, &claims, &rules, input.jd_text.as_deref());

    // 7. Call Ollama chat
    let messages = vec![
        OllamaChatMessage {
            role: "system".to_string(),
            content: full_prompt,
        },
        OllamaChatMessage {
            role: "user".to_string(),
            content: "Generate the document content using only confirmed claims.".to_string(),
        },
    ];

    let chat_response = chat_with_ollama(
        model,
        messages,
        Some(0.3),
        Some(2000),
        None,
    ).map_err(|e| format!("Ollama generation failed: {}", e))?;

    let markdown_output = chat_response.message.content;
    let elapsed_ms = start.elapsed().as_millis() as u64;

    // 8. Optionally write Typst file
    let typst_path = if input.application_id.is_some() {
        let typ_dir = workspace_root.join("applications").join(input.application_id.as_ref().unwrap());
        fs::create_dir_all(&typ_dir).map_err(|e| format!("Failed to create app dir: {}", e))?;
        let typ_path = typ_dir.join(format!("{}.typ", input.skill_id));
        fs::write(&typ_path, &markdown_output).map_err(|e| format!("Failed to write Typst file: {}", e))?;
        Some(typ_path.to_string_lossy().into_owned())
    } else {
        None
    };

    Ok(SkillExecutionResult {
        skill_id: input.skill_id,
        markdown_output,
        typst_path,
        generation_time_ms: elapsed_ms,
        success: true,
        error_message: None,
    })
}

/// List available skill templates from the workspace.
#[tauri::command]
pub fn list_skill_templates() -> Result<Vec<SkillTemplate>, String> {
    let docs_dir = dirs::document_dir().ok_or("Failed to locate Documents folder")?;
    let skills_dir = docs_dir.join("CareerOS").join(".careeros").join("skills");

    if !skills_dir.exists() {
        return Ok(vec![]);
    }

    let mut templates = Vec::new();
    for entry in fs::read_dir(&skills_dir).map_err(|e| format!("Failed to read skills dir: {}", e))? {
        let entry = entry.map_err(|e| format!("Failed to read dir entry: {}", e))?;
        let path = entry.path();
        if path.extension().map(|e| e == "md").unwrap_or(false) {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                templates.push(SkillTemplate {
                    id: stem.to_string(),
                    name: stem.replace('-', " ").replace('_', " "),
                    description: format!("{} skill template", stem),
                    template_path: path.to_string_lossy().into_owned(),
                    system_prompt: String::new(), // loaded on demand
                    applies_to: vec!["resume".to_string(), "cover_letter".to_string()],
                });
            }
        }
    }

    Ok(templates)
}

/// Seed default skill templates into the workspace.
#[tauri::command]
pub fn seed_default_skills() -> Result<usize, String> {
    let docs_dir = dirs::document_dir().ok_or("Failed to locate Documents folder")?;
    let skills_dir = docs_dir.join("CareerOS").join(".careeros").join("skills");
    fs::create_dir_all(&skills_dir).map_err(|e| format!("Failed to create skills dir: {}", e))?;

    let default_skills = vec![
        ("resume-tailor", "Resume Tailor", "Tailor resume to job description using confirmed claims"),
        ("cover-letter", "Cover Letter Generator", "Generate cover letter from confirmed claims and JD"),
        ("jd-analyzer", "JD Analyzer", "Analyze job description requirements"),
        ("star-builder", "STAR Builder", "Build STAR method stories from claims"),
        ("negotiation-coach", "Negotiation Coach", "Generate salary negotiation talking points"),
    ];

    let mut count = 0;
    for (id, name, desc) in &default_skills {
        let path = skills_dir.join(format!("{}.md", id));
        if !path.exists() {
            let content = format!(
                "# {}\n\n## Purpose\n{}\n\n## Instructions\nUse only confirmed claims from the Claims Ledger.\n",
                name, desc
            );
            fs::write(&path, content).map_err(|e| format!("Failed to write skill template {}: {}", id, e))?;
            count += 1;
        }
    }

    Ok(count)
}