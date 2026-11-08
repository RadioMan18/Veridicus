//! Gap Analyzer Engine for CareerOS.
//!
//! Analyzes job descriptions against the Claims Ledger to identify:
//! - Supported qualifications (matched to confirmed claims)
//! - Identified gaps (unmatched requirements with tiers, adjacent strengths and
//!   the fit-matrix fields the review screen renders)
//! - Ask-first triggers (unconfirmed subjects)
//! - Ambiguous requirements needing clarification
//! - Posting keyword coverage (shared with the ATS report via `keywords.rs`)

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tauri::State;

use crate::db::get_db_connection;
use crate::guardrails::fetch_confirmed_claims;
use crate::keywords::{self, KeywordCoverage};
use crate::ollama::{chat_with_ollama_async, OllamaChatMessage};
use crate::AppState;

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

/// A gap entry representing a requirement not covered by the claims ledger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GapEntry {
    pub requirement: String,
    pub requirement_tier: RequirementTier,
    pub matched_claim_id: Option<String>,
    pub adjacent_strength: String,
    pub proposed_letter_line: String,
    /// What the CV should show for this requirement.
    #[serde(default)]
    pub cv_focus: String,
    /// What the cover letter should say about it.
    #[serde(default)]
    pub cover_letter_point: String,
    /// Filled in Stage 3 (STAR bank). Empty until then, rendered as "no story yet".
    #[serde(default)]
    pub story_ids: Vec<String>,
}

/// The result of analyzing job description gaps against the claims ledger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub supported_claim_ids: Vec<String>,
    pub identified_gaps: Vec<GapEntry>,
    pub ask_first_triggered: Vec<String>,
    pub ambiguous_requirements: Vec<String>,
    /// What the posting is trying to achieve, in the model's words.
    #[serde(default)]
    pub hiring_goals: Vec<String>,
    /// What the posting says distinguishes a strong candidate.
    #[serde(default)]
    pub standout_traits: Vec<String>,
    /// Posting terms and whether the resume/claims cover them.
    #[serde(default)]
    pub ats_keywords: Vec<KeywordCoverage>,
}

/// Prompt response from the LLM for requirement extraction.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LlmRequirementResponse {
    #[serde(default)]
    pub requirements: Vec<LlmRequirementItem>,
    #[serde(default)]
    pub hiring_goals: Vec<String>,
    #[serde(default)]
    pub standout_traits: Vec<String>,
}

/// A single requirement item from the LLM response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequirementItem {
    pub description: String,
    #[serde(rename = "tier")]
    pub tier: String,
    #[serde(default)]
    pub rationale: Option<String>,
    #[serde(default)]
    pub cv_focus: String,
    #[serde(default)]
    pub cover_letter_point: String,
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

/// Minimum share of a requirement's significant words that one claim must cover.
///
/// The old rule accepted a third of the raw tokens, which reported unrelated
/// claims as support: "design scalable onboarding programs" matched a claim
/// mentioning only "design". That is a false strength in a product whose value
/// is not overstating support.
const MATCH_THRESHOLD: f64 = 0.6;

/// Extract requirements, hiring goals and standout traits in one model call.
async fn extract_requirements_with_llm(
    jd_text: &str,
    fast_model: &str,
    num_ctx: Option<u32>,
) -> Result<LlmRequirementResponse, String> {
    let system = "You are a job description analyzer. Read the posting below. Return JSON with: \
requirements (array of objects with description, tier (must_have|nice_to_have|gate), optional rationale, \
cv_focus (one line saying what evidence the CV should show for it), cover_letter_point (one line the cover \
letter can use about it)), hiring_goals (array of strings: what the employer is trying to achieve with this \
hire), and standout_traits (array of strings: what the posting says separates a strong candidate). Ground \
every item in the posting; do not add requirements it does not imply.";

    let user_prompt = format!("Analyze this job description:\n\n{}", jd_text);

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

    let response = chat_with_ollama_async(
        fast_model.to_string(),
        messages,
        Some(0.3),
        Some(1200),
        Some("json".to_string()),
        num_ctx,
    )
    .await
    .map_err(|e| format!("LLM chat failed: {}", e))?;

    serde_json::from_str(&response.message.content)
        .map_err(|e| format!("Failed to parse LLM requirement response: {} - {}", e, response.message.content))
}

/// Match one requirement against confirmed claims, best match first.
///
/// Matching uses the confirmed wording (the statement is a fallback for rows
/// written before wording was required) and whole-word comparison.
pub(crate) fn match_against_claims(
    requirement: &str,
    claims: &[crate::claims::Claim],
) -> (Option<String>, String) {
    let requirement_words = keywords::significant_words(requirement);
    let requirement_set: HashSet<&String> = requirement_words.iter().collect();

    let required_overlap = if requirement_words.len() <= 2 {
        requirement_words.len()
    } else {
        ((requirement_words.len() as f64) * MATCH_THRESHOLD).ceil() as usize
    };

    let mut best: Option<(usize, &crate::claims::Claim)> = None;
    if !requirement_words.is_empty() {
        for claim in claims {
            let claim_words: HashSet<String> = keywords::significant_words(&keywords::claim_text_of(claim))
                .into_iter()
                .collect();
            let overlap = requirement_set
                .iter()
                .filter(|word| claim_words.contains(**word))
                .count();
            if overlap >= required_overlap && best.map(|(count, _)| overlap > count).unwrap_or(true) {
                best = Some((overlap, claim));
            }
        }
    }

    match best {
        Some((_, claim)) => (
            Some(claim.id.clone()),
            format!(
                "While I have not directly stated '{}', my work in related areas enables me to address this need.",
                requirement
            ),
        ),
        None => (
            None,
            format!(
                "No direct claim found in ledger for: {}. This would need to be addressed through experience or certification.",
                requirement
            ),
        ),
    }
}

/// Turn a parsed model response into an analysis. Pure, so the honesty rules are
/// testable without a model.
pub(crate) fn assemble_analysis(
    response: &LlmRequirementResponse,
    claims: &[crate::claims::Claim],
    jd_text: &str,
    resume_text: &str,
) -> AnalysisResult {
    let mut supported_claim_ids: Vec<String> = Vec::new();
    let mut identified_gaps: Vec<GapEntry> = Vec::new();
    let mut ask_first_triggered: Vec<String> = Vec::new();
    let mut ambiguous_requirements: Vec<String> = Vec::new();
    let mut seen_claim_ids: HashSet<String> = HashSet::new();

    for item in &response.requirements {
        let tier = parse_tier(&item.tier);
        let requirement = item.description.trim();

        if requirement.len() < 5 {
            continue;
        }
        if requirement.len() > 500 {
            ambiguous_requirements.push(requirement.to_string());
            continue;
        }

        let (matched_claim_id, adjacent_strength) = match_against_claims(requirement, claims);
        let is_must_have = tier == RequirementTier::MustHave;
        let is_gap = matched_claim_id.is_none();

        if let Some(claim_id) = matched_claim_id.as_ref() {
            if seen_claim_ids.insert(claim_id.clone()) {
                supported_claim_ids.push(claim_id.clone());
            }
        } else {
            let cv_focus = if item.cv_focus.trim().is_empty() {
                format!("Show the closest evidence you have for: {}", requirement)
            } else {
                item.cv_focus.trim().to_string()
            };
            let cover_letter_point = if item.cover_letter_point.trim().is_empty() {
                format!("Name this gap plainly and offer the adjacent strength: {}", requirement)
            } else {
                item.cover_letter_point.trim().to_string()
            };

            identified_gaps.push(GapEntry {
                requirement: requirement.to_string(),
                requirement_tier: tier,
                matched_claim_id,
                adjacent_strength,
                proposed_letter_line: generate_letter_line(requirement),
                cv_focus,
                cover_letter_point,
                story_ids: Vec::new(),
            });
        }

        if is_must_have && is_gap {
            ask_first_triggered.push(requirement.to_string());
        }
    }

    AnalysisResult {
        supported_claim_ids,
        identified_gaps,
        ask_first_triggered,
        ambiguous_requirements,
        hiring_goals: clean_list(&response.hiring_goals),
        standout_traits: clean_list(&response.standout_traits),
        ats_keywords: keywords::coverage(jd_text, resume_text, claims),
    }
}

fn clean_list(values: &[String]) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    values
        .iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .filter(|value| seen.insert(value.to_lowercase()))
        .take(8)
        .collect()
}

/// Analyze job description gaps against the claims ledger.
#[tauri::command]
pub async fn analyze_job_gaps(
    jd_text: String,
    _application_id: String,
    state: State<'_, AppState>,
) -> Result<AnalysisResult, String> {
    analyze_text(&jd_text, &state).await
}

/// Analyze a pasted posting without opening an application.
///
/// The same code path as `/apply`; persistence only happens when the caller
/// supplies an application id.
#[tauri::command]
pub async fn analyze_jd_brief(
    jd_text: String,
    application_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<AnalysisResult, String> {
    let analysis = analyze_text(&jd_text, &state).await?;

    if let Some(application_id) = application_id.filter(|id| !id.trim().is_empty()) {
        let db_path_str = {
            let db_path_opt = state.db_path.lock().unwrap();
            db_path_opt
                .as_ref()
                .ok_or("Database not initialized")?
                .clone()
        };
        let conn = get_db_connection(&db_path_str)?;
        crate::guardrails::persist_gaps(&conn, &application_id, &analysis)?;
    }

    Ok(analysis)
}

async fn analyze_text(jd_text: &str, state: &State<'_, AppState>) -> Result<AnalysisResult, String> {
    let db_path_str = {
        let db_path_opt = state.db_path.lock().unwrap();
        db_path_opt
            .as_ref()
            .ok_or("Database not initialized")?
            .clone()
    };

    let conn = get_db_connection(&db_path_str)?;
    let claims = fetch_confirmed_claims(&conn)?;
    let resume_text: String = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'master_resume'",
            [],
            |row| row.get(0),
        )
        .unwrap_or_default();

    let ollama_config = crate::ollama::get_ollama_config(state.clone()).unwrap_or_default();
    let fast_model = ollama_config
        .fast_model
        .clone()
        .or_else(|| ollama_config.deep_model.clone())
        .unwrap_or_else(|| "llama3.2:3b".to_string());

    let response = match extract_requirements_with_llm(
        jd_text,
        &fast_model,
        ollama_config.num_ctx,
    )
    .await
    {
        Ok(response) => response,
        Err(_) => LlmRequirementResponse {
            requirements: extract_requirements_fallback(jd_text),
            hiring_goals: Vec::new(),
            standout_traits: Vec::new(),
        },
    };

    Ok(assemble_analysis(&response, &claims, jd_text, &resume_text))
}

/// Fallback requirement extraction using simple heuristics when the model is unavailable.
fn extract_requirements_fallback(jd_text: &str) -> Vec<LlmRequirementItem> {
    let mut requirements = Vec::new();

    for line in jd_text.lines() {
        let lower = line.to_lowercase();
        if lower.contains("experience")
            || lower.contains("skill")
            || lower.contains("knowledge")
            || lower.contains("ability")
            || lower.contains("certification")
            || lower.contains("familiarity")
        {
            if line.trim().len() > 10 && line.trim().len() < 200 {
                requirements.push(LlmRequirementItem {
                    description: line.trim().to_string(),
                    tier: "must_have".to_string(),
                    rationale: None,
                    cv_focus: String::new(),
                    cover_letter_point: String::new(),
                });
            }
        }
    }

    if requirements.is_empty() {
        requirements.push(LlmRequirementItem {
            description: "Professional experience in software development".to_string(),
            tier: "must_have".to_string(),
            rationale: None,
            cv_focus: String::new(),
            cover_letter_point: String::new(),
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

#[cfg(test)]
mod tests {
    use super::{assemble_analysis, LlmRequirementItem, LlmRequirementResponse};
    use crate::claims::Claim;

    fn claim(wording: &str) -> Claim {
        Claim {
            id: "claim-1".to_string(),
            statement: wording.to_string(),
            evidence_source: "master/resume.md".to_string(),
            confirmed_wording: wording.to_string(),
            scope: "as stated".to_string(),
            category: "project".to_string(),
            status: "confirmed".to_string(),
            last_confirmed_at: "2026-11-08".to_string(),
            evidence_type: "unspecified".to_string(),
            evidence_sha256: String::new(),
            verified_by: "ledger-repair".to_string(),
        }
    }

    fn item(description: &str, tier: &str) -> LlmRequirementItem {
        LlmRequirementItem {
            description: description.to_string(),
            tier: tier.to_string(),
            rationale: None,
            cv_focus: String::new(),
            cover_letter_point: String::new(),
        }
    }

    fn response(requirements: Vec<LlmRequirementItem>) -> LlmRequirementResponse {
        LlmRequirementResponse {
            requirements,
            hiring_goals: vec!["Grow the learning function".to_string()],
            standout_traits: vec!["Bias to practical impact".to_string()],
        }
    }

    #[test]
    fn one_third_token_overlap_is_not_supported() {
        // Six significant requirement words, a claim covering two of them: the
        // old 1/3 rule accepted this as support.
        let requirements = response(vec![item(
            "Design scalable onboarding programs for global leaders",
            "must_have",
        )]);
        let claims = vec![claim("Design programs")];

        let analysis = assemble_analysis(&requirements, &claims, "", "");

        assert!(analysis.supported_claim_ids.is_empty());
        assert_eq!(analysis.identified_gaps.len(), 1);
        assert_eq!(analysis.ask_first_triggered.len(), 1);
    }

    #[test]
    fn a_strength_cannot_exist_without_a_matched_claim() {
        let requirements = response(vec![
            item("Own the learning technology roadmap", "must_have"),
            item("Maintain the vendor budget", "nice_to_have"),
        ]);
        let claims = vec![claim("Owned a learning platform migration")];

        let analysis = assemble_analysis(&requirements, &claims, "", "");
        let known: Vec<String> = claims.iter().map(|claim| claim.id.clone()).collect();

        for id in &analysis.supported_claim_ids {
            assert!(known.contains(id), "supported id {} has no claim", id);
        }
        assert!(analysis
            .identified_gaps
            .iter()
            .all(|gap| gap.matched_claim_id.is_none()));
    }

    #[test]
    fn gaps_carry_matrix_fields_and_goals_are_kept() {
        let requirements = response(vec![item("Own the learning technology roadmap", "must_have")]);
        let analysis = assemble_analysis(&requirements, &[], "python learning", "learning design");

        assert_eq!(analysis.hiring_goals, vec!["Grow the learning function"]);
        assert_eq!(analysis.standout_traits, vec!["Bias to practical impact"]);
        assert!(!analysis.identified_gaps[0].cv_focus.is_empty());
        assert!(!analysis.identified_gaps[0].cover_letter_point.is_empty());
        assert!(analysis.identified_gaps[0].story_ids.is_empty());
        assert!(analysis
            .ats_keywords
            .iter()
            .any(|entry| entry.term == "python" && !entry.in_resume));
    }
}
