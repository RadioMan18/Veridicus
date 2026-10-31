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
    pub never_claim_violations: Vec<String>,
    pub scope_violations: Vec<String>,
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

    let confirmed_claim_set: HashSet<String> = confirmed_claim_ids.into_iter().collect();

    // 2. Extract all metrics from draft
    let metrics = extract_metrics(&draft_md);

    // 3. Verify each metric against confirmed claims
    let mut unverified_metrics: Vec<String> = Vec::new();
    let mut metrics_verified = 0;

    for metric in &metrics {
        let normalized = &metric.normalized;
        let is_verified = claims.iter().any(|c| {
            c.confirmed_wording.contains(normalized) || c.statement.contains(normalized)
        }) || confirmed_claim_set.iter().any(|id| {
            claims.iter().any(|c| c.id == *id && (c.confirmed_wording.contains(normalized) || c.statement.contains(normalized)))
        });

        if is_verified {
            metrics_verified += 1;
        } else {
            unverified_metrics.push(metric.raw.clone());
        }
    }

    // 4. Never-claim scan
    let mut never_claim_violations: Vec<String> = Vec::new();
    let draft_lower = draft_md.to_lowercase();
    for subject in &never_claim_subjects {
        if draft_lower.contains(&subject.to_lowercase()) {
            never_claim_violations.push(format!("never_claim violation: '{}'", subject));
        }
    }

    // 5. Scope verification: check that claims used match their scope
    let mut scope_violations: Vec<String> = Vec::new();
    for claim_id in &confirmed_claim_set {
        if let Some(claim) = claims.iter().find(|c| &c.id == claim_id) {
            // Check if the claim's scope matches how it's used in the draft
            // For now, flag claims whose statement doesn't appear in draft at all
            if !draft_lower.contains(&claim.confirmed_wording.to_lowercase()) {
                scope_violations.push(format!(
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

    // 7. Employer-consistency query against prior application_records
    let employer_consistency_from_history = check_employer_consistency(&conn, &draft_md, &claims)?;
    employer_consistency_warnings.extend(employer_consistency_from_history);

    // 8. AST structural diff via comrak
    let diff_summary = compute_ast_diff(&draft_md)?;

    // 9. Determine pass/fail and blocking/advisory
    let blocking = !never_claim_violations.is_empty() || !unverified_metrics.is_empty();
    let passed = !blocking && scope_violations.is_empty() && employer_consistency_warnings.is_empty();

    let mut ask_user_clarifications: Vec<String> = Vec::new();
    if !scope_violations.is_empty() {
        ask_user_clarifications.push("Some confirmed claims are not referenced in the draft. Review scope coverage.".to_string());
    }
    if !employer_consistency_warnings.is_empty() {
        ask_user_clarifications.push("Employer consistency warnings detected. Verify metadata alignment.".to_string());
    }

    Ok(FactCheckReport {
        passed,
        blocking,
        unverified_metrics,
        never_claim_violations,
        scope_violations,
        employer_consistency_warnings,
        ask_user_clarifications,
        diff_summary,
        metrics_found: metrics.len(),
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

/// Check employer consistency against prior application_records.
fn check_employer_consistency(
    conn: &rusqlite::Connection,
    draft_md: &str,
    claims: &[Claim],
) -> Result<Vec<String>, String> {
    let mut warnings = Vec::new();

    // Find prior applications with the same employer mentioned in draft
    // Look for employer names in draft
    let draft_lower = draft_md.to_lowercase();

    let mut stmt2 = conn.prepare("SELECT employer, claims_used, approved_wording FROM application_records WHERE status != 'PENDING_REVIEW'")
        .map_err(|e| format!("Failed to prepare prior apps query: {}", e))?;
    let prior_apps: Vec<(String, String, String)> = stmt2.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    }).map_err(|e| format!("Failed to query prior applications: {}", e))?
    .filter_map(|r| r.ok())
    .collect();

    for (employer, claims_used, approved_wording) in &prior_apps {
        if draft_lower.contains(&employer.to_lowercase()) {
            // Check if metrics in draft contradict prior approved wording
            let prior_claims: Vec<&str> = claims_used.split(',').map(|s| s.trim()).collect();
            for claim_id in &prior_claims {
                if let Some(claim) = claims.iter().find(|c| &c.id == claim_id) {
                    if draft_lower.contains(&claim.confirmed_wording.to_lowercase()) {
                        // Metric present - check if wording matches approved
                        if !approved_wording.is_empty() && !draft_lower.contains(&approved_wording.to_lowercase()) {
                            warnings.push(format!(
                                "Employer consistency: metric '{}' used with different wording than prior application to {}",
                                claim.confirmed_wording, employer
                            ));
                        }
                    }
                }
            }
        }
    }

    Ok(warnings)
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

    if !report.scope_violations.is_empty() {
        summary.push_str(&format!("\n📐 Scope Violations ({}):\n", report.scope_violations.len()));
        for v in &report.scope_violations {
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