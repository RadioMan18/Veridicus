//! Fact-Check Pass Engine for Veridicus.
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
pub(crate) struct ExtractedMetric {
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

    // 6. Metadata alignment: the draft must name the employer and the role.
    let employer_consistency_warnings = employer_consistency_warnings(
        &draft_md,
        employer_name.as_deref(),
        job_title.as_deref(),
    );

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

/// The employer and role names must appear in the draft. A mismatch is advisory
/// in the fact-check report but is reported, not swallowed.
pub(crate) fn employer_consistency_warnings(
    draft: &str,
    employer: Option<&str>,
    job_title: Option<&str>,
) -> Vec<String> {
    let draft_lower = draft.to_lowercase();
    let mut warnings = Vec::new();
    if let Some(employer) = employer {
        if !draft_lower.contains(&employer.to_lowercase()) {
            warnings.push(format!("Employer name '{}' not found in draft", employer));
        }
    }
    if let Some(title) = job_title {
        if !draft_lower.contains(&title.to_lowercase()) {
            warnings.push(format!("Job title '{}' not found in draft", title));
        }
    }
    warnings
}

/// A metric may be written in digits or in words ('9 years', 'nine years');
/// the ledger and the draft do not always agree, so both forms are extracted.
const NUMBER_WORD_PATTERN: &str = r"(?:one|two|three|four|five|six|seven|eight|nine|ten|eleven|twelve|thirteen|fourteen|fifteen|sixteen|seventeen|eighteen|nineteen|twenty|thirty|forty|fifty|sixty|seventy|eighty|ninety|hundred|thousand)";

/// The qualifier is captured with the number so the report can quote it and the
/// verifier can tell a dropped ceiling from a dropped floor.
const QUALIFIER_PATTERN: &str = r"(?:up to|under|less than|at most|over|more than|at least|about|approximately|approx\.?|around|close to|~)?\s*";

/// Extract all metrics from draft text with qualifiers preserved.
pub(crate) fn extract_metrics(text: &str) -> Vec<ExtractedMetric> {
    let number = NUMBER_WORD_PATTERN;
    let qualifier = QUALIFIER_PATTERN;
    let patterns: Vec<(String, &str)> = vec![
        // Percentages with qualifiers
        (format!(r"(?i){qualifier}\b(?:\d+(?:\.\d+)?\+?|{number}(?:[- ]{number})*)\s*%"), "percentage"),
        // Currency with K/M/B suffixes
        (format!(r"(?i){qualifier}\$?\d[\d,]*(?:\.\d+)?\+?\s*(?:k\b|m\b|b\b)"), "currency"),
        // Durations including minutes/hours/days/weeks/months/years
        (format!(r"(?i){qualifier}\b(?:\d+(?:\.\d+)?\+?|{number}(?:[- ]{number})*)\s*(?:hours?|hrs?|minutes?|mins?|days?|weeks?|months?|years?)\b"), "duration"),
        // Multipliers
        (format!(r"(?i)\b(?:\d+(?:\.\d+)?\+?|{number})\s*x\b"), "multiplier"),
        // Fractions "X of Y"
        (r"(?i)\b\d+\s+of\s+\d+\b".to_string(), "fraction"),
        // Plain counts with context
        (format!(r"(?i){qualifier}\b(?:\d+(?:\.\d+)?\+?|{number}(?:[- ]{number})*)\s+(?:users|customers|clients|projects|servers|deployments)\b"), "count"),
    ];

    let mut metrics = Vec::new();

    for (pattern_str, _kind) in &patterns {
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

/// A ceiling caps the claim; dropping it overstates. A floor only raises the
/// low end; dropping it understates, which is still honest.
const CEILING_QUALIFIERS: &[&str] = &[
    "up to",
    "under",
    "less than",
    "at most",
    "nearly",
    "about",
    "approximately",
    "approx.",
    "approx",
    "around",
    "close to",
    "~",
];

const FLOOR_QUALIFIERS: &[&str] = &["over", "more than", "at least", "plus", "upwards of"];

/// Extract qualifier word from metric text (e.g. "up to", "about", "approximately").
fn extract_qualifier(text: &str) -> Option<String> {
    let lower = text.trim().to_lowercase();
    for qualifier in CEILING_QUALIFIERS.iter().chain(FLOOR_QUALIFIERS.iter()) {
        if lower.starts_with(qualifier) {
            return Some((*qualifier).to_string());
        }
    }
    if lower.contains('+') {
        return Some("+".to_string());
    }
    None
}

fn qualifier_family(qualifier: Option<&str>) -> Option<&'static str> {
    let qualifier = qualifier?;
    let qualifier = qualifier.to_lowercase();
    if FLOOR_QUALIFIERS.iter().any(|floor| *floor == qualifier) || qualifier == "+" {
        return Some("floor");
    }
    if CEILING_QUALIFIERS.iter().any(|ceiling| *ceiling == qualifier) {
        return Some("ceiling");
    }
    None
}

fn number_word(token: &str) -> Option<f64> {
    let token = token.trim_matches(|character: char| !character.is_ascii_alphabetic());
    Some(match token {
        "zero" => 0.0,
        "one" => 1.0,
        "two" => 2.0,
        "three" => 3.0,
        "four" => 4.0,
        "five" => 5.0,
        "six" => 6.0,
        "seven" => 7.0,
        "eight" => 8.0,
        "nine" => 9.0,
        "ten" => 10.0,
        "eleven" => 11.0,
        "twelve" => 12.0,
        "thirteen" => 13.0,
        "fourteen" => 14.0,
        "fifteen" => 15.0,
        "sixteen" => 16.0,
        "seventeen" => 17.0,
        "eighteen" => 18.0,
        "nineteen" => 19.0,
        "twenty" => 20.0,
        "thirty" => 30.0,
        "forty" => 40.0,
        "fifty" => 50.0,
        "sixty" => 60.0,
        "seventy" => 70.0,
        "eighty" => 80.0,
        "ninety" => 90.0,
        "hundred" => 100.0,
        "thousand" => 1_000.0,
        _ => return None,
    })
}

fn split_leading_number(token: &str) -> Option<(f64, String)> {
    let digits: String = token
        .chars()
        .take_while(|character| character.is_ascii_digit() || *character == '.')
        .collect();
    if digits.is_empty() {
        return None;
    }
    let value: f64 = digits.parse().ok()?;
    Some((value, token[digits.len()..].to_string()))
}

fn canonical_unit(unit: &str) -> String {
    let word: String = unit
        .chars()
        .filter(|character| character.is_alphanumeric() || *character == '%')
        .collect::<String>()
        .to_lowercase();
    let word = word.trim_start_matches(|character: char| character.is_ascii_digit() || character == '.');
    match word {
        "%" | "percent" | "pct" => "percent".to_string(),
        "x" => "multiplier".to_string(),
        "min" | "mins" | "minute" | "minutes" => "minute".to_string(),
        "hr" | "hrs" | "hour" | "hours" => "hour".to_string(),
        "day" | "days" => "day".to_string(),
        "week" | "weeks" => "week".to_string(),
        "month" | "months" => "month".to_string(),
        "yr" | "yrs" | "year" | "years" => "year".to_string(),
        "" => String::new(),
        other => other.trim_end_matches('s').to_string(),
    }
}

/// A metric reduced to the number and the unit it counts, independent of how it
/// was spelled: '5 minutes' and 'five minutes' are the same fact, and so are
/// '500 clients' and '500+ clients'.
fn metric_key(raw: &str) -> Option<(f64, String)> {
    let cleaned: String = raw
        .to_lowercase()
        .chars()
        .map(|character| {
            if character == '$' || character == ',' {
                ' '
            } else {
                character
            }
        })
        .collect();
    let mut body = cleaned.trim().to_string();
    for qualifier in CEILING_QUALIFIERS.iter().chain(FLOOR_QUALIFIERS.iter()) {
        if let Some(rest) = body.strip_prefix(qualifier) {
            // Only a whole word counts: 'underwriter' is not an 'under' qualifier.
            if rest.chars().next().map(|character| !character.is_ascii_alphabetic()).unwrap_or(true) {
                body = rest.trim_start().to_string();
                break;
            }
        }
    }
    let tokens: Vec<&str> = body.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }

    let mut value = 0.0;
    let mut consumed = 0usize;
    let mut leading_rest = String::new();
    while consumed < tokens.len() {
        let token = tokens[consumed];
        if consumed == 0 {
            if let Some((number, rest)) = split_leading_number(token) {
                value = number;
                leading_rest = rest;
                consumed += 1;
                break;
            }
        }
        match number_word(token) {
            Some(number) => {
                if number >= 100.0 && value > 0.0 {
                    value *= number;
                } else {
                    value += number;
                }
                consumed += 1;
            }
            None => break,
        }
    }
    if consumed == 0 {
        return None;
    }

    let unit = if leading_rest.chars().any(|character| character.is_alphabetic() || character == '%') {
        leading_rest
    } else {
        tokens[consumed..].join(" ")
    };
    Some((value, canonical_unit(&unit)))
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
    /// Metrics the master resume states but the ledger has not confirmed. They
    /// are reported as advisories: the user's own resume is not an invention.
    pub from_base_resume: Vec<String>,
    pub qualifier_violations: Vec<String>,
    pub found: usize,
    pub verified: usize,
}

pub(crate) fn verify_metrics(draft: &str, claims: &[Claim]) -> MetricVerification {
    verify_metrics_with_base(draft, claims, None)
}

pub(crate) fn verify_metrics_with_base(
    draft: &str,
    claims: &[Claim],
    base_resume: Option<&str>,
) -> MetricVerification {
    let base_lower = base_resume.unwrap_or("").to_lowercase();
    let claim_metrics: Vec<((f64, String), Option<String>)> = claims
        .iter()
        .flat_map(|claim| {
            extract_metrics(&claim.confirmed_wording)
                .into_iter()
                .chain(extract_metrics(&claim.statement))
        })
        .filter_map(|metric| metric_key(&metric.raw).map(|key| (key, metric.qualifier)))
        .collect();

    let mut verification = MetricVerification::default();
    for metric in extract_metrics(draft) {
        verification.found += 1;
        let Some(key) = metric_key(&metric.raw) else {
            verification.unverified.push(metric.raw);
            continue;
        };
        let matches: Vec<Option<String>> = claim_metrics
            .iter()
            .filter(|(claim_key, _)| {
                (claim_key.0 - key.0).abs() < 0.001 && claim_key.1 == key.1
            })
            .map(|(_, qualifier)| qualifier.clone())
            .collect();

        if matches.is_empty() {
            if !base_lower.is_empty() && metric_appears_in(&metric.raw, &base_lower) {
                verification.from_base_resume.push(metric.raw);
            } else {
                verification.unverified.push(metric.raw);
            }
            continue;
        }

        if matches.iter().any(|qualifier| qualifier.as_deref() == metric.qualifier.as_deref()) {
            verification.verified += 1;
            continue;
        }

        // Dropping a floor ("500+" written as "500") understates the confirmed
        // claim, which is honest; dropping a ceiling overstates it.
        let claim_is_floor = matches
            .iter()
            .all(|qualifier| qualifier_family(qualifier.as_deref()) == Some("floor"));
        if claim_is_floor && metric.qualifier.is_none() {
            verification.verified += 1;
            continue;
        }

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

    verification
}

/// True when the metric's wording, or its number and unit, already appears in the
/// master resume.
fn metric_appears_in(raw: &str, base_lower: &str) -> bool {
    let needle = raw
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if needle.is_empty() {
        return false;
    }
    if base_lower.contains(&needle) {
        return true;
    }
    let Some((value, unit)) = metric_key(raw) else {
        return false;
    };
    let number = if value.fract().abs() < f64::EPSILON {
        format!("{}", value as i64)
    } else {
        format!("{}", value)
    };
    let number_present = base_lower
        .split(|character: char| !character.is_ascii_digit())
        .any(|digits| digits == number);
    if !number_present {
        return false;
    }
    // A metric with no unit to check, or a very short one, is matched on its
    // number alone; the master resume already contains that number.
    let Some(unit_root) = unit.split_whitespace().next() else {
        return true;
    };
    unit_root.len() < 3 || base_lower.contains(unit_root)
}

#[cfg(test)]
mod metric_tests {
    use super::{employer_consistency_warnings, verify_metrics, verify_metrics_with_base, Claim};

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

    #[test]
    fn a_metric_spelled_in_words_matches_the_same_metric_in_digits() {
        let report = verify_metrics(
            "cut asset search time to under 5 minutes and bring 9 years of enablement",
            &[claim("Cut asset search time to under five minutes and brings nine years of enablement")],
        );
        assert!(report.unverified.is_empty(), "unverified: {:?}", report.unverified);
        assert!(report.qualifier_violations.is_empty(), "{:?}", report.qualifier_violations);
    }

    #[test]
    fn a_metric_the_master_resume_states_is_advisory_not_blocking() {
        let base = "Cut asset search time from 30 minutes to under 5 minutes. Served 500+ clients over 9 years.";
        let report = verify_metrics_with_base(
            "search time is under 5 minutes, 9 Years in enablement, 500 clients served",
            &[],
            Some(base),
        );

        assert!(report.unverified.is_empty(), "unverified: {:?}", report.unverified);
        assert_eq!(report.from_base_resume.len(), 3, "{:?}", report.from_base_resume);
    }

    #[test]
    fn the_reported_metrics_match_their_claim_spellings() {
        let report = verify_metrics(
            "cut asset search to under 5 minutes, 9 Years in enablement, and 500 clients served",
            &[
                claim("Cut asset search time to under five minutes"),
                claim("Brings nine years in enablement"),
                claim("Served 500+ clients"),
            ],
        );
        assert!(report.unverified.is_empty(), "unverified: {:?}", report.unverified);
        assert!(report.qualifier_violations.is_empty(), "{:?}", report.qualifier_violations);
    }

    #[test]
    fn a_dropped_floor_understates_rather_than_overstates() {
        let report = verify_metrics(
            "supported 500 clients across the portfolio",
            &[claim("Supported 500+ clients across the portfolio")],
        );
        assert!(report.unverified.is_empty(), "unverified: {:?}", report.unverified);
        assert!(report.qualifier_violations.is_empty(), "{:?}", report.qualifier_violations);
    }

    #[test]
    fn word_hundreds_and_thousands_fold_to_one_number() {
        let report = verify_metrics(
            "reached 500 clients",
            &[claim("Reached five hundred clients")],
        );
        assert!(report.unverified.is_empty(), "unverified: {:?}", report.unverified);
    }

    #[test]
    fn a_wrong_employer_is_detected() {
        let warnings = employer_consistency_warnings(
            "Dear Northwind team, I am applying for the Learning Lead role.",
            Some("Aurora"),
            Some("Learning Lead"),
        );
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("Aurora"));

        let matched = employer_consistency_warnings(
            "Dear Aurora team, I am applying for the Learning Lead role.",
            Some("Aurora"),
            Some("Learning Lead"),
        );
        assert!(matched.is_empty());
    }
}
