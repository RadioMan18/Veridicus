//! Evidence and audit support for the honesty guardrails.
//!
//! Each application gets an append-only `audit/events.jsonl`. An event records what
//! was generated, with which model digest and rule set, what the guardrail gate
//! decided, and what the user approved or overrode. Nothing here overwrites an
//! earlier event; the file is the evidence trail that NIST MEASURE 2.8 and
//! ISO/IEC 42001 A.6.2.8 ask for.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::guardrails::{GuardrailFinding, RuleView};

pub const AUDIT_DIR: &str = "audit";
pub const EVENTS_FILE: &str = "events.jsonl";
pub const PROBLEMS_FILE: &str = "reported-problems.jsonl";
pub const COMPLIANCE_DIR: &str = "compliance";
pub const REVIEWS_FILE: &str = "reviews.jsonl";
pub const DISCLOSURES_FILE: &str = "disclosures.jsonl";

/// Cadence at which the honesty record is reviewed.
pub const REVIEW_CADENCE_DAYS: i64 = 90;
/// Bump when the disclosure text or intended-use statement changes.
pub const DISCLOSURE_VERSION: &str = "1";
pub const DISCLOSURE_TEXT: &str = "Veridicus is an AI-assisted drafting tool. It drafts documents with a local model against claims you confirmed. Confirmation means you verified the claim, not that Veridicus verified it. Veridicus does not submit applications. You remain responsible for reviewing every document and for what you submit.";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentDigest {
    pub name: String,
    pub sha256: String,
    pub bytes: usize,
}

/// One model call behind a generated document. A reply that stopped at `length`
/// lost its tail, so these numbers make a document that ran close to its budget
/// visible after the fact instead of only in a failure message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationAttempt {
    pub prompt_tokens: Option<u32>,
    pub reply_tokens: Option<u32>,
    pub done_reason: Option<String>,
    /// Whether this attempt's text is the document that was kept.
    #[serde(default)]
    pub kept: bool,
}

/// What one generated document cost, and how many attempts it took.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentUsage {
    pub skill_id: String,
    /// The reply budget each attempt was given.
    pub reply_limit: u32,
    pub attempts: Vec<GenerationAttempt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub event_id: String,
    pub timestamp: String,
    pub app_version: String,
    pub application_id: String,
    pub event: String,
    pub model: Option<String>,
    pub model_digest: Option<String>,
    pub model_digest_at_commit: Option<String>,
    pub template_version: u32,
    pub rule_ids: Vec<String>,
    pub rules_sha256: String,
    pub documents: Vec<DocumentDigest>,
    pub blocking: bool,
    pub findings: Vec<GuardrailFinding>,
    pub advisory_count: usize,
    pub override_reason: Option<String>,
    pub note: Option<String>,
    /// Token use per generated document. Absent from events written before this
    /// was recorded, so reading old trails stays possible.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub generation_usage: Vec<DocumentUsage>,
}

fn default_incident_status() -> String {
    "open".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProblemReport {
    #[serde(default)]
    pub incident_id: String,
    pub timestamp: String,
    pub application_id: String,
    pub category: String,
    pub note: String,
    #[serde(default = "default_incident_status")]
    pub status: String,
    pub app_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewRecord {
    pub review_id: String,
    pub timestamp: String,
    pub reviewer: String,
    pub note: String,
    pub applications_covered: usize,
    pub blocking_findings: usize,
    pub overrides: usize,
    pub false_block_reports: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisclosureRecord {
    pub timestamp: String,
    pub application_id: Option<String>,
    pub version: String,
    pub context: String,
    pub app_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HonestyStats {
    pub applications: usize,
    pub generated: usize,
    pub approved: usize,
    pub overrides: usize,
    pub blocked_findings: usize,
    pub advisory_findings: usize,
    pub by_code: BTreeMap<String, usize>,
    pub blocks_by_code: BTreeMap<String, usize>,
    pub false_block_reports: usize,
    pub override_rate: f64,
    pub last_review_at: Option<String>,
    pub next_review_due: Option<String>,
    pub review_cadence_days: i64,
    pub review_overdue: bool,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn digest_text(name: &str, text: &str) -> DocumentDigest {
    DocumentDigest {
        name: name.to_string(),
        sha256: sha256_hex(text.as_bytes()),
        bytes: text.len(),
    }
}

/// Digest the rendered materials that are actually on disk.
pub fn document_digests(app_dir: &Path) -> Vec<DocumentDigest> {
    ["resume.md", "cover-letter.md"]
        .iter()
        .filter_map(|name| {
            fs::read(app_dir.join(name)).ok().map(|bytes| DocumentDigest {
                name: name.to_string(),
                sha256: sha256_hex(&bytes),
                bytes: bytes.len(),
            })
        })
        .collect()
}

/// A single version for the pair of materials, derived from their content.
pub fn materials_version(app_dir: &Path) -> String {
    let mut hasher = Sha256::new();
    for name in ["resume.md", "cover-letter.md"] {
        if let Ok(bytes) = fs::read(app_dir.join(name)) {
            hasher.update(name.as_bytes());
            hasher.update(&bytes);
        }
    }
    format!("{:x}", hasher.finalize())
}

/// Stable fingerprint of the rules that were active for a run.
pub fn rules_fingerprint(rules: &[RuleView]) -> (Vec<String>, String) {
    let mut entries: Vec<(String, String, String, String)> = rules
        .iter()
        .map(|rule| {
            (
                rule.kind.clone(),
                rule.subject.clone(),
                rule.rule_text.clone(),
                rule.applies_to.clone(),
            )
        })
        .collect();
    entries.sort();
    entries.dedup();
    let ids = entries
        .iter()
        .map(|(kind, subject, _, _)| format!("{}:{}", kind, subject))
        .collect();
    let canonical = serde_json::to_string(&entries).unwrap_or_default();
    (ids, sha256_hex(canonical.as_bytes()))
}

pub fn append_event(app_dir: &Path, event: &AuditEvent) -> Result<(), String> {
    let audit_dir = app_dir.join(AUDIT_DIR);
    fs::create_dir_all(&audit_dir)
        .map_err(|error| format!("Failed to create audit directory: {}", error))?;
    let line = serde_json::to_string(event)
        .map_err(|error| format!("Failed to serialize audit event: {}", error))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(audit_dir.join(EVENTS_FILE))
        .map_err(|error| format!("Failed to open audit file: {}", error))?;
    writeln!(file, "{}", line).map_err(|error| format!("Failed to append audit event: {}", error))
}

pub fn read_events(app_dir: &Path) -> Vec<AuditEvent> {
    fs::read_to_string(app_dir.join(AUDIT_DIR).join(EVENTS_FILE))
        .map(|contents| {
            contents
                .lines()
                .filter_map(|line| serde_json::from_str::<AuditEvent>(line).ok())
                .collect()
        })
        .unwrap_or_default()
}

pub fn last_generation(app_dir: &Path) -> Option<AuditEvent> {
    read_events(app_dir)
        .into_iter()
        .rev()
        .find(|event| event.event == "generated")
}

/// A per-application disclosure acknowledgement is only valid for the generation
/// it follows. A new generation requires a fresh acknowledgement.
pub fn has_output_disclosure(app_dir: &Path) -> bool {
    let events = read_events(app_dir);
    let Some(generation) = events.iter().rposition(|event| event.event == "generated") else {
        return false;
    };
    events[generation + 1..].iter().any(|event| {
        event.event == "disclosure_acknowledged"
            && event.note.as_deref() == Some(DISCLOSURE_VERSION)
    })
}

pub fn append_problem(app_dir: &Path, report: &ProblemReport) -> Result<(), String> {
    fs::create_dir_all(app_dir)
        .map_err(|error| format!("Failed to create application directory: {}", error))?;
    let line = serde_json::to_string(report)
        .map_err(|error| format!("Failed to serialize problem report: {}", error))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(app_dir.join(PROBLEMS_FILE))
        .map_err(|error| format!("Failed to open problem report file: {}", error))?;
    writeln!(file, "{}", line)
        .map_err(|error| format!("Failed to append problem report: {}", error))
}

pub fn all_problem_reports(workspace_root: &Path) -> Vec<ProblemReport> {
    let applications = workspace_root.join("applications");
    let Ok(entries) = fs::read_dir(applications) else {
        return Vec::new();
    };
    let mut reports = Vec::new();
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        if let Ok(contents) = fs::read_to_string(entry.path().join(PROBLEMS_FILE)) {
            reports.extend(
                contents
                    .lines()
                    .filter_map(|line| serde_json::from_str::<ProblemReport>(line).ok()),
            );
        }
    }
    reports
}

pub fn append_review(workspace_root: &Path, review: &ReviewRecord) -> Result<(), String> {
    let dir = workspace_root.join(COMPLIANCE_DIR);
    fs::create_dir_all(&dir)
        .map_err(|error| format!("Failed to create compliance directory: {}", error))?;
    let line = serde_json::to_string(review)
        .map_err(|error| format!("Failed to serialize review record: {}", error))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(REVIEWS_FILE))
        .map_err(|error| format!("Failed to open review record file: {}", error))?;
    writeln!(file, "{}", line).map_err(|error| format!("Failed to append review: {}", error))
}

pub fn read_reviews(workspace_root: &Path) -> Vec<ReviewRecord> {
    fs::read_to_string(workspace_root.join(COMPLIANCE_DIR).join(REVIEWS_FILE))
        .map(|contents| {
            contents
                .lines()
                .filter_map(|line| serde_json::from_str::<ReviewRecord>(line).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Append a disclosure acknowledgement to the workspace-level record.
pub fn append_disclosure(
    workspace_root: &Path,
    application_id: Option<&str>,
    context: &str,
) -> Result<DisclosureRecord, String> {
    let dir = workspace_root.join(COMPLIANCE_DIR);
    fs::create_dir_all(&dir)
        .map_err(|error| format!("Failed to create compliance directory: {}", error))?;
    let record = DisclosureRecord {
        timestamp: Utc::now().to_rfc3339(),
        application_id: application_id.map(str::to_string),
        version: DISCLOSURE_VERSION.to_string(),
        context: context.to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
    };
    let line = serde_json::to_string(&record)
        .map_err(|error| format!("Failed to serialize disclosure record: {}", error))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(DISCLOSURES_FILE))
        .map_err(|error| format!("Failed to open disclosure record file: {}", error))?;
    writeln!(file, "{}", line)
        .map_err(|error| format!("Failed to append disclosure record: {}", error))?;
    Ok(record)
}

pub fn new_event(application_id: &str, event: &str) -> AuditEvent {
    AuditEvent {
        event_id: Uuid::new_v4().to_string(),
        timestamp: Utc::now().to_rfc3339(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        application_id: application_id.to_string(),
        event: event.to_string(),
        model: None,
        model_digest: None,
        model_digest_at_commit: None,
        template_version: 0,
        rule_ids: Vec::new(),
        rules_sha256: String::new(),
        documents: Vec::new(),
        blocking: false,
        findings: Vec::new(),
        advisory_count: 0,
        override_reason: None,
        note: None,
        generation_usage: Vec::new(),
    }
}

fn parse_time(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|time| time.with_timezone(&Utc))
}

fn review_window(workspace_root: &Path) -> (Option<String>, Option<String>, bool) {
    let reviews = read_reviews(workspace_root);
    let Some(last) = reviews
        .iter()
        .max_by_key(|review| parse_time(&review.timestamp))
    else {
        return (None, None, false);
    };
    let due = parse_time(&last.timestamp).map(|time| time + Duration::days(REVIEW_CADENCE_DAYS));
    let overdue = due.map(|due| due < Utc::now()).unwrap_or(false);
    (
        Some(last.timestamp.clone()),
        due.map(|time| time.to_rfc3339()),
        overdue,
    )
}

/// Aggregate evidence across all applications plus the review cadence record.
pub fn stats(workspace_root: &Path) -> HonestyStats {
    let mut stats = HonestyStats {
        review_cadence_days: REVIEW_CADENCE_DAYS,
        ..HonestyStats::default()
    };
    let applications_root = workspace_root.join("applications");
    let Ok(entries) = fs::read_dir(&applications_root) else {
        let (last, due, overdue) = review_window(workspace_root);
        stats.last_review_at = last;
        stats.next_review_due = due;
        stats.review_overdue = overdue;
        return stats;
    };

    let mut decisions = 0usize;
    for entry in entries.flatten() {
        let app_dir = entry.path();
        if !app_dir.is_dir() {
            continue;
        }
        let events = read_events(&app_dir);
        if events.is_empty() {
            continue;
        }
        stats.applications += 1;
        for event in events {
            match event.event.as_str() {
                "generated" => {
                    stats.generated += 1;
                    for finding in &event.findings {
                        *stats.by_code.entry(finding.code.clone()).or_insert(0) += 1;
                        if finding.blocking {
                            *stats.blocks_by_code.entry(finding.code.clone()).or_insert(0) += 1;
                            stats.blocked_findings += 1;
                        } else {
                            stats.advisory_findings += 1;
                        }
                    }
                }
                "approved" => {
                    stats.approved += 1;
                    decisions += 1;
                }
                "override" => {
                    stats.approved += 1;
                    stats.overrides += 1;
                    decisions += 1;
                }
                _ => {}
            }
        }
    }

    stats.false_block_reports = all_problem_reports(workspace_root)
        .iter()
        .filter(|report| report.category == "false_block")
        .count();
    stats.override_rate = if decisions == 0 {
        0.0
    } else {
        stats.overrides as f64 / decisions as f64
    };
    let (last, due, overdue) = review_window(workspace_root);
    stats.last_review_at = last;
    stats.next_review_due = due;
    stats.review_overdue = overdue;
    stats
}

#[cfg(test)]
mod tests {
    use super::{
        append_disclosure, append_event, append_problem, append_review, digest_text,
        has_output_disclosure, materials_version, new_event, read_events, rules_fingerprint,
        sha256_hex, stats, ProblemReport, ReviewRecord, DISCLOSURE_VERSION,
    };
    use crate::guardrails::RuleView;
    use std::fs;

    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("veridicus-compliance-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn rule(kind: &str, subject: &str) -> RuleView {
        RuleView {
            kind: kind.to_string(),
            subject: subject.to_string(),
            rule_text: String::new(),
            applies_to: "all".to_string(),
        }
    }

    #[test]
    fn sha256_matches_known_value() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn events_append_and_survive_a_read() {
        let dir = temp_dir();
        let mut first = new_event("app-1", "generated");
        first.blocking = true;
        let mut second = new_event("app-1", "approved");
        second.note = Some("reviewed".to_string());

        append_event(&dir, &first).unwrap();
        append_event(&dir, &second).unwrap();

        let events = read_events(&dir);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event, "generated");
        assert!(events[0].blocking);
        assert_eq!(events[1].note.as_deref(), Some("reviewed"));
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn generation_usage_survives_a_read_and_an_older_event_still_parses() {
        let dir = temp_dir();
        let mut event = new_event("app-1", "generated");
        event.generation_usage.push(super::DocumentUsage {
            skill_id: "resume-tailor".to_string(),
            reply_limit: 8_192,
            attempts: vec![
                super::GenerationAttempt {
                    prompt_tokens: Some(7_400),
                    reply_tokens: Some(8_192),
                    done_reason: Some("length".to_string()),
                    kept: false,
                },
                super::GenerationAttempt {
                    prompt_tokens: Some(7_400),
                    reply_tokens: Some(1_502),
                    done_reason: Some("stop".to_string()),
                    kept: true,
                },
            ],
        });

        // A trail written before usage was recorded has no such field.
        let mut legacy = serde_json::to_value(&event).unwrap();
        legacy.as_object_mut().unwrap().remove("generation_usage");
        let audit_dir = dir.join(super::AUDIT_DIR);
        std::fs::create_dir_all(&audit_dir).unwrap();
        std::fs::write(
            audit_dir.join(super::EVENTS_FILE),
            format!("{}\n", serde_json::to_string(&legacy).unwrap()),
        )
        .unwrap();
        append_event(&dir, &event).unwrap();

        let events = read_events(&dir);
        assert_eq!(events.len(), 2);
        assert!(events[0].generation_usage.is_empty());
        let usage = &events[1].generation_usage[0];
        assert_eq!(usage.skill_id, "resume-tailor");
        assert_eq!(usage.reply_limit, 8_192);
        assert_eq!(usage.attempts[0].done_reason.as_deref(), Some("length"));
        assert!(!usage.attempts[0].kept);
        assert!(usage.attempts[1].kept);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn rule_fingerprint_ignores_order() {
        let forward = rules_fingerprint(&[
            rule("never_claim", "clearance"),
            rule("ask_first", "kubernetes"),
        ]);
        let reverse = rules_fingerprint(&[
            rule("ask_first", "kubernetes"),
            rule("never_claim", "clearance"),
        ]);

        assert_eq!(forward.1, reverse.1);
        assert_eq!(forward.0.len(), 2);
    }

    #[test]
    fn materials_version_changes_with_content() {
        let dir = temp_dir();
        fs::write(dir.join("resume.md"), "first").unwrap();
        let first = materials_version(&dir);
        fs::write(dir.join("resume.md"), "second").unwrap();
        let second = materials_version(&dir);

        assert_ne!(first, second);
        assert_eq!(digest_text("resume.md", "second").sha256.len(), 64);
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn output_disclosure_must_follow_the_latest_generation() {
        let dir = temp_dir();
        append_event(&dir, &new_event("app-1", "generated")).unwrap();
        assert!(!has_output_disclosure(&dir));

        let mut ack = new_event("app-1", "disclosure_acknowledged");
        ack.note = Some(DISCLOSURE_VERSION.to_string());
        append_event(&dir, &ack).unwrap();
        assert!(has_output_disclosure(&dir));

        append_event(&dir, &new_event("app-1", "generated")).unwrap();
        assert!(!has_output_disclosure(&dir));
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn stats_count_blocks_incidents_and_reviews() {
        let workspace = temp_dir();
        let app = workspace.join("applications").join("app-1");
        fs::create_dir_all(&app).unwrap();

        let mut generated = new_event("app-1", "generated");
        generated.findings.push(crate::guardrails::GuardrailFinding {
            code: "ungrounded_bullet".to_string(),
            message: "invented".to_string(),
            blocking: true,
        });
        generated.findings.push(crate::guardrails::GuardrailFinding {
            code: "weak_claim_provenance".to_string(),
            message: "thin".to_string(),
            blocking: false,
        });
        append_event(&app, &generated).unwrap();
        append_event(&app, &new_event("app-1", "override")).unwrap();

        append_problem(
            &app,
            &ProblemReport {
                incident_id: "inc-1".to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                application_id: "app-1".to_string(),
                category: "false_block".to_string(),
                note: "this was true".to_string(),
                status: "open".to_string(),
                app_version: "test".to_string(),
            },
        )
        .unwrap();

        append_review(
            &workspace,
            &ReviewRecord {
                review_id: "review-1".to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                reviewer: "local-user".to_string(),
                note: "monthly review".to_string(),
                applications_covered: 1,
                blocking_findings: 1,
                overrides: 1,
                false_block_reports: 1,
            },
        )
        .unwrap();

        let result = stats(&workspace);
        assert_eq!(result.applications, 1);
        assert_eq!(result.generated, 1);
        assert_eq!(result.overrides, 1);
        assert_eq!(result.approved, 1);
        assert_eq!(result.blocked_findings, 1);
        assert_eq!(result.advisory_findings, 1);
        assert_eq!(result.blocks_by_code.get("ungrounded_bullet"), Some(&1));
        assert_eq!(result.false_block_reports, 1);
        assert!((result.override_rate - 1.0).abs() < f64::EPSILON);
        assert!(result.last_review_at.is_some());
        assert!(result.next_review_due.is_some());
        assert!(!result.review_overdue);
        fs::remove_dir_all(workspace).ok();
    }

    #[test]
    fn disclosure_records_are_append_only() {
        let workspace = temp_dir();
        let first = append_disclosure(&workspace, None, "first-run").unwrap();
        let second = append_disclosure(&workspace, Some("app-1"), "per-application").unwrap();
        let contents = fs::read_to_string(
            workspace
                .join(super::COMPLIANCE_DIR)
                .join(super::DISCLOSURES_FILE),
        )
        .unwrap();
        assert_eq!(contents.lines().count(), 2);
        assert_eq!(first.version, DISCLOSURE_VERSION);
        assert_eq!(second.application_id.as_deref(), Some("app-1"));
        fs::remove_dir_all(workspace).ok();
    }

}
