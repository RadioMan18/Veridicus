//! Live smoke run for campaign stage 0.9.
//!
//! Builds the real Tauri context on the main thread, then calls the same command
//! the review screen calls. The artefacts, guardrail report and audit records
//! inspected afterwards are the ones the app produces; nothing is synthesized here.
//!
//! ```text
//! cd src-tauri
//! CAREEROS_SMOKE_EMPLOYER=Aurora \
//! CAREEROS_SMOKE_ROLE="Learning and Development Lead" \
//! cargo run --features smoke-harness --bin smoke_apply
//! ```
//!
//! Requires a running Ollama daemon and the real `Documents/CareerOS` workspace.
//! It writes one PENDING_REVIEW application and never commits it. It is a `bin`
//! rather than an `example` because tauri-build applies its Windows manifest link
//! arguments to bins only; an example binary fails to load (`0xc0000139`).

#[path = "../workspace.rs"]
mod workspace;
#[path = "../db.rs"]
mod db;
#[path = "../claims.rs"]
mod claims;
#[path = "../git_engine.rs"]
mod git_engine;
#[path = "../setup_wizard.rs"]
mod setup_wizard;
#[path = "../ollama.rs"]
mod ollama;
#[path = "../scraper.rs"]
mod scraper;
#[path = "../gap_analyzer.rs"]
mod gap_analyzer;
#[path = "../skills.rs"]
mod skills;
#[path = "../fact_check.rs"]
mod fact_check;
#[path = "../guardrails.rs"]
mod guardrails;
#[path = "../compliance.rs"]
mod compliance;
#[path = "../scope_review.rs"]
mod scope_review;
#[path = "../typst_engine.rs"]
mod typst_engine;
#[path = "../orchestrator.rs"]
mod orchestrator;
#[path = "../resume_parser.rs"]
mod resume_parser;
#[path = "../ats_engine.rs"]
mod ats_engine;
#[path = "../resume_exporter.rs"]
mod resume_exporter;

use std::sync::Mutex;
use tauri::Manager;

/// Mirrors the app state defined in `main.rs`; the harness crate owns its own copy.
#[derive(Default)]
pub struct AppState {
    pub db_path: Mutex<Option<String>>,
}

fn workspace_root() -> std::path::PathBuf {
    dirs::document_dir()
        .expect("documents directory")
        .join("CareerOS")
}

fn short(value: Option<&str>, max: usize) -> String {
    match value {
        Some(text) => text.chars().take(max).collect(),
        None => "-".to_string(),
    }
}

/// Print the shape of a serialized report without echoing generated prose.
fn print_summary(label: &str, value: &serde_json::Value) {
    let Some(map) = value.as_object() else {
        println!("{label}: {value}");
        return;
    };
    let parts: Vec<String> = map
        .iter()
        .map(|(key, item)| match item {
            serde_json::Value::Array(items) => format!("{key}[{}]", items.len()),
            serde_json::Value::String(_) => format!("{key}:text"),
            serde_json::Value::Object(_) => format!("{key}:obj"),
            other => format!("{key}:{other}"),
        })
        .collect();
    println!("{label}: {}", parts.join(" "));
}

fn print_report_file(app_dir: &std::path::Path, name: &str) {
    match std::fs::read_to_string(app_dir.join(name)) {
        Ok(text) => {
            println!("report {name}: {} bytes", text.len());
            match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(value) => print_summary(&format!("report {name}"), &value),
                Err(_) => println!("report {name}: unparseable"),
            }
        }
        Err(_) => println!("report {name}: MISSING"),
    }
}

fn main() {
    let app = tauri::Builder::default()
        .manage(AppState::default())
        .build(tauri::generate_context!())
        .expect("build tauri app");
    let handle = app.handle().clone();
    let state = app.state::<AppState>();

    let status = workspace::initialize_workspace(state.clone()).expect("workspace");
    println!("workspace: {}", status.root_path);
    println!("git active: {}", status.git_active);

    if let Ok(config) = ollama::get_ollama_config(state.clone()) {
        println!(
            "ollama config: fast={:?} deep={:?}",
            config.fast_model, config.deep_model
        );
    }

    if let Some(db_path) = state.db_path.lock().unwrap().clone() {
        if let Ok(conn) = db::get_db_connection(&db_path) {
            match guardrails::fetch_confirmed_claims(&conn) {
                Ok(claims) => {
                    println!("claims: confirmed={}", claims.len());
                    for claim in claims.iter().take(3) {
                        let snippet: String = claim.statement.chars().take(90).collect();
                        println!("  claim {} scope={:?} {}", claim.id, claim.scope, snippet);
                    }
                }
                Err(error) => println!("claims: unreadable ({error})"),
            }
        }
    }

    // Maintenance mode: repair a ledger polluted by the pre-fix DOCX parser.
    if std::env::var("CAREEROS_REBUILD_LEDGER").is_ok() {
        let db_path = state
            .db_path
            .lock()
            .unwrap()
            .clone()
            .expect("workspace database path");
        let conn = db::get_db_connection(&db_path).expect("open database");
        let master = workspace_root().join("master").join("resume.md");
        let report = setup_wizard::rebuild_claims_from_master(&conn, Some(&master))
            .expect("rebuild ledger");
        println!(
            "ledger rebuilt: removed={} inserted={} suggestions={} master_chars={} master_cleaned={}",
            report.removed,
            report.inserted,
            report.suggestions,
            report.master_chars,
            report.master_cleaned
        );
        return;
    }

    let url = std::env::var("CAREEROS_SMOKE_JD_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8765/smoke-posting.html".to_string());
    let employer = std::env::var("CAREEROS_SMOKE_EMPLOYER").ok();
    let role = std::env::var("CAREEROS_SMOKE_ROLE").ok();
    println!("posting: {url}");

    let apply_state = state.clone();
    let started = std::time::Instant::now();
    // Tauri owns the async runtime on this thread; a locally created Runtime
    // cannot be dropped inside that context.
    let result = tauri::async_runtime::block_on(async move {
            orchestrator::execute_apply_workflow(
                orchestrator::ApplyInput {
                    jd_url: url,
                    employer,
                    role,
                    req_id: None,
                    // Stage 0.9 observes generation only; approval stays a human action.
                    defer_commit: true,
                },
                apply_state,
                handle,
            )
            .await
        })
        .expect("workflow returned Err");

    println!(
        "apply: success={} application_id={} elapsed={:?}",
        result.success,
        result.application_id,
        started.elapsed()
    );
    if let Some(error) = &result.error_message {
        println!("warning: {error}");
    }
    println!(
        "jd scraped={} path={}",
        result.jd_scraped,
        result.jd_path.as_deref().unwrap_or("-")
    );

    if let Some(gaps) = &result.gap_analysis {
        println!(
            "gaps: supported={} gaps={} ask_first={} ambiguous={}",
            gaps.supported_claim_ids.len(),
            gaps.identified_gaps.len(),
            gaps.ask_first_triggered.len(),
            gaps.ambiguous_requirements.len()
        );
        for gap in gaps.identified_gaps.iter().take(5) {
            println!(
                "  gap tier={:?} requirement={}",
                gap.requirement_tier,
                short(Some(&gap.requirement), 70)
            );
        }
    } else {
        println!("gaps: none");
    }

    for (label, skill) in [
        ("resume", result.resume_tailored.as_ref()),
        ("letter", result.cover_letter.as_ref()),
    ] {
        match skill {
            Some(skill) => println!(
                "{label}: {} bytes model={} digest={} template_v={} warning={}",
                skill.markdown_output.len(),
                skill.model.as_deref().unwrap_or("-"),
                short(skill.model_digest.as_deref(), 12),
                skill.template_version,
                short(skill.untrusted_input_warning.as_deref(), 60)
            ),
            None => println!("{label}: none"),
        }
    }

    if let Some(report) = &result.guardrail_report {
        println!(
            "guardrails: passed={} blocking={} checks={} findings={}",
            report.passed,
            report.blocking,
            report.checks_run,
            report.findings.len()
        );
        for finding in &report.findings {
            println!("  finding: {} blocking={}", finding.code, finding.blocking);
        }
    } else {
        println!("guardrails: none");
    }

    for (label, report) in [
        ("fact check (letter)", result.fact_check_report.as_ref()),
        ("fact check (resume)", result.resume_fact_check_report.as_ref()),
    ] {
        match report {
            Some(report) => match serde_json::to_value(report) {
                Ok(value) => print_summary(label, &value),
                Err(_) => println!("{label}: unprintable"),
            },
            None => println!("{label}: none"),
        }
    }

    let app_dir = workspace_root()
        .join("applications")
        .join(&result.application_id);
    for name in [
        "job-description.md",
        "resume.md",
        "cover-letter.md",
        "resume.pdf",
        "cover-letter.pdf",
    ] {
        match std::fs::metadata(app_dir.join(name)) {
            Ok(meta) => println!("artifact {name}: {} bytes", meta.len()),
            Err(_) => println!("artifact {name}: MISSING"),
        }
    }
    for name in [
        "guardrail-report.json",
        "guardrail-report-apply.json",
        "guardrail-report-commit.json",
    ] {
        print_report_file(&app_dir, name);
    }

    match std::fs::read_to_string(app_dir.join("audit").join("events.jsonl")) {
        Ok(text) => {
            let lines: Vec<&str> = text.lines().filter(|line| !line.trim().is_empty()).collect();
            println!("audit events: {}", lines.len());
            for line in lines {
                match serde_json::from_str::<serde_json::Value>(line) {
                    Ok(event) => println!(
                        "  audit event={} model={} digest={} template_v={} rules={} documents={} blocking={} findings={} advisory={}",
                        event.get("event").and_then(|v| v.as_str()).unwrap_or("?"),
                        event.get("model").and_then(|v| v.as_str()).unwrap_or("-"),
                        short(event.get("model_digest").and_then(|v| v.as_str()), 12),
                        event.get("template_version").map(|v| v.to_string()).unwrap_or_else(|| "-".into()),
                        event.get("rule_ids").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0),
                        event.get("documents").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0),
                        event.get("blocking").and_then(|v| v.as_bool()).unwrap_or(false),
                        event.get("findings").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0),
                        event.get("advisory_count").map(|v| v.to_string()).unwrap_or_else(|| "-".into()),
                    ),
                    Err(_) => println!("  audit event: unparseable line"),
                }
            }
        }
        Err(error) => println!("audit events: MISSING ({error})"),
    }

    if let Ok(stats) = orchestrator::get_honesty_stats(state.clone()) {
        if let Ok(value) = serde_json::to_value(&stats) {
            print_summary("honesty stats", &value);
        }
    }

    // The review gate must refuse to commit until the per-application disclosure
    // is acknowledged. Cheap to prove here; no model call.
    let refusal = orchestrator::commit_application(result.application_id.clone(), None, state.clone());
    match &refusal {
        Ok(commit) => println!("commit gate: FAILED, committed without acknowledgement ({commit})"),
        Err(reason) => println!("commit gate: refused ({reason})"),
    }

    if !result.success {
        eprintln!(
            "SMOKE FAILED: apply did not complete: {:?}",
            result.error_message
        );
        std::process::exit(1);
    }
    if refusal.is_ok() {
        eprintln!("SMOKE FAILED: commit gate accepted a commit without acknowledgement");
        std::process::exit(1);
    }
    println!("SMOKE OK");
}
