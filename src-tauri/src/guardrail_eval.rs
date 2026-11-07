//! Adversarial evaluation harness for the honesty guardrails.
//!
//! The unit tests beside each check prove the check works on its own. These
//! fixtures prove the assembled gate still blocks the failure modes that matter
//! when a document is evaluated end to end. They are the TEVV evidence for
//! NIST AI RMF MEASURE 2.1/2.3 and ISO/IEC 42001 A.6.2.4.

use serde::Deserialize;

use crate::claims::Claim;
use crate::guardrails::{evaluate_document, RequiredGap, RuleView};

#[derive(Debug, Deserialize)]
struct Case {
    id: String,
    kind: String,
    draft: String,
    #[serde(default)]
    claims: Vec<CaseClaim>,
    #[serde(default)]
    rules: Vec<CaseRule>,
    #[serde(default)]
    gaps: Vec<CaseGap>,
    employer: Option<String>,
    role: Option<String>,
    expect_blocking: bool,
    #[serde(default)]
    expect_codes: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CaseClaim {
    id: String,
    wording: String,
}

#[derive(Debug, Deserialize)]
struct CaseRule {
    kind: String,
    subject: String,
    #[serde(default)]
    rule_text: String,
}

#[derive(Debug, Deserialize)]
struct CaseGap {
    requirement: String,
    #[serde(default)]
    adjacent_strength: String,
}

fn claim(case: &CaseClaim) -> Claim {
    Claim {
        id: case.id.clone(),
        statement: case.wording.clone(),
        evidence_source: "eval fixture".to_string(),
        confirmed_wording: case.wording.clone(),
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
fn guardrail_fixtures_produce_their_expected_findings() {
    let cases: Vec<Case> = serde_json::from_str(include_str!("../eval/guardrails.json"))
        .expect("guardrail fixtures must parse");
    let mut failures = Vec::new();

    for case in &cases {
        let claims: Vec<Claim> = case.claims.iter().map(claim).collect();
        let rules: Vec<RuleView> = case
            .rules
            .iter()
            .map(|rule| RuleView {
                kind: rule.kind.clone(),
                subject: rule.subject.clone(),
                rule_text: rule.rule_text.clone(),
                applies_to: "all".to_string(),
            })
            .collect();
        let gaps: Vec<RequiredGap> = case
            .gaps
            .iter()
            .map(|gap| RequiredGap {
                id: format!("gap-{}", case.id),
                requirement: gap.requirement.clone(),
                requirement_tier: "must_have".to_string(),
                adjacent_strength: if gap.adjacent_strength.is_empty() {
                    "adjacent work".to_string()
                } else {
                    gap.adjacent_strength.clone()
                },
                status: "open".to_string(),
            })
            .collect();

        let report = evaluate_document(
            &case.kind,
            &case.draft,
            &claims,
            &rules,
            case.employer.as_deref(),
            case.role.as_deref(),
            &gaps,
        );

        if report.blocking != case.expect_blocking {
            failures.push(format!(
                "{}: expected blocking={}, got {} ({:?})",
                case.id, case.expect_blocking, report.blocking, report.findings
            ));
        }
        for code in &case.expect_codes {
            if !report.findings.iter().any(|finding| &finding.code == code) {
                failures.push(format!("{}: missing expected finding '{}'", case.id, code));
            }
        }
        if !case.expect_blocking && !report.findings.is_empty() {
            failures.push(format!(
                "{}: clean fixture produced findings {:?}",
                case.id, report.findings
            ));
        }
    }

    println!(
        "guardrail eval: {} cases, {} failures",
        cases.len(),
        failures.len()
    );
    assert!(
        failures.is_empty(),
        "guardrail evaluation failures:\n{}",
        failures.join("\n")
    );
}
