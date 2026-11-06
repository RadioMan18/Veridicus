use std::fs;
use std::path::PathBuf;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;
use chrono::Utc;

use crate::AppState;
use crate::db::get_db_connection;
use crate::claims::CreateClaimRequest;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SetupStep {
    pub id: String,
    pub title: String,
    pub description: String,
    pub completed: bool,
    pub required: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SetupWizardState {
    pub current_step: usize,
    pub total_steps: usize,
    pub steps: Vec<SetupStep>,
    pub completed: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SetupWizardData {
    pub profile: Option<UserProfile>,
    pub resume_content: String,
    pub selected_ollama_models: Vec<String>,
    pub presentation_rule: Option<PresentationRuleInput>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UserProfile {
    pub full_name: String,
    pub email: String,
    pub phone: Option<String>,
    pub location: Option<String>,
    pub linkedin: Option<String>,
    pub github: Option<String>,
    pub portfolio: Option<String>,
    pub target_roles: Vec<String>,
    pub years_experience: Option<u32>,
    pub current_title: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PresentationRuleInput {
    pub section: String,
    pub date_policy: String,
    pub tenure_phrasing: String,
    pub employer_neutral_metrics: bool,
}

#[tauri::command]
pub fn get_setup_wizard_state(
    state: State<'_, AppState>
) -> Result<SetupWizardState, String> {
    let db_path_opt = state.db_path.lock().unwrap();
    let db_path_str = db_path_opt.as_ref()
        .ok_or("Database not initialized")?;

    let conn = get_db_connection(db_path_str)?;

    let setup_completed: Option<String> = conn.query_row(
        "SELECT value FROM app_settings WHERE key = 'setup_completed'",
        [],
        |row| row.get(0),
    ).optional().map_err(|e| format!("Failed to read setup status: {}", e))?;
    let setup_completed = setup_completed.as_deref() == Some("true");

    let steps = vec![
        SetupStep {
            id: "welcome".to_string(),
            title: "Welcome to CareerOS".to_string(),
            description: "Set up your career management workspace".to_string(),
            completed: true,
            required: true,
        },
        SetupStep {
            id: "profile".to_string(),
            title: "Your Professional Profile".to_string(),
            description: "Enter your name, contact info, and target roles".to_string(),
            completed: setup_step_is_complete(&conn, "profile")?,
            required: true,
        },
        SetupStep {
            id: "resume".to_string(),
            title: "Master Resume".to_string(),
            description: "Upload or paste your master resume content".to_string(),
            completed: setup_step_is_complete(&conn, "resume")?,
            required: true,
        },
        SetupStep {
            id: "claims".to_string(),
            title: "Verify Your Claims".to_string(),
            description: "Review and confirm your skills, metrics, and experience".to_string(),
            completed: setup_step_is_complete(&conn, "claims")?,
            required: true,
        },
        SetupStep {
            id: "ollama".to_string(),
            title: "Local AI Models".to_string(),
            description: "Select Ollama models for generation and analysis".to_string(),
            completed: setup_step_is_complete(&conn, "ollama")?,
            required: true,
        },
        SetupStep {
            id: "presentation".to_string(),
            title: "Presentation Preferences".to_string(),
            description: "Choose how resume details should be presented".to_string(),
            completed: setup_step_is_complete(&conn, "presentation")?,
            required: true,
        },
    ];

    let current_step = steps.iter().position(|step| !step.completed).unwrap_or(steps.len());
    Ok(SetupWizardState {
        current_step,
        total_steps: steps.len(),
        steps,
        completed: setup_completed,
    })
}

#[tauri::command]
pub fn get_setup_wizard_data(
    state: State<'_, AppState>
) -> Result<SetupWizardData, String> {
    let db_path_opt = state.db_path.lock().unwrap();
    let db_path_str = db_path_opt.as_ref()
        .ok_or("Database not initialized")?;
    let conn = get_db_connection(db_path_str)?;

    let profile_json: Option<String> = conn.query_row(
        "SELECT value FROM app_settings WHERE key = 'user_profile'",
        [],
        |row| row.get(0),
    ).optional().map_err(|e| format!("Failed to read profile: {}", e))?;
    let profile = profile_json
        .map(|json| serde_json::from_str(&json).map_err(|e| format!("Failed to parse profile: {}", e)))
        .transpose()?;

    let resume_content = conn.query_row(
        "SELECT value FROM app_settings WHERE key = 'master_resume'",
        [],
        |row| row.get(0),
    ).optional().map_err(|e| format!("Failed to read master resume: {}", e))?
        .unwrap_or_default();

    let models_json: Option<String> = conn.query_row(
        "SELECT value FROM app_settings WHERE key = 'selected_ollama_models'",
        [],
        |row| row.get(0),
    ).optional().map_err(|e| format!("Failed to read selected models: {}", e))?;
    let selected_ollama_models = models_json
        .map(|json| serde_json::from_str(&json).map_err(|e| format!("Failed to parse selected models: {}", e)))
        .transpose()?
        .unwrap_or_default();

    let rule_text: Option<String> = conn.query_row(
        "SELECT rule_text FROM rules
         WHERE kind = 'presentation' AND subject = 'resume' AND source LIKE 'setup-wizard%'
         ORDER BY rowid DESC LIMIT 1",
        [],
        |row| row.get(0),
    ).optional().map_err(|e| format!("Failed to read presentation preferences: {}", e))?;
    let presentation_rule = rule_text.map(|text| parse_presentation_rule(&text)).transpose()?;

    Ok(SetupWizardData { profile, resume_content, selected_ollama_models, presentation_rule })
}

#[tauri::command]
pub fn save_user_profile(
    profile: UserProfile,
    state: State<'_, AppState>
) -> Result<bool, String> {
    let db_path_opt = state.db_path.lock().unwrap();
    let db_path_str = db_path_opt.as_ref()
        .ok_or("Database not initialized")?;

    let conn = get_db_connection(db_path_str)?;

    let json = serde_json::to_string(&profile)
        .map_err(|e| format!("Failed to serialize profile: {}", e))?;

    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('user_profile', ?1)",
        [json],
    ).map_err(|e| format!("Failed to save profile: {}", e))?;

    update_setup_step(&conn, "profile")?;

    Ok(true)
}

#[tauri::command]
pub fn save_resume_content(
    content: String,
    state: State<'_, AppState>
) -> Result<bool, String> {
    let db_path_opt = state.db_path.lock().unwrap();
    let db_path_str = db_path_opt.as_ref()
        .ok_or("Database not initialized")?;

    let conn = get_db_connection(db_path_str)?;

    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('master_resume', ?1)",
        [content.clone()],
    ).map_err(|e| format!("Failed to save resume: {}", e))?;

    let workspace_root = PathBuf::from(db_path_str).parent().unwrap().to_path_buf();
    let master_resume_path = workspace_root.join("master").join("resume.md");
    fs::write(&master_resume_path, content).map_err(|e| format!("Failed to write master resume: {}", e))?;

    update_setup_step(&conn, "resume")?;

    Ok(true)
}

#[tauri::command]
pub fn extract_claims_from_resume(
    resume_text: String,
    _state: State<'_, AppState>
) -> Result<Vec<ClaimSuggestion>, String> {
    let suggestions = parse_claims_from_resume(&resume_text);
    Ok(suggestions)
}

#[tauri::command]
pub fn save_claims(
    claims: Vec<CreateClaimRequest>,
    state: State<'_, AppState>
) -> Result<usize, String> {
    let db_path_opt = state.db_path.lock().unwrap();
    let db_path_str = db_path_opt.as_ref()
        .ok_or("Database not initialized")?;

    let conn = get_db_connection(db_path_str)?;

    let mut count = 0;
    for claim in claims {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        let inserted = conn.execute(
            "INSERT INTO claims (id, statement, evidence_source, confirmed_wording, scope, category, status, last_confirmed_at)
             SELECT ?1, ?2, ?3, ?4, ?5, ?6, 'confirmed', ?7
             WHERE NOT EXISTS (
                 SELECT 1 FROM claims
                 WHERE statement = ?2 AND evidence_source = ?3 AND confirmed_wording = ?4
                   AND scope = ?5 AND category = ?6 AND status = 'confirmed'
             )",
            [
                id.as_str(),
                claim.statement.as_str(),
                claim.evidence_source.as_str(),
                claim.confirmed_wording.as_str(),
                claim.scope.as_str(),
                claim.category.as_str(),
                now.as_str(),
            ],
        ).map_err(|e| format!("Failed to save claim: {}", e))?;
        count += inserted;
    }

    update_setup_step(&conn, "claims")?;

    Ok(count)
}

#[tauri::command]
pub fn save_presentation_rules(
    rules: Vec<PresentationRuleInput>,
    state: State<'_, AppState>
) -> Result<usize, String> {
    let db_path_opt = state.db_path.lock().unwrap();
    let db_path_str = db_path_opt.as_ref()
        .ok_or("Database not initialized")?;

    let conn = get_db_connection(db_path_str)?;

    let mut count = 0;
    for rule in rules {
        let source = format!("setup-wizard {}", Utc::now().format("%Y-%m-%d"));
        let rule_text = format!(
            "section={},date_policy={},tenure_phrasing={},employer_neutral={}",
            rule.section, rule.date_policy, rule.tenure_phrasing, rule.employer_neutral_metrics
        );

        let existing_id: Option<String> = conn.query_row(
            "SELECT id FROM rules
             WHERE kind = 'presentation' AND subject = ?1 AND applies_to = 'all'
               AND source LIKE 'setup-wizard%'
             ORDER BY rowid DESC LIMIT 1",
            [rule.section.as_str()],
            |row| row.get(0),
        ).optional().map_err(|e| format!("Failed to check existing presentation rule: {}", e))?;

        if let Some(id) = existing_id {
            conn.execute(
                "UPDATE rules SET rule_text = ?1, source = ?2, active = 1 WHERE id = ?3",
                [rule_text.as_str(), source.as_str(), id.as_str()],
            ).map_err(|e| format!("Failed to update presentation rule: {}", e))?;
        } else {
            let id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO rules (id, kind, subject, rule_text, applies_to, source, active)
                 VALUES (?1, 'presentation', ?2, ?3, 'all', ?4, 1)",
                [id.as_str(), rule.section.as_str(), rule_text.as_str(), source.as_str()],
            ).map_err(|e| format!("Failed to save presentation rule: {}", e))?;
        }
        count += 1;
    }

    update_setup_step(&conn, "presentation")?;
    Ok(count)
}

#[tauri::command]
pub fn complete_setup_wizard(
    ollama_models: Vec<String>,
    state: State<'_, AppState>
) -> Result<bool, String> {
    let db_path_opt = state.db_path.lock().unwrap();
    let db_path_str = db_path_opt.as_ref()
        .ok_or("Database not initialized")?;

    let conn = get_db_connection(db_path_str)?;

    let models_json = serde_json::to_string(&ollama_models)
        .map_err(|e| format!("Failed to serialize models: {}", e))?;

    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('selected_ollama_models', ?1)",
        [models_json],
    ).map_err(|e| format!("Failed to save models: {}", e))?;

    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('setup_completed', 'true')",
        [],
    ).map_err(|e| format!("Failed to mark setup complete: {}", e))?;

    update_setup_step(&conn, "ollama")?;

    crate::workspace::append_daily_log(&format!(
        "Setup Wizard completed - profile, master resume and confirmed claims saved; Ollama models: {}",
        ollama_models.join(", ")
    ))?;

    Ok(true)
}

#[tauri::command]
pub fn get_available_ollama_models() -> Result<Vec<OllamaModelInfo>, String> {
    let client = reqwest::blocking::Client::new();
    let response = client
        .get("http://localhost:11434/api/tags")
        .timeout(std::time::Duration::from_secs(5))
        .send();

    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                let json: serde_json::Value = resp.json().map_err(|e| e.to_string())?;
                let models = json["models"].as_array()
                    .map(|arr| arr.iter().filter_map(|m| {
                        m["name"].as_str().map(|n| OllamaModelInfo {
                            name: n.to_string(),
                            size: m["size"].as_u64().unwrap_or(0),
                            modified_at: m["modified_at"].as_str().unwrap_or("").to_string(),
                        })
                    }).collect())
                    .unwrap_or_default();
                Ok(models)
            } else {
                Ok(vec![])
            }
        }
        Err(_) => Ok(vec![]),
    }
}

#[tauri::command]
pub fn pull_ollama_model(model_name: String) -> Result<bool, String> {
    let client = reqwest::blocking::Client::new();
    let response = client
        .post("http://localhost:11434/api/pull")
        .json(&serde_json::json!({ "name": model_name }))
        .timeout(std::time::Duration::from_secs(300))
        .send();

    match response {
        Ok(resp) => Ok(resp.status().is_success()),
        Err(e) => Err(format!("Failed to pull model: {}", e)),
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ClaimSuggestion {
    pub statement: String,
    pub evidence_source: String,
    pub confirmed_wording: String,
    pub scope: String,
    pub category: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OllamaModelInfo {
    pub name: String,
    pub size: u64,
    pub modified_at: String,
}

pub fn parse_claims_from_resume(resume: &str) -> Vec<ClaimSuggestion> {
    let mut suggestions = Vec::new();
    let lines: Vec<&str> = resume.lines().collect();

    for (_i, line) in lines.iter().enumerate() {
        let lower = line.to_lowercase();

        if lower.contains("year") && (lower.contains("experience") || lower.contains("worked") || lower.contains("employed")) {
            suggestions.push(ClaimSuggestion {
                statement: line.trim().to_string(),
                evidence_source: "resume".to_string(),
                confirmed_wording: line.trim().to_string(),
                scope: "tenure".to_string(),
                category: "tenure".to_string(),
            });
        }

        if lower.contains("%") || lower.contains("increase") || lower.contains("decrease") ||
           lower.contains("growth") || lower.contains("revenue") || lower.contains("cost") ||
           lower.contains("save") || lower.contains("reduced") || lower.contains("improved") {
            suggestions.push(ClaimSuggestion {
                statement: line.trim().to_string(),
                evidence_source: "resume".to_string(),
                confirmed_wording: line.trim().to_string(),
                scope: "metric".to_string(),
                category: "metric".to_string(),
            });
        }

        if lower.contains("aws") || lower.contains("azure") || lower.contains("gcp") ||
           lower.contains("kubernetes") || lower.contains("docker") || lower.contains("terraform") ||
           lower.contains("python") || lower.contains("rust") || lower.contains("typescript") ||
           lower.contains("react") || lower.contains("node") || lower.contains("sql") {
            suggestions.push(ClaimSuggestion {
                statement: line.trim().to_string(),
                evidence_source: "resume".to_string(),
                confirmed_wording: line.trim().to_string(),
                scope: "tool".to_string(),
                category: "tool".to_string(),
            });
        }
    }

    if suggestions.is_empty() {
        suggestions.push(ClaimSuggestion {
            statement: "Professional experience in software development".to_string(),
            evidence_source: "resume".to_string(),
            confirmed_wording: "Professional experience in software development".to_string(),
            scope: "general".to_string(),
            category: "tenure".to_string(),
        });
    }

    suggestions
}

pub fn update_setup_step(conn: &rusqlite::Connection, step_id: &str) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, 'true')",
        [format!("setup_step_{}", step_id)],
    ).map_err(|e| format!("Failed to update setup step: {}", e))?;
    Ok(())
}

fn setup_step_is_complete(conn: &rusqlite::Connection, step_id: &str) -> Result<bool, String> {
    let value: Option<String> = conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        [format!("setup_step_{}", step_id)],
        |row| row.get(0),
    ).optional().map_err(|e| format!("Failed to read setup step '{}': {}", step_id, e))?;
    Ok(value.as_deref() == Some("true"))
}

fn parse_presentation_rule(rule_text: &str) -> Result<PresentationRuleInput, String> {
    let values: std::collections::HashMap<&str, &str> = rule_text
        .split(',')
        .filter_map(|part| part.split_once('='))
        .collect();
    let required = |key: &str| {
        values.get(key).copied()
            .ok_or_else(|| format!("Presentation preference is missing '{}'", key))
    };
    let employer_neutral_metrics = required("employer_neutral")?
        .parse::<bool>()
        .map_err(|e| format!("Invalid employer-neutral preference: {}", e))?;

    Ok(PresentationRuleInput {
        section: required("section")?.to_string(),
        date_policy: required("date_policy")?.to_string(),
        tenure_phrasing: required("tenure_phrasing")?.to_string(),
        employer_neutral_metrics,
    })
}