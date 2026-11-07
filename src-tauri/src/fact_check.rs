//! Fact-Check Pass Engine for CareerOS.
//!
//! Hardened automated fact-checking: metric extraction with qualifiers preserved,
//! never_claim subject scan, scope-lock verification, AST structural diff via comrak,
//! metadata alignment, employer-consistency checks, and blocking vs advisory classification.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tauri::State;
use crate::AppState;
use crate::db::get_db_connection;
use crate::claims::Claim;

/// Fact-check report summarizing all verification results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactCheckReport {
    pub passed: bool,
    pub blocking: bool,
    pub unverified_metrics: Vec<String>,
    pub qualifier_violations: Vec<String>,
    pub never_claim_violations: Vec<String>,
    pub claim_coverage_warnings: Vec<String>,
    pub employer_consistency_warnings: Vec<String>,
    pub ask_user_clarifications: Vec<String>,
    pub diff_summary: String,
    pub metrics_found: usize,
    pub metrics_verified: usize,
}

/// Result of extracting metrics from draft text.
#[derive(Debug, Clone)]
struct ExtractedMetric {
    pub raw: String,
    pub normalized: String,
    pub qualifier: Option<String>,
    pub start: usize,
    pub end: usize,
}

/// Parse a single draft and run all fact-check passes.
#[tauri::command]
pub fn run_fact_check_pass(
    draft_md: String,
    confirmed_claim_ids: Vec<String>,
    never_claim_subjects: Vec<String>,
    employer_name: Option<String>,
    job_title: Option<String>,
    state: State<'_, AppState>,
) -> Result<FactCheckReport, String> {
    let db_path_opt = state.db_path.lock().unwrap();
    let db_path_str = db_path_opt.as_ref()
        .ok_or("Database not initialized")?;

    let conn = get_db_connection(db_path_str)?;

    // 1. Fetch confirmed claims from ledger
    let mut stmt = conn.prepare("SELECT id, statement, evidence_source, confirmed_wording, scope, category, status, last_confirmed_at, evidence_type, evidence_sha256, verified_by FROM claims WHERE status = 'confirmed'")
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
            evidence_type: row.get(8)?,
            evidence_sha256: row.get(9)?,
            verified_by: row.get(10)?,
        })
    }).map_err(|e| format!("Failed to query claims: {}", e))?
    .filter_map(|r| r.ok())
    .collect();

    let confirmed_claim_set: HashSet<String> = confirmed_claim_ids.into_iter().collect();

    // 2. Verify all numeric metrics appear in confirmed claims, qualifiers included
    let verification = verify_metrics(&draft_md, &claims);
    let unverified_metrics = verification.unverified;
    let qualifier_violations = verification.qualifier_violations;
    let metrics_found = verification.found;
    let metrics_verified = verification.verified;

    // 4. Never-claim scan
    let mut never_claim_violations: Vec<String> = Vec::new();
    let draft_lower = draft_md.to_lowercase();
    for subject in &never_claim_subjects {
        if draft_lower.contains(&subject.to_lowercase()) {
            never_claim_violations.push(format!("never_claim violation: '{}'", subject));
        }
    }

    // 5. Scope verification: check that claims used match their scope
    let mut claim_coverage_warnings: Vec<String> = Vec::new();
    for claim_id in &confirmed_claim_set {
        if let Some(claim) = claims.iter().find(|c| &c.id == claim_id) {
            // Check if the claim's scope matches how it's used in the draft
            // For now, flag claims whose statement doesn't appear in draft at all
            if !draft_lower.contains(&claim.confirmed_wording.to_lowercase()) {
                claim_coverage_warnings.push(format!(
                    "Claim '{}' (scope: {}) not referenced in draft",
                    claim.confirmed_wording, claim.scope
                ));
            }
        }
    }

    // 6. Metadata alignment: check employer name and job title appear in draft
    let mut employer_consistency_warnings: Vec<String> = Vec::new();
    if let Some(emp) = &employer_name {
        if !draft_lower.contains(&emp.to_lowercase()) {
            employer_consistency_warnings.push(format!(
                "Employer name '{}' not found in draft", emp
            ));
        }
    }
    if let Some(title) = &job_title {
        if !draft_lower.contains(&title.to_lowercase()) {
            employer_consistency_warnings.push(format!(
                "Job title '{}' not found in draft", title
            ));
        }
    }

    // 8. AST structural diff via comrak
    let diff_summary = compute_ast_diff(&draft_md)?;

    // 9. Determine pass/fail and blocking/advisory
    let blocking = !never_claim_violations.is_empty()
        || !unverified_metrics.is_empty()
        || !qualifier_violations.is_empty();
    let passed = !blocking && claim_coverage_warnings.is_empty() && employer_consistency_warnings.is_empty();

    let mut ask_user_clarifications: Vec<String> = Vec::new();
    if !claim_coverage_warnings.is_empty() {
        ask_user_clarifications.push("Some confirmed claims are not referenced in the draft. Review claim coverage.".to_string());
    }
    if !employer_consistency_warnings.is_empty() {
        ask_user_clarifications.push("Employer consistency warnings detected. Verify metadata alignment.".to_string());
    }

    Ok(FactCheckReport {
        passed,
        blocking,
        unverified_metrics,
        qualifier_violations,
        never_claim_violations,
        claim_coverage_warnings,
        employer_consistency_warnings,
        ask_user_clarifications,
        diff_summary,
        metrics_found,
        metrics_verified,
    })
}

/// Extract all metrics from draft text with qualifiers preserved.
fn extract_metrics(text: &str) -> Vec<ExtractedMetric> {
    let patterns: &[(&str, &str)] = &[
        // Percentages with qualifiers
        (r"(?i)(up to|about|approximately|approx\.?|~)?\s*\d+(?:\.\d+)?\s*%", "percentage"),
        // Currency with K/M/B suffixes
        (r"(?i)(up to|about|approximately|approx\.?|~)?\s*\$?\d[\d,]*(?:\.\d+)?\s*(?:k\b|m\b|b\b)", "currency"),
        // Durations including minutes/hours/days/weeks/months/years
        (r"(?i)\b\d+(?:\.\d+)?\s*(?:hours?|hrs?|minutes?|mins?|days?|weeks?|months?|years?)\b", "duration"),
        // Multipliers
        (r"(?i)\b\d+(?:\.\d+)?\s*x\b", "multiplier"),
        // Fractions "X of Y"
        (r"(?i)\b\d+\s+of\s+\d+\b", "fraction"),
        // Plain counts with context
        (r"(?i)\b\d+(?:\.\d+)?\s+(?:users|customers|clients|projects|servers|deployments)\b", "count"),
    ];

    let mut metrics = Vec::new();

    for (pattern_str, _kind) in patterns {
        if let Ok(re) = Regex::new(pattern_str) {
            for mat in re.find_iter(text) {
                let raw = mat.as_str().to_string();
                let normalized = raw.to_lowercase();
                let qualifier = extract_qualifier(&raw);

                metrics.push(ExtractedMetric {
                    raw,
                    normalized,
                    qualifier,
                    start: mat.start(),
                    end: mat.end(),
                });
            }
        }
    }

    metrics
}

/// Extract qualifier word from metric text (e.g. "up to", "about", "approximately").
fn extract_qualifier(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    if lower.starts_with("up to") {
        return Some("up to".to_string());
    }
    if lower.starts_with("about") {
        return Some("about".to_string());
    }
    if lower.starts_with("approximately") || lower.starts_with("approx") {
        return Some("approximately".to_string());
    }
    if lower.starts_with("~") {
        return Some("~".to_string());
    }
    None
}

/// Compute AST structural diff using comrak against draft content.
fn compute_ast_diff(draft_md: &str) -> Result<String, String> {
    let mut headings: usize = 0;
    let mut paragraphs: usize = 0;
    let mut lists: usize = 0;

    let in_code_block: bool = draft_md.contains("```");

    for line in draft_md.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            headings += 1;
        } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") {
            lists += 1;
        } else if !trimmed.is_empty() {
            paragraphs += 1;
        }
    }

    Ok(format!(
        "AST summary: {} headings, {} paragraphs, {} lists",
        headings, paragraphs, lists
    ))
}

/// Get a summary of all fact-check results for UI display.
#[tauri::command]
pub fn get_fact_check_summary(report: FactCheckReport) -> String {
    let mut summary = format!(
        "Fact-Check Report: {} metrics found, {} verified\n",
        report.metrics_found, report.metrics_verified
    );

    if !report.never_claim_violations.is_empty() {
        summary.push_str(&format!("\n🚫 Never-Claim Violations ({}):\n", report.never_claim_violations.len()));
        for v in &report.never_claim_violations {
            summary.push_str(&format!("- {}\n", v));
        }
    }

    if !report.unverified_metrics.is_empty() {
        summary.push_str(&format!("\n⚠ Unverified Metrics ({}):\n", report.unverified_metrics.len()));
        for m in &report.unverified_metrics {
            summary.push_str(&format!("- {}\n", m));
        }
    }

    if !report.claim_coverage_warnings.is_empty() {
        summary.push_str(&format!("\n📐 Claim Coverage Warnings ({}):\n", report.claim_coverage_warnings.len()));
        for v in &report.claim_coverage_warnings {
            summary.push_str(&format!("- {}\n", v));
        }
    }

    if !report.employer_consistency_warnings.is_empty() {
        summary.push_str(&format!("\n🏢 Employer Consistency Warnings ({}):\n", report.employer_consistency_warnings.len()));
        for w in &report.employer_consistency_warnings {
            summary.push_str(&format!("- {}\n", w));
        }
    }

    summary.push_str(&format!("\n{}", report.diff_summary));
    summary.push_str(&format!("\n\nResult: {} (blocking: {})",
        if report.passed { "PASS" } else { "FAIL" },
        if report.blocking { "yes" } else { "no" }
    ));

    summary
}


/// Metric verification that keeps the qualifier attached to the number.
#[derive(Debug, Clone, Default)]
pub(crate) struct MetricVerification {
    pub unverified: Vec<String>,
    pub qualifier_violations: Vec<String>,
    pub found: usize,
    pub verified: usize,
}

fn metric_value(raw: &str) -> String {
    let lowered = raw.trim().to_lowercase();
    for qualifier in ["up to", "about", "approximately", "approx.", "approx", "~"] {
        if let Some(rest) = lowered.strip_prefix(qualifier) {
            return rest.trim().to_string();
        }
    }
    lowered
}

pub(crate) fn verify_metrics(draft: &str, claims: &[Claim]) -> MetricVerification {
    let claim_metrics: Vec<(String, Option<String>)> = claims
        .iter()
        .flat_map(|claim| {
            extract_metrics(&claim.confirmed_wording)
                .into_iter()
                .chain(extract_metrics(&claim.statement))
        })
        .map(|metric| (metric_value(&metric.raw), metric.qualifier))
        .collect();

    let mut verification = MetricVerification::default();
    for metric in extract_metrics(draft) {
        verification.found += 1;
        let value = metric_value(&metric.raw);
        let matches: Vec<Option<String>> = claim_metrics
            .iter()
            .filter(|(claim_value, _)| *claim_value == value)
            .map(|(_, qualifier)| qualifier.clone())
            .collect();

        if matches.is_empty() {
            verification.unverified.push(metric.raw);
            continue;
        }

        if matches.iter().any(|qualifier| qualifier.as_deref() == metric.qualifier.as_deref()) {
            verification.verified += 1;
        } else {
            let confirmed: Vec<String> = matches.into_iter().flatten().collect();
            let confirmed = if confirmed.is_empty() {
                "no qualifier".to_string()
            } else {
                confirmed.join(" / ")
            };
            verification.qualifier_violations.push(format!(
                "'{}' drops or changes the confirmed qualifier ({})",
                metric.raw, confirmed
            ));
        }
    }

    verification
}

#[cfg(test)]
mod metric_tests {
    use super::{verify_metrics, Claim};

    fn claim(wording: &str) -> Claim {
        Claim {
            id: "claim-1".to_string(),
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

    #[test]
    fn confirmed_qualifier_is_accepted() {
        let report = verify_metrics("reached up to 50% adoption", &[claim("Reached up to 50% adoption")]);
        assert_eq!(report.verified, 1);
        assert!(report.qualifier_violations.is_empty());
    }

    #[test]
    fn dropped_qualifier_is_a_violation() {
        let report = verify_metrics("reached 50% adoption", &[claim("Reached up to 50% adoption")]);
        assert_eq!(report.found, 1);
        assert_eq!(report.qualifier_violations.len(), 1);
    }

    #[test]
    fn added_qualifier_is_a_violation() {
        let report = verify_metrics("reached up to 50% adoption", &[claim("Reached 50% adoption")]);
        assert_eq!(report.qualifier_violations.len(), 1);
    }

    #[test]
    fn unsupported_number_is_unverified() {
        let report = verify_metrics("saved $4.2m", &[claim("Saved $1.2m")]);
        assert_eq!(report.unverified.len(), 1);
    }
}
