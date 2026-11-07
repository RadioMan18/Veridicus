//! Gap Analyzer Engine for CareerOS.
//!
//! Analyzes job descriptions against the Claims Ledger to identify:
//! - Supported qualifications (matched to confirmed claims)
//! - Identified gaps (unmatched requirements with tiers and adjacent strengths)
//! - Ask-first triggers (unconfirmed subjects)
//! - Ambiguous requirements needing clarification

use std::collections::HashSet;
use serde::{Deserialize, Serialize};
use tauri::State;
use crate::AppState;
use crate::db::get_db_connection;
use crate::ollama::{chat_with_ollama, OllamaChatMessage};

/// Represents the tier of a job requirement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RequirementTier {
    #[serde(rename = "must_have")]
    MustHave,
    #[serde(rename = "nice_to_have")]
    NiceToHave,
    #[serde(rename = "gate")]
    Gate,
}

/// A gap entry representing a requirement not fully covered by the claims ledger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GapEntry {
    pub requirement: String,
    pub requirement_tier: RequirementTier,
    pub matched_claim_id: Option<String>,
    pub adjacent_strength: String,
    pub proposed_letter_line: String,
}

/// The result of analyzing job description gaps against the claims ledger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub supported_claim_ids: Vec<String>,
    pub identified_gaps: Vec<GapEntry>,
    pub ask_first_triggered: Vec<String>,
    pub ambiguous_requirements: Vec<String>,
}

/// Prompt response from the LLM for requirement extraction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequirementResponse {
    pub requirements: Vec<LlmRequirementItem>,
}

/// A single requirement item from the LLM response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequirementItem {
    pub description: String,
    #[serde(rename = "tier")]
    pub tier: String,
    pub rationale: Option<String>,
}

/// Parse a requirement tier string into the enum.
fn parse_tier(tier_str: &str) -> RequirementTier {
    match tier_str.to_lowercase().trim() {
        "must_have" | "must" => RequirementTier::MustHave,
        "nice_to_have" | "nice" => RequirementTier::NiceToHave,
        "gate" => RequirementTier::Gate,
        _ => RequirementTier::MustHave,
    }
}

/// Format tier for serialization.

/// Extract requirements from job description text using the Ollama fast model.
fn extract_requirements_with_llm(jd_text: &str, fast_model: &str) -> Result<Vec<LlmRequirementItem>, String> {
    let system = "You are a job description analyzer. Extract all requirements from the job description below. For each requirement, assign a tier: must_have (critical, must have to be considered), nice_to_have (preferred but not deal-breaker), or gate (cannot proceed without). Output as JSON with a requirements array, each item having: description (the requirement text), tier (must_have|nice_to_have|gate), and optional rationale.";

    let user_prompt = format!("Extract requirements from this job description:\n\n{}", jd_text);

    let messages = vec![
        OllamaChatMessage {
            role: "system".to_string(),
            content: system.to_string(),
        },
        OllamaChatMessage {
            role: "user".to_string(),
            content: user_prompt,
        },
    ];

    let response = chat_with_ollama(
        fast_model.to_string(),
        messages,
        Some(0.3),
        Some(800),
        None,
    ).map_err(|e| format!("LLM chat failed: {}", e))?;

    let content = response.message.content;

    // Try to parse JSON array from the response
    let result: LlmRequirementResponse = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse LLM requirement response: {} - {}", e, content))?;

    Ok(result.requirements)
}

/// Match a requirement description against confirmed claims to find the best match.
fn match_against_claims(requirement: &str, claims: &[crate::claims::Claim]) -> (Option<String>, String) {
    let requirement_lower = requirement.to_lowercase();
    let req_words: HashSet<&str> = requirement_lower.split_whitespace().collect();

    for claim in claims {
        let claim_lower = claim.statement.to_lowercase();

        // Check if claim statement contains most requirement keywords
        let overlap_count = req_words.iter().filter(|w| claim_lower.contains(*w)).count();
        let min_overlap = req_words.len().max(1) / 3; // At least 1/3 of words must overlap

        if overlap_count >= min_overlap {
            let adjacent = format!(
                "While I have not directly stated '{}', my experience in related areas enables me to address this need.",
                requirement
            );
            return (Some(claim.id.clone()), adjacent);
        }
    }

    // No matching claim found
    let adjacent = format!(
        "No direct claim found in ledger for: {}. This would need to be addressed through experience or certification.",
        requirement
    );
    (None, adjacent)
}

/// Analyze job description gaps against the claims ledger.
#[tauri::command]
pub async fn analyze_job_gaps(
    jd_text: String,
    _application_id: String,
    state: State<'_, AppState>,
) -> Result<AnalysisResult, String> {
    // _application_id not used in this version - passed for future use
    // Extract db path from state, releasing the lock
    let db_path_str = {
        let db_path_opt = state.db_path.lock().unwrap();
        db_path_opt.as_ref()
            .ok_or("Database not initialized")?
            .clone()
    };

    let conn = get_db_connection(&db_path_str)?;

    // 1. Fetch confirmed claims from SQLCipher SQLite database
    // Use query_map with a mutable stmt locally scoped
    let claims: Vec<crate::claims::Claim> = {
        let mut stmt = conn.prepare("SELECT id, statement, scope, category, status FROM claims WHERE status = 'confirmed'")
            .map_err(|e| format!("Failed to prepare query: {}", e))?;
        let rows = stmt.query_map([], |row| {
            Ok(crate::claims::Claim {
                id: row.get(0)?,
                statement: row.get(1)?,
                scope: row.get(2)?,
                category: row.get(3)?,
                status: row.get(4)?,
                evidence_source: String::new(),
                confirmed_wording: String::new(),
                last_confirmed_at: String::new(),
                evidence_type: "unspecified".to_string(),
                evidence_sha256: String::new(),
                verified_by: String::new(),
            })
        }).map_err(|e| format!("Failed to query claims: {}", e))?
            .filter_map(|r| r.ok())
            .collect();
        drop(stmt);
        rows
    };

    // 2. Get Ollama config for fast model assignment
    let ollama_config = crate::ollama::get_ollama_config(state);
    let fast_model = ollama_config.unwrap_or_default().fast_model
        .unwrap_or_else(|| "llama3.2:3b".to_string());

    // 3. Extract requirements from JD using LLM
    let llm_requirements = extract_requirements_with_llm(&jd_text, &fast_model)
        .unwrap_or_else(|_| {
            extract_requirements_fallback(&jd_text)
        });

    // 4. Analyze each requirement against the claims ledger
    let mut supported_claim_ids: Vec<String> = Vec::new();
    let mut identified_gaps: Vec<GapEntry> = Vec::new();
    let mut ask_first_triggered: Vec<String> = Vec::new();
    let mut ambiguous_requirements: Vec<String> = Vec::new();

    let mut seen_claim_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    for req_item in &llm_requirements {
        let tier = parse_tier(&req_item.tier);
        let requirement = &req_item.description;

        // Skip very short or very long requirements
        if requirement.trim().len() < 5 {
            continue;
        }
        if requirement.trim().len() > 500 {
            ambiguous_requirements.push(requirement.clone());
            continue;
        }

        // Match against claims
        let (matched_claim_id, adjacent_strength) = match_against_claims(&requirement, &claims);
        let is_must_have = tier == RequirementTier::MustHave;

        if let Some(claim_id) = &matched_claim_id {
            // Only add each claim ID once to the supported list
            if seen_claim_ids.insert(claim_id.clone()) {
                supported_claim_ids.push(claim_id.clone());
            }
        } else {
            // No matching claim - this is a gap
            let letter_line = generate_letter_line(&requirement);

            identified_gaps.push(GapEntry {
                requirement: requirement.clone(),
                requirement_tier: tier,
                matched_claim_id: matched_claim_id.clone(),
                adjacent_strength,
                proposed_letter_line: letter_line,
            });
        }

        // Check for ask_first triggers: must-have requirements without a matching claim
        if is_must_have && matched_claim_id.is_none() {
            ask_first_triggered.push(requirement.clone());
        }
    }

    Ok(AnalysisResult {
        supported_claim_ids,
        identified_gaps,
        ask_first_triggered,
        ambiguous_requirements,
    })
}

/// Fallback requirement extraction using simple heuristics when LLM is unavailable.
fn extract_requirements_fallback(jd_text: &str) -> Vec<LlmRequirementItem> {
    let mut requirements = Vec::new();
    let lines: Vec<&str> = jd_text.lines().collect();

    for line in lines {
        let lower = line.to_lowercase();
        // Look for lines that look like requirements (have action verbs, specific nouns)
        if lower.contains("experience")
            || lower.contains("skill")
            || lower.contains("knowledge")
            || lower.contains("ability")
            || lower.contains("certification")
            || lower.contains("familiarity")
        {
            // Only include if the line has substantial content
            if line.trim().len() > 10 && line.trim().len() < 200 {
                requirements.push(LlmRequirementItem {
                    description: line.trim().to_string(),
                    tier: "must_have".to_string(),
                    rationale: None,
                });
            }
        }
    }

    if requirements.is_empty() {
        requirements.push(LlmRequirementItem {
            description: "Professional experience in software development".to_string(),
            tier: "must_have".to_string(),
            rationale: None,
        });
    }

    requirements
}

/// Generate a plain-language letter line explaining the adjacent strength for a gap.
fn generate_letter_line(requirement: &str) -> String {
    format!(
        "While I have not directly stated '{}', my experience in related areas enables me to address this need.",
        requirement
    )
}