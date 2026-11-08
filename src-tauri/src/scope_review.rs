//! Advisory model-assisted scope review.
//!
//! The deterministic checks in `guardrails` are authoritative. This pass gives the
//! local model one narrow job: compare sentences that rely on a confirmed claim
//! against that claim's confirmed wording and recorded scope, and report inflation
//! the lexicon misses.
//!
//! Model output is untrusted. An observation is kept only when it cites a real
//! claim ID and quotes a sentence that occurs in the document. Findings are always
//! advisory and never change the blocking verdict.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::claims::Claim;
use crate::guardrails::GuardrailFinding;
use crate::ollama::{chat_with_ollama_async, OllamaChatMessage};

const MAX_FINDINGS: usize = 5;
const MAX_CLAIMS_IN_PROMPT: usize = 25;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScopeObservation {
    pub claim_id: String,
    pub sentence: String,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct ObservationList {
    #[serde(default)]
    findings: Vec<ScopeObservation>,
}

#[derive(Debug, Clone, Default)]
pub struct ScopeReviewOutcome {
    pub findings: Vec<ScopeObservation>,
    pub ran: bool,
    pub error: Option<String>,
}

fn normalize(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn claim_appears(wording: &str, draft_normalized: &str) -> bool {
    let claim = normalize(wording);
    if claim.is_empty() {
        return false;
    }
    let words: Vec<&str> = draft_normalized.split_whitespace().collect();
    words
        .windows(4)
        .any(|window| claim.contains(&window.join(" ")))
}

fn claims_in_draft<'a>(draft: &str, claims: &'a [Claim]) -> Vec<&'a Claim> {
    let draft_normalized = normalize(draft);
    claims
        .iter()
        .filter(|claim| claim_appears(&claim.confirmed_wording, &draft_normalized))
        .take(MAX_CLAIMS_IN_PROMPT)
        .collect()
}

fn system_prompt() -> &'static str {
    "The deterministic honesty checks have already run. You are an advisory second reader for scope inflation in one job-application document. For each sentence that relies on a confirmed claim, compare it with that claim's confirmed wording and recorded scope. Report only inflation of scope, ownership, or audience that is visible by comparing the two: a team-level contribution described as company-wide, a supporting role described as ownership, a local result described as an organization result. Do not report missing detail, style, tone, generic risk, or anything you cannot tie to a claim. Every finding must quote the exact sentence from the document and cite the claim id it relies on. Output JSON only, shaped exactly as {\"findings\":[{\"claim_id\":\"...\",\"sentence\":\"...\",\"reason\":\"...\"}]}. Use an empty findings array when nothing qualifies."
}

fn extract_json(raw: &str) -> &str {
    match (raw.find('{'), raw.rfind('}')) {
        (Some(start), Some(end)) if end > start => &raw[start..=end],
        _ => raw,
    }
}

/// Keep only observations that cite a known claim and quote this document.
pub fn parse_observations(raw: &str, claims: &[Claim], draft: &str) -> Vec<ScopeObservation> {
    let parsed: ObservationList = match serde_json::from_str(extract_json(raw)) {
        Ok(parsed) => parsed,
        Err(_) => return Vec::new(),
    };

    let known: HashSet<&str> = claims.iter().map(|claim| claim.id.as_str()).collect();
    let draft_normalized = normalize(draft);
    let mut seen = HashSet::new();
    let mut findings = Vec::new();

    for observation in parsed.findings {
        let claim_id = observation.claim_id.trim().to_string();
        if !known.contains(claim_id.as_str()) {
            continue;
        }
        let sentence = observation
            .sentence
            .trim()
            .trim_matches('"')
            .trim()
            .to_string();
        if sentence.len() < 12 || !draft_normalized.contains(&normalize(&sentence)) {
            continue;
        }
        let reason = observation.reason.trim();
        if reason.is_empty() {
            continue;
        }
        let reason: String = reason.chars().take(400).collect();
        if !seen.insert((claim_id.clone(), normalize(&sentence))) {
            continue;
        }
        findings.push(ScopeObservation {
            claim_id,
            sentence,
            reason,
        });
        if findings.len() >= MAX_FINDINGS {
            break;
        }
    }

    findings
}

/// Convert validated observations into non-blocking guardrail findings.
pub fn advisory_findings(observations: &[ScopeObservation]) -> Vec<GuardrailFinding> {
    observations
        .iter()
        .map(|observation| GuardrailFinding {
            code: "scope_review_advisory".to_string(),
            message: format!(
                "possible scope inflation (claim {}): '{}' - {}",
                observation.claim_id, observation.sentence, observation.reason
            ),
            blocking: false,
        })
        .collect()
}

/// Run the advisory pass. Returns an error string when the model could not be used.
pub async fn review(
    draft: &str,
    claims: &[Claim],
    model: &str,
    num_ctx: Option<u32>,
) -> ScopeReviewOutcome {
    let used = claims_in_draft(draft, claims);
    if used.is_empty() {
        return ScopeReviewOutcome::default();
    }

    let claim_lines = used
        .iter()
        .map(|claim| {
            format!(
                "- {} | {} | scope: {}",
                claim.id, claim.confirmed_wording, claim.scope
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let messages = vec![
        OllamaChatMessage {
            role: "system".to_string(),
            content: system_prompt().to_string(),
        },
        OllamaChatMessage {
            role: "user".to_string(),
            content: format!(
                "Confirmed claims used in this document (id | confirmed wording | recorded scope):\n{}\n\nDocument:\n{}\n\nReturn JSON only.",
                claim_lines, draft
            ),
        },
    ];

    match chat_with_ollama_async(
        model.to_string(),
        messages,
        Some(0.0),
        Some(700),
        Some("json".to_string()),
        num_ctx,
    )
    .await
    {
        Ok(response) => ScopeReviewOutcome {
            findings: parse_observations(&response.message.content, claims, draft),
            ran: true,
            error: None,
        },
        Err(error) => ScopeReviewOutcome {
            findings: Vec::new(),
            ran: false,
            error: Some(error),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{advisory_findings, claims_in_draft, parse_observations};
    use crate::claims::Claim;

    const DRAFT: &str =
        "I supported the migration for one team. It reached about 50% adoption across the team.";

    fn claim(id: &str, wording: &str) -> Claim {
        Claim {
            id: id.to_string(),
            statement: wording.to_string(),
            evidence_source: "resume".to_string(),
            confirmed_wording: wording.to_string(),
            scope: "within the team".to_string(),
            category: "project".to_string(),
            status: "confirmed".to_string(),
            last_confirmed_at: "2026-01-01".to_string(),
            evidence_type: "performance_review".to_string(),
            evidence_sha256: "abc".to_string(),
            verified_by: "Jane".to_string(),
        }
    }

    fn response(claim_id: &str, sentence: &str) -> String {
        format!(
            "{{\"findings\":[{{\"claim_id\":\"{}\",\"sentence\":\"{}\",\"reason\":\"scope widened\"}}]}}",
            claim_id, sentence
        )
    }

    #[test]
    fn keeps_a_cited_observation_that_quotes_the_draft() {
        let claims = vec![claim("claim-1", "Supported the migration for one team")];
        let parsed = parse_observations(
            &response("claim-1", "I supported the migration for one team."),
            &claims,
            DRAFT,
        );

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].claim_id, "claim-1");
        assert_eq!(claims_in_draft(DRAFT, &claims).len(), 1);
    }

    #[test]
    fn drops_an_unknown_claim_id() {
        let claims = vec![claim("claim-1", "Supported the migration for one team")];
        let parsed = parse_observations(
            &response("claim-9", "I supported the migration for one team."),
            &claims,
            DRAFT,
        );

        assert!(parsed.is_empty());
    }

    #[test]
    fn drops_a_sentence_not_in_the_draft() {
        let claims = vec![claim("claim-1", "Supported the migration for one team")];
        let parsed = parse_observations(
            &response("claim-1", "I led the migration across the company."),
            &claims,
            DRAFT,
        );

        assert!(parsed.is_empty());
    }

    #[test]
    fn parses_fenced_json() {
        let claims = vec![claim("claim-1", "Supported the migration for one team")];
        let raw = format!(
            "```json\n{}\n```",
            response("claim-1", "I supported the migration for one team.")
        );

        assert_eq!(parse_observations(&raw, &claims, DRAFT).len(), 1);
    }

    #[test]
    fn findings_are_advisory_only() {
        let claims = vec![claim("claim-1", "Supported the migration for one team")];
        let parsed = parse_observations(
            &response("claim-1", "I supported the migration for one team."),
            &claims,
            DRAFT,
        );
        let findings = advisory_findings(&parsed);

        assert_eq!(findings.len(), 1);
        assert!(!findings[0].blocking);
        assert_eq!(findings[0].code, "scope_review_advisory");
    }
}
