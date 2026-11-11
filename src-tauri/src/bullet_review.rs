//! Bullet strengthening (prompt 3).
//!
//! A rewrite is only returned when the ledger can support it. Otherwise the
//! bullet comes back as `needs_human_input` or `blocked`, with the offending
//! number named — never as plausible prose the candidate cannot defend.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::OnceLock;
use tauri::State;

use crate::claims::Claim;
use crate::db::get_db_connection;
use crate::fact_check::verify_metrics;
use crate::guardrails::{cited_claim_ids, fetch_confirmed_claims, is_grounded};
use crate::ollama::{chat_with_ollama_async, OllamaChatMessage};
use crate::skills::markers_in;
use crate::AppState;

const MAX_CLAIMS_IN_PROMPT: usize = 24;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulletRewrite {
    pub original: String,
    pub rewrite: String,
    pub claim_ids: Vec<String>,
    pub markers: Vec<String>,
    /// `rewritten`, `needs_human_input` or `blocked`.
    pub status: String,
    pub problem: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulletReviewReport {
    pub bullets: Vec<BulletRewrite>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RewriteDraft {
    #[serde(default)]
    original: String,
    #[serde(default)]
    rewrite: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RewriteResponse {
    #[serde(default)]
    bullets: Vec<RewriteDraft>,
}

/// Models sometimes cite a claim as `[uuid]`; those digits are not metrics.
static CLAIM_CITATION: OnceLock<Regex> = OnceLock::new();

fn strip_claim_citations(text: &str) -> String {
    let pattern = CLAIM_CITATION.get_or_init(|| {
        Regex::new(r"(?i)\[?[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\]?")
            .expect("claim citation pattern")
    });
    pattern.replace_all(text, "").to_string()
}

/// Glue words that do not identify which achievement a bullet is about.
const SUBJECT_STOPWORDS: &[&str] = &[
    "with", "from", "that", "this", "through", "across", "into", "within", "their", "there",
    "been", "were", "have", "them", "they", "your", "will", "over", "under", "then", "than",
    "also", "more", "most", "less", "least", "used", "using", "including", "plus", "only",
    "just", "each", "both", "when", "what", "which", "while", "after", "before", "about",
    "against", "between", "during",
];

fn content_words(text: &str) -> HashSet<String> {
    text.to_lowercase()
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| word.chars().count() >= 4 && !SUBJECT_STOPWORDS.contains(word))
        .map(str::to_string)
        .collect()
}

/// A rewrite may rephrase a bullet, but it may not answer a different one.
fn subject_changed(original: &str, rewrite: &str) -> bool {
    let original_words = content_words(original);
    let rewrite_words = content_words(rewrite);
    if original_words.is_empty() || rewrite_words.is_empty() {
        return false;
    }
    original_words.is_disjoint(&rewrite_words)
}

fn numeric_tokens(text: &str) -> Vec<String> {
    text.split(|character: char| !character.is_ascii_digit())
        .filter(|token| !token.is_empty())
        .map(|token| token.trim_start_matches('0').to_string())
        .filter(|token| !token.is_empty())
        .collect()
}

/// Numbers in the rewrite that no confirmed claim contains, deduplicated.
fn unsupported_numbers(text: &str, claims: &[Claim]) -> Vec<String> {
    let allowed: HashSet<String> = claims
        .iter()
        .flat_map(|claim| {
            numeric_tokens(&claim.confirmed_wording)
                .into_iter()
                .chain(numeric_tokens(&claim.statement))
        })
        .collect();
    let mut unsupported: Vec<String> = numeric_tokens(text)
        .into_iter()
        .filter(|token| !allowed.contains(token))
        .collect();
    unsupported.sort();
    unsupported.dedup();
    unsupported
}

fn needs_human_input(original: &str, problem: String) -> BulletRewrite {
    BulletRewrite {
        original: original.to_string(),
        rewrite: String::new(),
        claim_ids: Vec::new(),
        markers: Vec::new(),
        status: "needs_human_input".to_string(),
        problem: Some(problem),
    }
}

fn blocked(original: &str, problem: String, markers: Vec<String>) -> BulletRewrite {
    BulletRewrite {
        original: original.to_string(),
        rewrite: String::new(),
        claim_ids: Vec::new(),
        markers,
        status: "blocked".to_string(),
        problem: Some(problem),
    }
}

/// Decide what a proposed rewrite may be shown as.
pub(crate) fn classify_rewrite(
    original: &str,
    rewrite: &str,
    claims: &[Claim],
) -> BulletRewrite {
    let cleaned = strip_claim_citations(rewrite);
    let rewrite = cleaned.trim();
    if rewrite.is_empty() {
        return needs_human_input(
            original,
            "the model returned no rewrite for this bullet".to_string(),
        );
    }

    let markers = markers_in(rewrite);

    // A rewritten number the ledger cannot support is blocking, not advisory.
    let unsupported = unsupported_numbers(rewrite, claims);
    if !unsupported.is_empty() {
        return blocked(
            original,
            format!("unsupported number(s): {}", unsupported.join(", ")),
            markers,
        );
    }
    let metrics = verify_metrics(rewrite, claims);
    if !metrics.unverified.is_empty() {
        return blocked(
            original,
            format!("unsupported metric(s): {}", metrics.unverified.join(", ")),
            markers,
        );
    }
    if !metrics.qualifier_violations.is_empty() {
        return blocked(
            original,
            format!(
                "metric qualifier changed: {}",
                metrics.qualifier_violations.join("; ")
            ),
            markers,
        );
    }

    if subject_changed(original, rewrite) {
        return needs_human_input(
            original,
            "the rewrite dropped the original bullet's subject; rewrite that same achievement or return nothing".to_string(),
        );
    }

    if !is_grounded(rewrite, claims) {
        return needs_human_input(
            original,
            "no confirmed claim supports this rewrite; supply the missing fact".to_string(),
        );
    }
    let claim_ids = cited_claim_ids(rewrite, claims);
    if claim_ids.is_empty() {
        return needs_human_input(
            original,
            "no confirmed claim supports this rewrite; supply the missing fact".to_string(),
        );
    }

    BulletRewrite {
        original: original.to_string(),
        rewrite: rewrite.to_string(),
        claim_ids,
        markers,
        status: "rewritten".to_string(),
        problem: None,
    }
}

fn demo_drafts() -> Option<Vec<RewriteDraft>> {
    let path = std::env::var("VERIDICUS_DEMO_BULLETS").ok()?;
    let raw = std::fs::read_to_string(path).ok()?;
    parse_demo_drafts(&raw)
}

fn parse_demo_drafts(raw: &str) -> Option<Vec<RewriteDraft>> {
    serde_json::from_str::<RewriteResponse>(raw)
        .ok()
        .map(|parsed| parsed.bullets)
}

fn review_prompt(bullets: &[String], jd_text: &str, claims: &[Claim]) -> String {
    let mut prompt = String::from("## Confirmed claims\n");
    for claim in claims.iter().take(MAX_CLAIMS_IN_PROMPT) {
        prompt.push_str(&format!("- [{}] {}\n", claim.id, claim.confirmed_wording));
    }
    if !jd_text.trim().is_empty() {
        prompt.push_str("\n## Target posting\n");
        prompt.push_str(jd_text.trim());
        prompt.push('\n');
    }
    prompt.push_str("\n## Bullets\n");
    for (index, bullet) in bullets.iter().enumerate() {
        prompt.push_str(&format!("{}. {}\n", index + 1, bullet));
    }
    prompt.push_str(
        "\nReturn only JSON: {\"bullets\":[{\"original\":\"...\",\"rewrite\":\"...\"}]}, one entry per bullet, in order. \
Every number in a rewrite must already appear in a confirmed claim. \
Append exactly one marker when a fact is missing: [ADD METRIC], [ADD SCOPE] or [ADD TOOL]. Never invent the fact. \
Leave the rewrite empty when no claim supports the bullet.",
    );
    prompt
}

/// Record a marker decision against the claim it concerns, so a later resume
/// generation can see that the user still owes a fact there.
fn record_marker_decision(
    conn: &rusqlite::Connection,
    claim_id: &str,
    bullet: &BulletRewrite,
) -> Result<(), String> {
    let value = serde_json::json!({
        "markers": bullet.markers,
        "original": bullet.original,
        "rewrite": bullet.rewrite,
        "recorded_at": chrono::Utc::now().to_rfc3339(),
    })
    .to_string();
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = ?2",
        rusqlite::params![format!("bullet_marker:{}", claim_id), value],
    )
    .map_err(|error| format!("Failed to record marker decision: {}", error))?;
    Ok(())
}

/// Persist the wording the user approved for reuse on this application.
pub(crate) fn write_approved_wording(
    conn: &rusqlite::Connection,
    application_id: &str,
    wording: &str,
) -> Result<(), String> {
    if wording.trim().is_empty() {
        return Err("Approved wording cannot be empty.".to_string());
    }
    let updated = conn
        .execute(
            "UPDATE application_records SET approved_wording = ?1 WHERE id = ?2",
            rusqlite::params![wording, application_id],
        )
        .map_err(|error| format!("Failed to record approved wording: {}", error))?;
    if updated == 0 {
        return Err(format!("Application not found: {}", application_id));
    }
    Ok(())
}

pub(crate) fn read_approved_wording(
    conn: &rusqlite::Connection,
    application_id: &str,
) -> Result<String, String> {
    conn.query_row(
        "SELECT approved_wording FROM application_records WHERE id = ?1",
        [application_id],
        |row| row.get(0),
    )
    .map_err(|error| format!("Failed to read approved wording: {}", error))
}

/// Approval is explicit: an empty string is never a valid decision.
#[tauri::command]
pub fn record_approved_wording(
    application_id: String,
    wording: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let wording = wording.trim().to_string();
    if wording.is_empty() {
        return Err(
            "Approved wording cannot be empty. Approve the claim wording you will reuse."
                .to_string(),
        );
    }
    let db_path = {
        let guard = state.db_path.lock().unwrap();
        guard
            .as_ref()
            .ok_or("Database not initialized")?
            .clone()
    };
    let conn = get_db_connection(&db_path)?;
    write_approved_wording(&conn, &application_id, &wording)?;
    Ok(wording)
}

#[tauri::command]
pub async fn review_bullets(
    application_id: Option<String>,
    bullets: Vec<String>,
    jd_text: Option<String>,
    state: State<'_, AppState>,
) -> Result<BulletReviewReport, String> {
    let bullets: Vec<String> = bullets
        .into_iter()
        .map(|bullet| bullet.trim().to_string())
        .filter(|bullet| !bullet.is_empty())
        .collect();
    if bullets.is_empty() {
        return Err("No bullets were supplied to review.".to_string());
    }

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
        return Err("No confirmed claims. Complete Setup so rewrites can be grounded.".to_string());
    }

    let config = crate::ollama::get_ollama_config(state.clone()).unwrap_or_default();
    let model = config
        .fast_model
        .clone()
        .or_else(|| config.deep_model.clone())
        .unwrap_or_else(|| "llama3.2:latest".to_string());

    let messages = vec![
        OllamaChatMessage {
            role: "system".to_string(),
            content: "You strengthen resume bullets from a confirmed claims ledger. \
Never invent facts, numbers, tools or scope. Return JSON only."
                .to_string(),
        },
        OllamaChatMessage {
            role: "user".to_string(),
            content: review_prompt(&bullets, jd_text.as_deref().unwrap_or(""), &claims),
        },
    ];

    // A checked-in fixture can stand in for the model so the blocked-claim flow is
    // reproducible for screenshots and recordings. It still passes through every
    // classifier below, so a fixture cannot fake a pass the guards would refuse.
    let drafts: Vec<RewriteDraft> = match demo_drafts() {
        Some(drafts) => drafts,
        None => match chat_with_ollama_async(
            model.clone(),
            messages,
            Some(0.3),
            Some(1500),
            Some("json".to_string()),
            config.num_ctx,
        )
        .await
        {
            Ok(response) => serde_json::from_str::<RewriteResponse>(response.message.content.trim())
                .map(|parsed| parsed.bullets)
                .unwrap_or_default(),
            Err(_) => Vec::new(),
        },
    };

    let mut reviewed: Vec<BulletRewrite> = Vec::with_capacity(bullets.len());
    for (index, original) in bullets.iter().enumerate() {
        let draft = drafts
            .iter()
            .find(|draft| draft.original.trim() == original.trim())
            .or_else(|| drafts.get(index));
        let reviewed_bullet = match draft {
            Some(draft) => classify_rewrite(original, &draft.rewrite, &claims),
            None => needs_human_input(
                original,
                "the model returned no usable rewrite for this bullet".to_string(),
            ),
        };
        reviewed.push(reviewed_bullet);
    }

    for bullet in &reviewed {
        if bullet.markers.is_empty() {
            continue;
        }
        for claim_id in &bullet.claim_ids {
            record_marker_decision(&conn, claim_id, bullet)?;
        }
    }

    let _ = application_id;
    Ok(BulletReviewReport {
        bullets: reviewed,
        model: Some(model),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        classify_rewrite, read_approved_wording, record_marker_decision, write_approved_wording,
        BulletRewrite,
    };
    use crate::claims::Claim;

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

    #[test]
    fn an_invented_metric_is_blocked_and_named() {
        let result = classify_rewrite(
            "Cut reporting costs",
            "Cut reporting costs by 45% for the division",
            &[claim("claim-1", "Cut reporting costs by 12% for the division")],
        );
        assert_eq!(result.status, "blocked");
        assert!(result.rewrite.is_empty());
        let problem = result.problem.unwrap_or_default();
        assert!(problem.contains("45"), "names the offending token: {}", problem);
    }

    #[test]
    fn an_unsupported_bullet_needs_human_input_not_prose() {
        let result = classify_rewrite(
            "Owned the vendor relationship",
            "Negotiated a new vendor contract that saved the department money",
            &[claim("claim-1", "Cut reporting costs by 12% for the division")],
        );
        assert_eq!(result.status, "needs_human_input");
        assert!(result.rewrite.is_empty());
        assert!(result.problem.is_some());
    }

    #[test]
    fn a_supported_rewrite_cites_its_claim() {
        let result = classify_rewrite(
            "Cut reporting costs",
            "Cut reporting costs by 12% for the division",
            &[claim("claim-1", "Cut reporting costs by 12% for the division")],
        );
        assert_eq!(result.status, "rewritten", "problem: {:?}", result.problem);
        assert_eq!(result.claim_ids, vec!["claim-1".to_string()]);
        assert!(result.markers.is_empty());
    }

    #[test]
    fn a_marker_rewrite_is_accepted_for_bullet_review() {
        let result = classify_rewrite(
            "Cut reporting costs",
            "Cut reporting costs by 12% for the division [ADD SCOPE]",
            &[claim("claim-1", "Cut reporting costs by 12% for the division")],
        );
        assert_eq!(result.status, "rewritten", "problem: {:?}", result.problem);
        assert_eq!(result.markers, vec!["[ADD SCOPE]".to_string()]);
    }

    #[test]
    fn approved_wording_round_trips_and_rejects_empty() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE application_records (id TEXT PRIMARY KEY, approved_wording TEXT NOT NULL DEFAULT '');\n             INSERT INTO application_records (id) VALUES ('app-1');",
        )
        .unwrap();

        assert!(write_approved_wording(&conn, "app-1", "  ").is_err());
        write_approved_wording(&conn, "app-1", "Cut reporting costs by 12%").unwrap();
        assert_eq!(
            read_approved_wording(&conn, "app-1").unwrap(),
            "Cut reporting costs by 12%"
        );
        assert!(write_approved_wording(&conn, "missing", "x").is_err());
    }

    #[test]
    fn recorded_markers_surface_for_generation() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO app_settings (key, value) VALUES ('bullet_marker:claim-1', ?1)",
            [r#"{"markers":["[ADD SCOPE]"],"original":"Cut reporting costs","rewrite":"Cut costs [ADD SCOPE]","recorded_at":"2026-11-08T00:00:00Z"}"#],
        )
        .unwrap();
        // A malformed row is ignored rather than failing generation.
        conn.execute(
            "INSERT INTO app_settings (key, value) VALUES ('bullet_marker:claim-2', 'not json')",
            [],
        )
        .unwrap();

        let markers = crate::skills::open_markers(&conn).unwrap();
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].claim_id, "claim-1");
        assert_eq!(markers[0].markers, vec!["[ADD SCOPE]".to_string()]);
    }

    #[test]
    fn marker_decisions_are_recorded_per_claim() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
        )
        .unwrap();

        let bullet = BulletRewrite {
            original: "Cut reporting costs".to_string(),
            rewrite: "Cut reporting costs by 12% [ADD SCOPE]".to_string(),
            claim_ids: vec!["claim-1".to_string()],
            markers: vec!["[ADD SCOPE]".to_string()],
            status: "rewritten".to_string(),
            problem: None,
        };
        record_marker_decision(&conn, "claim-1", &bullet).unwrap();

        let stored: String = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = 'bullet_marker:claim-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(stored.contains("[ADD SCOPE]"));
    }

    #[test]
    fn claim_citations_are_not_counted_as_numbers() {
        let result = classify_rewrite(
            "Cut reporting costs",
            "Cut reporting costs by 12% for the division [9abc6037-707e-4531-a654-5947e610666c]",
            &[claim("claim-1", "Cut reporting costs by 12% for the division")],
        );
        assert_eq!(result.status, "rewritten", "problem: {:?}", result.problem);
        assert!(
            !result.rewrite.contains("9abc6037"),
            "citation is stripped: {}",
            result.rewrite
        );
    }

    #[test]
    fn a_rewrite_about_a_different_achievement_needs_human_input() {
        let result = classify_rewrite(
            "Increased customer retention by 47% through a redesigned onboarding programme",
            "Cut asset search time from 30 minutes to under 5 minutes by restructuring the SharePoint library",
            &[
                claim(
                    "claim-1",
                    "Cut asset search time from 30 minutes to under 5 minutes by restructuring the SharePoint library",
                ),
                claim("claim-2", "Rebuilt the new-hire onboarding curriculum"),
            ],
        );
        assert_eq!(result.status, "needs_human_input");
        assert!(result.rewrite.is_empty());
        let problem = result.problem.unwrap_or_default();
        assert!(problem.contains("subject"), "{problem}");
    }

    #[test]
    fn demo_fixture_parses_into_drafts() {
        let raw = std::fs::read_to_string("eval/demo-bullets.json").expect("fixture is checked in");
        let drafts = super::parse_demo_drafts(&raw).expect("fixture parses");
        assert_eq!(drafts.len(), 2);
        assert!(drafts[0].rewrite.contains("47%"));
    }
}
