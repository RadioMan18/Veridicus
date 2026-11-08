//! Recruiter-style review (prompt 9).
//!
//! The deterministic evidence set is built before the model runs, and the model
//! may only cite those items. A verdict with fewer than two citations, an
//! improvement that names nothing concrete, or a number the ledger does not
//! contain is rejected and asked for once more.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use tauri::State;

use crate::claims::Claim;
use crate::db::get_db_connection;
use crate::fact_check::verify_metrics;
use crate::guardrails::{cited_claim_ids, fetch_confirmed_claims};
use crate::ollama::{chat_with_ollama_async, OllamaChatMessage};
use crate::AppState;

const MAX_DOCUMENT_BYTES: usize = 12_000;
const MAX_JD_BYTES: usize = 8_000;
const MAX_EVIDENCE_CLAIMS: usize = 6;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewEvidence {
    pub id: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewPoint {
    pub text: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Improvement {
    pub text: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecruiterReview {
    pub verdict: String,
    pub employer: String,
    pub role: String,
    pub generated_at: String,
    pub model: Option<String>,
    pub attempts: u32,
    pub evidence: Vec<ReviewEvidence>,
    pub strengths: Vec<ReviewPoint>,
    pub weaknesses: Vec<ReviewPoint>,
    pub improvements: Vec<Improvement>,
    pub markdown: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct ReviewDraft {
    #[serde(default)]
    verdict: String,
    #[serde(default)]
    strengths: Vec<PointDraft>,
    #[serde(default)]
    weaknesses: Vec<PointDraft>,
    #[serde(default)]
    improvements: Vec<ImprovementDraft>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct PointDraft {
    #[serde(default)]
    text: String,
    #[serde(default)]
    evidence: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ImprovementDraft {
    #[serde(default)]
    text: String,
    #[serde(default)]
    target: String,
}

/// The evidence a verdict may cite, and the ids an improvement may target.
pub(crate) struct ReviewContext {
    pub evidence: Vec<ReviewEvidence>,
    pub evidence_ids: HashSet<String>,
    pub claim_ids: HashSet<String>,
    pub gap_ids: HashSet<String>,
    pub marker_ids: HashSet<String>,
}

/// Models sometimes wrap JSON in a code fence. Parse the object itself.
fn json_body(text: &str) -> &str {
    let start = text.find('{');
    let end = text.rfind('}');
    match (start, end) {
        (Some(start), Some(end)) if end > start => &text[start..=end],
        _ => text.trim(),
    }
}

fn truncate(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

fn load_gaps(conn: &rusqlite::Connection, application_id: &str) -> Vec<(String, String)> {
    let Ok(mut statement) = conn.prepare(
        "SELECT id, requirement FROM gaps WHERE application_id = ?1 ORDER BY requirement",
    ) else {
        return Vec::new();
    };
    let Ok(rows) = statement.query_map([application_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) else {
        return Vec::new();
    };
    rows.flatten().collect()
}

fn missing_keywords(application_id: &str) -> Vec<String> {
    let Ok(docs) = dirs::document_dir().ok_or(()) else {
        return Vec::new();
    };
    let path = docs
        .join("CareerOS")
        .join("applications")
        .join(application_id)
        .join("fit-matrix.json");
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    value["ats_keywords"]
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .filter(|entry| {
                    !entry["in_resume"].as_bool().unwrap_or(true)
                        && !entry["in_claims"].as_bool().unwrap_or(true)
                })
                .filter_map(|entry| entry["term"].as_str().map(str::to_string))
                .take(8)
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn build_context(
    conn: &rusqlite::Connection,
    application_id: &str,
    claims: &[Claim],
) -> ReviewContext {
    let mut evidence = Vec::new();
    let mut claim_ids = HashSet::new();
    let mut gap_ids = HashSet::new();
    let mut marker_ids = HashSet::new();

    for claim in claims.iter().take(MAX_EVIDENCE_CLAIMS) {
        let id = format!("claim:{}", claim.id);
        claim_ids.insert(claim.id.clone());
        evidence.push(ReviewEvidence {
            id,
            summary: truncate(claim.confirmed_wording.trim(), 180),
        });
    }

    for (gap_id, requirement) in load_gaps(conn, application_id) {
        gap_ids.insert(gap_id.clone());
        evidence.push(ReviewEvidence {
            id: format!("gap:{}", gap_id),
            summary: truncate(&requirement, 180),
        });
    }

    if let Ok(markers) = crate::skills::open_markers(conn) {
        for marker in markers {
            marker_ids.insert(marker.claim_id.clone());
            evidence.push(ReviewEvidence {
                id: format!("marker:{}", marker.claim_id),
                summary: format!(
                    "{} on \"{}\"",
                    marker.markers.join(", "),
                    truncate(&marker.original, 120)
                ),
            });
        }
    }

    let missing = missing_keywords(application_id);
    if !missing.is_empty() {
        evidence.push(ReviewEvidence {
            id: "ats:missing".to_string(),
            summary: format!("posting terms the materials do not cover: {}", missing.join(", ")),
        });
    }

    let evidence_ids = evidence.iter().map(|item| item.id.clone()).collect();
    ReviewContext {
        evidence,
        evidence_ids,
        claim_ids,
        gap_ids,
        marker_ids,
    }
}

fn text_of(draft: &ReviewDraft) -> String {
    let points = draft
        .strengths
        .iter()
        .chain(draft.weaknesses.iter())
        .map(|point| point.text.as_str());
    let improvements = draft.improvements.iter().map(|entry| entry.text.as_str());
    points.chain(improvements).collect::<Vec<&str>>().join("\n")
}

/// Accept a bare id the model copied from an evidence summary; it still has to
/// match an item in the evidence set, so nothing new can be cited.
fn resolve_id(id: &str, ctx: &ReviewContext) -> Option<String> {
    let id = id.trim();
    if ctx.evidence_ids.contains(id) {
        return Some(id.to_string());
    }
    for prefix in ["claim", "gap", "marker"] {
        let candidate = format!("{}:{}", prefix, id);
        if ctx.evidence_ids.contains(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn normalize_draft(draft: &mut ReviewDraft, ctx: &ReviewContext) {
    for point in draft.strengths.iter_mut().chain(draft.weaknesses.iter_mut()) {
        point.evidence = point
            .evidence
            .iter()
            .filter_map(|id| resolve_id(id, ctx))
            .collect();
    }
    for improvement in draft.improvements.iter_mut() {
        if improvement.target.contains(':') {
            continue;
        }
        if let Some(resolved) = resolve_id(&improvement.target, ctx) {
            improvement.target = resolved;
        }
    }
}

fn validate_target(target: &str, ctx: &ReviewContext) -> bool {
    let Some((kind, id)) = target.split_once(':') else {
        return false;
    };
    match kind {
        "claim" => ctx.claim_ids.contains(id),
        "gap" => ctx.gap_ids.contains(id),
        "marker" => ctx.marker_ids.contains(id),
        _ => false,
    }
}

/// A verdict is only accepted with two cited evidence items; every improvement
/// must name a claim, a gap or a marker; no number may be new.
pub(crate) fn validate_draft(
    draft: &ReviewDraft,
    ctx: &ReviewContext,
    claims: &[Claim],
) -> Result<(), String> {
    if !matches!(draft.verdict.as_str(), "shortlist" | "maybe" | "reject") {
        return Err(format!(
            "verdict '{}' is not one of shortlist, maybe or reject",
            draft.verdict
        ));
    }
    if draft.strengths.is_empty() {
        return Err("the review gives no strengths".to_string());
    }
    if draft.weaknesses.is_empty() {
        return Err("the review gives no weaknesses".to_string());
    }
    if draft.improvements.is_empty() {
        return Err("the review gives no improvements".to_string());
    }
    if draft.strengths.len() > 3 || draft.weaknesses.len() > 3 || draft.improvements.len() > 3 {
        return Err(
            "the review lists more than three strengths, weaknesses or improvements; keep it to two of each"
                .to_string(),
        );
    }
    for point in &draft.strengths {
        for id in &point.evidence {
            if id.starts_with("gap:") || id.starts_with("marker:") {
                return Err(format!(
                    "'{}' is listed as a strength but cites {}",
                    truncate(&point.text, 80),
                    id
                ));
            }
        }
    }

    let mut cited: HashSet<String> = HashSet::new();
    for point in draft.strengths.iter().chain(draft.weaknesses.iter()) {
        if point.evidence.is_empty() {
            return Err(format!("'{}' cites no evidence", truncate(&point.text, 80)));
        }
        for id in &point.evidence {
            if !ctx.evidence_ids.contains(id) {
                return Err(format!("'{}' cites unknown evidence {}", point.text, id));
            }
            cited.insert(id.clone());
        }
    }
    if cited.len() < 2 {
        return Err(format!(
            "the verdict cites {} evidence item(s); at least two are required",
            cited.len()
        ));
    }

    for improvement in &draft.improvements {
        if improvement.text.trim().is_empty() {
            return Err("an improvement is empty".to_string());
        }
        if !validate_target(&improvement.target, ctx) {
            return Err(format!(
                "'{}' does not name a claim, gap or marker ({})",
                improvement.text, improvement.target
            ));
        }
    }

    let metrics = verify_metrics(&text_of(draft), claims);
    if !metrics.unverified.is_empty() {
        return Err(format!(
            "the review states number(s) no claim supports: {}",
            metrics.unverified.join(", ")
        ));
    }
    if !metrics.qualifier_violations.is_empty() {
        return Err(format!(
            "the review changes a confirmed qualifier: {}",
            metrics.qualifier_violations.join("; ")
        ));
    }

    Ok(())
}

fn prompt(
    role: &str,
    jd: &str,
    resume: &str,
    letter: &str,
    ctx: &ReviewContext,
) -> String {
    let mut prompt = format!(
        "## Role
{}

## Posting
{}

## Tailored CV
{}

## Cover letter
{}

## Evidence you may cite
",
        role,
        truncate(jd, MAX_JD_BYTES),
        truncate(resume, MAX_DOCUMENT_BYTES),
        truncate(letter, MAX_DOCUMENT_BYTES)
    );
    for item in &ctx.evidence {
        prompt.push_str(&format!("- {} — {}
", item.id, item.summary));
    }
    prompt.push_str(
        r#"
Return only JSON: {"verdict":"<verdict>","strengths":[{"text":"...","evidence":["<evidence id>"]}],"weaknesses":[...],"improvements":[{"text":"...","target":"<claim, gap or marker id>"}]}.
The verdict value must be exactly one word: shortlist, maybe, or reject. Do not repeat this instruction and do not use a pipe character.
Give exactly two strengths, two weaknesses and two improvements, each one sentence. Do not list more than two of each. Strengths may cite claim ids only; gap and marker ids describe what is missing and belong in weaknesses. Every improvement must name a claim, gap or marker id from the list; general advice is rejected.
Do not add any number that is not in the evidence text. Judge only against the evidence: do not invent company culture, team size or hiring process."#,
    );
    prompt
}

fn render_markdown(review: &RecruiterReview) -> String {
    let mut md = format!(
        "# Recruiter review — {} — {}\n\n**Verdict: {}**\n\n",
        review.employer, review.role, review.verdict
    );
    md.push_str("## Strengths\n\n");
    for point in &review.strengths {
        md.push_str(&format!(
            "- {}\n  - Evidence: {}\n",
            point.text,
            point.evidence.join(", ")
        ));
    }
    md.push_str("\n## Weaknesses\n\n");
    for point in &review.weaknesses {
        md.push_str(&format!(
            "- {}\n  - Evidence: {}\n",
            point.text,
            point.evidence.join(", ")
        ));
    }
    md.push_str("\n## Quick improvements\n\n");
    for improvement in &review.improvements {
        md.push_str(&format!(
            "- {} ({})\n",
            improvement.text, improvement.target
        ));
    }
    md.push_str("\n## Evidence\n\n");
    for item in &review.evidence {
        md.push_str(&format!("- {} — {}\n", item.id, item.summary));
    }
    md
}

pub(crate) async fn review_documents(
    employer: &str,
    role: &str,
    jd: &str,
    resume: &str,
    letter: &str,
    claims: &[Claim],
    ctx: &ReviewContext,
    model: &str,
    num_ctx: Option<u32>,
) -> Result<RecruiterReview, String> {
    let system = "You are a recruiter reviewing application materials against a fixed evidence set. \
Never invent facts, numbers or company details. Return JSON only.";
    let mut messages = vec![
        OllamaChatMessage {
            role: "system".to_string(),
            content: system.to_string(),
        },
        OllamaChatMessage {
            role: "user".to_string(),
            content: prompt(role, jd, resume, letter, ctx),
        },
    ];

    // The documents alone can approach 8k tokens; keep room for the JSON reply.
    let num_ctx = Some(num_ctx.unwrap_or(8192).max(8192));
    let mut attempts = 0;
    let mut last_problem = String::new();
    for _ in 0..2 {
        attempts += 1;
        let response = chat_with_ollama_async(
            model.to_string(),
            messages.clone(),
            Some(0.2),
            Some(1500),
            Some("json".to_string()),
            num_ctx,
        )
        .await?;
        let content = response.message.content.clone();
        let parsed: ReviewDraft = match serde_json::from_str(json_body(&content)) {
            Ok(mut draft) => {
                normalize_draft(&mut draft, ctx);
                draft
            }
            Err(error) => {
                last_problem = format!("the model did not return usable review JSON: {}", error);
                messages.push(OllamaChatMessage {
                    role: "user".to_string(),
                    content: format!(
                        "{}\n\nReturn the JSON again, correcting that problem.",
                        last_problem
                    ),
                });
                continue;
            }
        };
        match validate_draft(&parsed, ctx, claims) {
            Ok(()) => {
                let review = RecruiterReview {
                    verdict: parsed.verdict,
                    employer: employer.to_string(),
                    role: role.to_string(),
                    generated_at: Utc::now().to_rfc3339(),
                    model: Some(model.to_string()),
                    attempts,
                    evidence: ctx.evidence.clone(),
                    strengths: parsed
                        .strengths
                        .into_iter()
                        .map(|point| ReviewPoint {
                            text: point.text,
                            evidence: point.evidence,
                        })
                        .collect(),
                    weaknesses: parsed
                        .weaknesses
                        .into_iter()
                        .map(|point| ReviewPoint {
                            text: point.text,
                            evidence: point.evidence,
                        })
                        .collect(),
                    improvements: parsed
                        .improvements
                        .into_iter()
                        .map(|entry| Improvement {
                            text: entry.text,
                            target: entry.target,
                        })
                        .collect(),
                    markdown: String::new(),
                };
                let mut review = review;
                review.markdown = render_markdown(&review);
                return Ok(review);
            }
            Err(problem) => {
                last_problem = problem;
                messages.push(OllamaChatMessage {
                    role: "user".to_string(),
                    content: format!(
                        "That review was rejected: {}\n\nReturn corrected JSON.",
                        last_problem
                    ),
                });
            }
        }
    }

    Err(format!(
        "the recruiter review did not pass validation after {} attempts: {}",
        attempts, last_problem
    ))
}

fn app_dir(application_id: &str) -> Result<std::path::PathBuf, String> {
    let docs = dirs::document_dir().ok_or("Failed to locate Documents folder")?;
    Ok(docs
        .join("CareerOS")
        .join("applications")
        .join(application_id))
}

fn latest_application(conn: &rusqlite::Connection) -> Result<(String, String, String), String> {
    conn.query_row(
        "SELECT id, employer, role FROM application_records ORDER BY date DESC, id DESC LIMIT 1",
        [],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        },
    )
    .map_err(|_| "No applications yet. Run /apply first.".to_string())
}

fn read_document(application_id: &str, name: &str) -> String {
    app_dir(application_id)
        .ok()
        .and_then(|dir| fs::read_to_string(dir.join(name)).ok())
        .unwrap_or_default()
}

/// Review the most recent application, or a named one.
#[tauri::command]
pub async fn generate_recruiter_review(
    application_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<RecruiterReview, String> {
    let db_path = {
        let guard = state.db_path.lock().unwrap();
        guard
            .as_ref()
            .ok_or("Database not initialized")?
            .clone()
    };
    let conn = get_db_connection(&db_path)?;
    let claims = fetch_confirmed_claims(&conn)?;
    if claims.is_empty() {
        return Err("No confirmed claims. Complete Setup before asking for a review.".to_string());
    }

    let (application_id, employer, role) = match application_id {
        Some(id) => {
            let (employer, role) = conn
                .query_row(
                    "SELECT employer, role FROM application_records WHERE id = ?1",
                    [id.as_str()],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .map_err(|_| format!("Application not found: {}", id))?;
            (id, employer, role)
        }
        None => latest_application(&conn)?,
    };

    let resume = read_document(&application_id, "resume.md");
    let letter = read_document(&application_id, "cover-letter.md");
    if resume.trim().is_empty() && letter.trim().is_empty() {
        return Err("No generated materials for this application. Run /apply first.".to_string());
    }
    let jd = read_document(&application_id, "job-description.md");

    let ctx = build_context(&conn, &application_id, &claims);
    let config = crate::ollama::get_ollama_config(state.clone()).unwrap_or_default();
    let model = config
        .deep_model
        .clone()
        .or_else(|| config.fast_model.clone())
        .unwrap_or_else(|| "llama3.2:latest".to_string());

    let review = review_documents(
        &employer,
        &role,
        &jd,
        &resume,
        &letter,
        &claims,
        &ctx,
        &model,
        config.num_ctx,
    )
    .await?;

    let dir = app_dir(&application_id)?;
    fs::write(dir.join("recruiter-review.md"), &review.markdown)
        .map_err(|error| format!("Failed to write recruiter-review.md: {}", error))?;
    fs::write(
        dir.join("recruiter-review.json"),
        serde_json::to_string_pretty(&review).unwrap_or_default(),
    )
    .map_err(|error| format!("Failed to write recruiter-review.json: {}", error))?;

    Ok(review)
}

#[cfg(test)]
mod tests {
    use super::{build_context, validate_draft, ReviewContext, ReviewDraft, ReviewEvidence};
    use crate::claims::Claim;
    use std::collections::HashSet;

    fn claim(id: &str, wording: &str) -> Claim {
        Claim {
            id: id.to_string(),
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

    fn context() -> ReviewContext {
        let evidence = vec![
            ReviewEvidence {
                id: "claim:claim-1".to_string(),
                summary: "Cut reporting costs by 12%".to_string(),
            },
            ReviewEvidence {
                id: "gap:gap-1".to_string(),
                summary: "Own the learning technology roadmap".to_string(),
            },
        ];
        ReviewContext {
            evidence_ids: evidence
                .iter()
                .map(|item| item.id.clone())
                .collect::<HashSet<_>>(),
            evidence,
            claim_ids: ["claim-1".to_string()].into_iter().collect(),
            gap_ids: ["gap-1".to_string()].into_iter().collect(),
            marker_ids: HashSet::new(),
        }
    }

    fn draft(
        verdict: &str,
        strengths: Vec<&str>,
        weaknesses: Vec<&str>,
        target: &str,
    ) -> ReviewDraft {
        let point = |text: &str, evidence: Vec<&str>| super::PointDraft {
            text: text.to_string(),
            evidence: evidence.iter().map(|id| id.to_string()).collect(),
        };
        ReviewDraft {
            verdict: verdict.to_string(),
            strengths: vec![point("Cost reduction is evidenced", strengths)],
            weaknesses: vec![point("The roadmap is not evidenced", weaknesses)],
            improvements: vec![super::ImprovementDraft {
                text: "Name the scope of the cost reduction".to_string(),
                target: target.to_string(),
            }],
        }
    }

    #[test]
    fn a_review_citing_one_item_is_rejected() {
        let claims = vec![claim("claim-1", "Cut reporting costs by 12%")];
        let err = validate_draft(
            &draft(
                "shortlist",
                vec!["claim:claim-1"],
                vec!["claim:claim-1"],
                "claim:claim-1",
            ),
            &context(),
            &claims,
        )
        .unwrap_err();
        assert!(err.contains("at least two"), "{}", err);
    }

    #[test]
    fn a_review_with_general_advice_is_rejected() {
        let claims = vec![claim("claim-1", "Cut reporting costs by 12%")];
        let err = validate_draft(
            &draft(
                "maybe",
                vec!["claim:claim-1"],
                vec!["gap:gap-1"],
                "improve the summary",
            ),
            &context(),
            &claims,
        )
        .unwrap_err();
        assert!(err.contains("does not name"), "{}", err);
    }

    #[test]
    fn a_review_with_an_unknown_verdict_is_rejected() {
        let claims = vec![claim("claim-1", "Cut reporting costs by 12%")];
        let err = validate_draft(
            &draft(
                "strong-hire",
                vec!["claim:claim-1"],
                vec!["gap:gap-1"],
                "gap:gap-1",
            ),
            &context(),
            &claims,
        )
        .unwrap_err();
        assert!(err.contains("not one of"), "{}", err);
    }

    #[test]
    fn a_review_may_not_add_a_number() {
        let claims = vec![claim("claim-1", "Cut reporting costs by 12%")];
        let mut review = draft(
            "reject",
            vec!["claim:claim-1"],
            vec!["gap:gap-1"],
            "gap:gap-1",
        );
        review.strengths[0].text = "Costs fell 45%".to_string();
        let err = validate_draft(&review, &context(), &claims).unwrap_err();
        assert!(err.contains("45"), "{}", err);
    }

    #[test]
    fn a_bare_id_is_resolved_against_the_evidence_set() {
        let claims = vec![claim("claim-1", "Cut reporting costs by 12%")];
        let mut review = draft("maybe", vec!["claim-1"], vec!["gap-1"], "gap-1");
        super::normalize_draft(&mut review, &context());
        validate_draft(&review, &context(), &claims).unwrap();
        assert_eq!(review.strengths[0].evidence, vec!["claim:claim-1"]);
        assert_eq!(review.weaknesses[0].evidence, vec!["gap:gap-1"]);
    }

    #[test]
    fn a_gap_cited_as_a_strength_is_rejected() {
        let claims = vec![claim("claim-1", "Cut reporting costs by 12%")];
        let err = validate_draft(
            &draft(
                "maybe",
                vec!["claim:claim-1", "gap:gap-1"],
                vec!["gap:gap-1"],
                "gap:gap-1",
            ),
            &context(),
            &claims,
        )
        .unwrap_err();
        assert!(err.contains("listed as a strength"), "{}", err);
    }

    #[test]
    fn a_review_with_too_many_points_is_rejected() {
        let claims = vec![claim("claim-1", "Cut reporting costs by 12%")];
        let mut review = draft(
            "maybe",
            vec!["claim:claim-1"],
            vec!["gap:gap-1"],
            "gap:gap-1",
        );
        let point = |text: &str| super::PointDraft {
            text: text.to_string(),
            evidence: vec!["claim:claim-1".to_string()],
        };
        review.strengths = vec![point("a"), point("b"), point("c"), point("d")];
        let err = validate_draft(&review, &context(), &claims).unwrap_err();
        assert!(err.contains("more than three"), "{}", err);
    }

    #[test]
    fn a_review_without_weaknesses_is_rejected() {
        let claims = vec![claim("claim-1", "Cut reporting costs by 12%")];
        let mut review = draft(
            "maybe",
            vec!["claim:claim-1"],
            vec!["gap:gap-1"],
            "gap:gap-1",
        );
        review.weaknesses.clear();
        let err = validate_draft(&review, &context(), &claims).unwrap_err();
        assert!(err.contains("no weaknesses"), "{}", err);
    }

    #[test]
    fn a_valid_review_is_accepted() {
        let claims = vec![claim("claim-1", "Cut reporting costs by 12%")];
        validate_draft(
            &draft(
                "maybe",
                vec!["claim:claim-1"],
                vec!["gap:gap-1"],
                "gap:gap-1",
            ),
            &context(),
            &claims,
        )
        .unwrap();
    }

    #[test]
    fn evidence_comes_from_claims_gaps_and_markers() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE gaps (id TEXT PRIMARY KEY, application_id TEXT, requirement TEXT);
             INSERT INTO gaps VALUES ('gap-1', 'app-1', 'Own the roadmap');
             CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO app_settings VALUES ('bullet_marker:claim-1', '{\"markers\":[\"[ADD SCOPE]\"]}');",
        )
        .unwrap();

        let claims = vec![claim("claim-1", "Cut reporting costs by 12%")];
        let ctx = build_context(&conn, "app-1", &claims);
        let ids: Vec<&str> = ctx.evidence.iter().map(|item| item.id.as_str()).collect();
        assert!(ids.contains(&"claim:claim-1"));
        assert!(ids.contains(&"gap:gap-1"));
        assert!(ids.contains(&"marker:claim-1"));
    }
}
