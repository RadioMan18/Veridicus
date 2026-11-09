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

/// Sent as `format`. Plain `"json"` only guarantees well-formed JSON, so a model
/// may end after `strengths` and still satisfy it — which is how a review arrives
/// with no weaknesses at all. A schema makes every key, and two items per key,
/// mandatory at the decoder.
const REVIEW_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "verdict": {"type": "string", "enum": ["shortlist", "maybe", "reject"]},
    "strengths": {"type": "array", "minItems": 2, "maxItems": 2, "items": {"type": "object", "properties": {"text": {"type": "string"}, "evidence": {"type": "array", "minItems": 1, "items": {"type": "string"}}}, "required": ["text", "evidence"]}},
    "weaknesses": {"type": "array", "minItems": 2, "maxItems": 2, "items": {"type": "object", "properties": {"text": {"type": "string"}, "evidence": {"type": "array", "minItems": 1, "items": {"type": "string"}}}, "required": ["text", "evidence"]}},
    "improvements": {"type": "array", "minItems": 2, "maxItems": 2, "items": {"type": "object", "properties": {"text": {"type": "string"}, "target": {"type": "string"}}, "required": ["text", "target"]}}
  },
  "required": ["verdict", "strengths", "weaknesses", "improvements"]
}"#;

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

/// Append the closers a prefix still needs, so a reply cut off by the token cap
/// can be read instead of discarded. Returns None when the cut lands mid-string.
fn close_open_structures(prefix: &str) -> Option<String> {
    let mut stack: Vec<char> = Vec::new();
    let mut in_string = false;
    let mut escaped = false;
    for character in prefix.chars() {
        if in_string {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }
        match character {
            '"' => in_string = true,
            '{' | '[' => stack.push(character),
            '}' | ']' => {
                let expected = if character == '}' { '{' } else { '[' };
                if stack.pop() != Some(expected) {
                    return None;
                }
            }
            _ => {}
        }
    }
    if in_string {
        return None;
    }
    let mut repaired = prefix.to_string();
    while let Some(open) = stack.pop() {
        repaired.push(if open == '{' { '}' } else { ']' });
    }
    Some(repaired)
}

/// Retry the parse from the last complete value when the reply was truncated.
fn close_truncated_json(body: &str) -> Option<String> {
    let cuts: Vec<usize> = body
        .char_indices()
        .filter(|(_, character)| matches!(character, '}' | ']' | ','))
        .map(|(index, _)| index + 1)
        .rev()
        .take(400)
        .collect();
    for cut in cuts {
        if !body.is_char_boundary(cut) {
            continue;
        }
        let prefix = body[..cut].trim_end().trim_end_matches(',').trim_end();
        if let Some(repaired) = close_open_structures(prefix) {
            if serde_json::from_str::<ReviewDraft>(&repaired).is_ok() {
                return Some(repaired);
            }
        }
    }
    None
}

/// A parsed review, and whether the reply had to be repaired because the model
/// ran out of room. A repaired reply is still validated, but a failure on it is
/// reported as truncation so the retry asks for less instead of for a fix.
struct ParsedReview {
    draft: ReviewDraft,
    truncated: bool,
}

fn parse_review(content: &str) -> Result<ParsedReview, String> {
    let body = json_body(content);
    if let Ok(draft) = serde_json::from_str::<ReviewDraft>(body) {
        return Ok(ParsedReview {
            draft,
            truncated: false,
        });
    }
    if let Some(repaired) = close_truncated_json(body) {
        if let Ok(draft) = serde_json::from_str::<ReviewDraft>(&repaired) {
            return Ok(ParsedReview {
                draft,
                truncated: true,
            });
        }
    }
    Err(serde_json::from_str::<ReviewDraft>(body)
        .err()
        .map(|error| error.to_string())
        .unwrap_or_else(|| "the reply was not valid JSON".to_string()))
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

/// Roughly how many bytes of prompt text one token costs. Deliberately low: a
/// prompt that overruns `num_ctx` is trimmed by Ollama, and the reply then has
/// nowhere to go.
const BYTES_PER_TOKEN: usize = 3;

/// Split the context window between reading and replying. The visible JSON is a
/// few hundred tokens, but a model that reasons before answering can spend
/// several times that first, so the reply keeps a third of the window and the
/// documents are capped to what is left instead of to a fixed size.
/// Where the reply's tokens went, so a failure names the limit it hit instead
/// of leaving the next attempt to guess.
fn usage_note(response: &crate::ollama::OllamaChatResponse) -> String {
    let mut parts = Vec::new();
    if let Some(reason) = response.done_reason.as_deref() {
        parts.push(format!("stopped: {}", reason));
    }
    if let Some(prompt) = response.prompt_eval_count {
        parts.push(format!("prompt {} tokens", prompt));
    }
    if let Some(reply) = response.eval_count {
        parts.push(format!("reply {} tokens", reply));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" [{}]", parts.join(", "))
    }
}

fn prompt_caps(num_ctx: u32) -> (usize, usize) {
    let reply_tokens = (num_ctx / 3).max(1_500);
    let prompt_bytes = num_ctx.saturating_sub(reply_tokens) as usize * BYTES_PER_TOKEN;
    let jd = (prompt_bytes * 3 / 10).min(MAX_JD_BYTES);
    let document = (prompt_bytes * 3 / 10).min(MAX_DOCUMENT_BYTES);
    (jd, document)
}

fn prompt(
    role: &str,
    jd: &str,
    resume: &str,
    letter: &str,
    ctx: &ReviewContext,
    jd_cap: usize,
    doc_cap: usize,
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
        truncate(jd, jd_cap),
        truncate(resume, doc_cap),
        truncate(letter, doc_cap)
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
    let num_ctx = Some(
        num_ctx
            .unwrap_or(crate::ollama::DEFAULT_NUM_CTX)
            .max(8_192),
    );
    let (jd_cap, doc_cap) = prompt_caps(num_ctx.unwrap_or(8_192));
    let mut attempts = 0;
    let mut last_problem = String::new();
    let mut correction: Option<String> = None;
    for _ in 0..2 {
        attempts += 1;
        // A retry sends less to read, so a reply that ran out of room the first
        // time gets room back instead of hitting the same wall.
        let scale = if attempts == 1 { 100 } else { 50 };
        let mut messages = vec![
            OllamaChatMessage {
                role: "system".to_string(),
                content: system.to_string(),
            },
            OllamaChatMessage {
                role: "user".to_string(),
                content: prompt(
                    role,
                    jd,
                    resume,
                    letter,
                    ctx,
                    jd_cap * scale / 100,
                    doc_cap * scale / 100,
                ),
            },
        ];
        if let Some(pending) = correction.take() {
            messages.push(OllamaChatMessage {
                role: "user".to_string(),
                content: pending,
            });
        }
        let response = chat_with_ollama_async(
            model.to_string(),
            messages,
            Some(0.2),
            // 2600 tokens is a generous two-point review once thinking is off.
            Some(2600),
            Some(REVIEW_SCHEMA.to_string()),
            num_ctx,
        )
        .await?;
        let content = response.message.content.clone();
        let (mut parsed, truncated) = match parse_review(&content) {
            Ok(parsed) => (parsed.draft, parsed.truncated),
            Err(error) => {
                last_problem = format!("the model did not return usable review JSON: {}", error);
                correction = Some(format!(
                    "{}\n\nReturn the JSON again, correcting that problem.",
                    last_problem
                ));
                continue;
            }
        };
        normalize_draft(&mut parsed, ctx);
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
                last_problem = if truncated {
                    format!(
                        "the reply was cut off before it finished ({}){}",
                        problem,
                        usage_note(&response)
                    )
                } else {
                    problem
                };
                // A cut-off reply will be cut off again if the request stays the
                // same size, so ask for the smallest valid review instead.
                correction = Some(if truncated {
                    "That reply ran out of room. Return compact JSON with exactly one strength, one weakness and one improvement, each a single short sentence, and no other keys.".to_string()
                } else {
                    format!(
                        "That review was rejected: {}\n\nA review needs at least one strength, one weakness and one improvement. Return corrected JSON.",
                        last_problem
                    )
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
    use super::{
        build_context, parse_review, prompt_caps, validate_draft, ReviewContext, ReviewDraft,
        ReviewEvidence, BYTES_PER_TOKEN,
    };
    use crate::claims::Claim;
    use std::collections::HashSet;

    #[test]
    fn the_review_schema_requires_every_section() {
        let schema: serde_json::Value =
            serde_json::from_str(super::REVIEW_SCHEMA).expect("the schema is valid JSON");
        let required: Vec<&str> = schema["required"]
            .as_array()
            .expect("the schema lists required keys")
            .iter()
            .map(|key| key.as_str().expect("required keys are strings"))
            .collect();

        for key in ["verdict", "strengths", "weaknesses", "improvements"] {
            assert!(required.contains(&key), "schema does not require {}", key);
            if key != "verdict" {
                assert_eq!(
                    schema["properties"][key]["minItems"], 2,
                    "{} must not be optional or empty",
                    key
                );
            }
        }
    }

    #[test]
    fn a_cut_off_reply_reports_where_its_tokens_went() {
        let response = crate::ollama::OllamaChatResponse {
            model: "m".to_string(),
            created_at: String::new(),
            message: crate::ollama::OllamaChatMessage {
                role: "assistant".to_string(),
                content: String::new(),
            },
            done: true,
            done_reason: Some("length".to_string()),
            prompt_eval_count: Some(7_400),
            eval_count: Some(2_600),
        };

        let note = super::usage_note(&response);
        assert!(note.contains("stopped: length"), "{}", note);
        assert!(note.contains("prompt 7400 tokens"), "{}", note);
        assert!(note.contains("reply 2600 tokens"), "{}", note);
    }

    #[test]
    fn the_prompt_leaves_the_reply_room_in_the_window() {
        for num_ctx in [8_192u32, 16_384, 24_576] {
            let (jd, doc) = prompt_caps(num_ctx);
            let prompt_tokens = (jd + doc * 2) as u32 / BYTES_PER_TOKEN as u32;
            assert!(
                prompt_tokens < num_ctx * 3 / 4,
                "{} prompt tokens fills a {} token window",
                prompt_tokens,
                num_ctx
            );
            assert!(
                num_ctx - prompt_tokens >= 1_500,
                "{} tokens of reply room in a {} token window",
                num_ctx - prompt_tokens,
                num_ctx
            );
        }
    }

    #[test]
    fn a_reply_cut_off_mid_review_is_parsed_and_marked_truncated() {
        let content = r#"{"verdict":"maybe","strengths":[{"text":"Strong systems background, matching the integration work in the posting","evidence":["claim:a"]}],"weaknesses":[{"text":"No printable ev"#;
        let parsed = parse_review(content).expect("a truncated reply should be salvaged");
        assert!(parsed.truncated);
        assert_eq!(parsed.draft.verdict, "maybe");
        assert_eq!(parsed.draft.strengths.len(), 1);
        assert!(parsed.draft.weaknesses.is_empty());
    }

    #[test]
    fn a_complete_reply_is_not_marked_truncated() {
        let content = r#"{"verdict":"maybe","strengths":[{"text":"One strength worth naming here","evidence":["claim:a"]}],"weaknesses":[{"text":"One weakness worth naming here","evidence":["claim:b"]}],"improvements":[{"text":"Name the gap in the letter","target":"gap:g1"}]}"#;
        let parsed = parse_review(content).expect("valid JSON should parse");
        assert!(!parsed.truncated);
        assert_eq!(parsed.draft.weaknesses.len(), 1);
    }

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
