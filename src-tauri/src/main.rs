//! Veridicus Desktop — Tauri v2 entry point.
//!
//! Honesty guardrail verification engine with SQLCipher-encrypted Claims Ledger.

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod workspace;
mod db;
mod claims;
mod git_engine;
mod setup_wizard;
mod ollama;
mod scraper;
mod gap_analyzer;
mod skills;
mod fact_check;
mod guardrails;
mod compliance;
#[cfg(test)]
mod guardrail_eval;
mod scope_review;
mod typst_engine;
mod orchestrator;
mod resume_parser;
mod ats_engine;
mod resume_exporter;
mod keywords;
mod interview_prep;
mod mock_interview;
mod bullet_review;
mod recruiter_review;
mod pack;

use std::sync::Mutex;
use tauri::generate_handler;

#[derive(Default)]
pub struct AppState {
    pub db_path: Mutex<Option<String>>,
}

pub fn main() {

    // A workspace created before the rename moves onto the new names first.
    workspace::migrate_legacy_workspace();
    let _single_instance_guard = match std::net::TcpListener::bind("127.0.0.1:42139") {
        Ok(listener) => listener,
        Err(_) => {
            rfd::MessageDialog::new()
                .set_title("Veridicus")
                .set_description("Veridicus is already running in the background. Please close the existing instance or check Task Manager.")
                .show();
            return;
        }
    };

    std::panic::set_hook(Box::new(|info| {
        let msg = format!("PANIC: {}\nBacktrace: {:?}", info, std::backtrace::Backtrace::capture());
        let _ = std::fs::write("panic.log", msg);
    }));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .manage(AppState::default())
        .invoke_handler(generate_handler![
            workspace::initialize_workspace,
            workspace::get_workspace_status,
            workspace::get_trust_status,
            db::init_encrypted_database_cmd,
            db::verify_sqlcipher_cmd,
            db::verify_key_cmd,
            claims::create_claim,
            claims::get_claim,
            claims::list_claims,
            claims::update_claim,
            claims::delete_claim,
            claims::verify_claim_evidence,
            claims::create_rule,
            claims::list_rules,
            claims::update_rule,
            claims::delete_rule,
            claims::get_presentation_rules,
            claims::save_presentation_rule,
            git_engine::init_repository,
            git_engine::commit_changes,
            git_engine::get_commit_history,
            git_engine::get_git_status,
            setup_wizard::get_setup_wizard_state,
            setup_wizard::get_setup_wizard_data,
            setup_wizard::save_user_profile,
            setup_wizard::save_resume_content,
            setup_wizard::extract_claims_from_resume,
            setup_wizard::save_claims,
            setup_wizard::save_presentation_rules,
            setup_wizard::complete_setup_wizard,
            setup_wizard::get_available_ollama_models,
            setup_wizard::pull_ollama_model,
            ollama::ollama_health_check,
            ollama::get_ollama_models,
            ollama::pull_ollama_model_progress,
            ollama::chat_with_ollama,
            ollama::generate_with_ollama,
            ollama::save_ollama_config,
            ollama::get_ollama_config,
            ollama::pull_ollama_model_progress,
            scraper::scrape_job_page,
            gap_analyzer::analyze_job_gaps,
            gap_analyzer::analyze_jd_brief,
            guardrails::set_gap_status,
            guardrails::list_gaps,
            orchestrator::get_fit_matrix,
            interview_prep::generate_interview_prep,
            interview_prep::get_interview_prep,
            interview_prep::get_interview_prep_markdown,
            mock_interview::plan_mock_interview,
            mock_interview::plan_mock_drill,
            mock_interview::evaluate_mock_answer,
            mock_interview::finish_mock_interview,
            mock_interview::recent_mock_sessions,
            mock_interview::save_mock_debrief,
            bullet_review::review_bullets,
            bullet_review::record_approved_wording,
            recruiter_review::generate_recruiter_review,
            pack::generate_application_pack,
            skills::execute_skill,
            skills::list_skill_templates,
            skills::seed_default_skills,
            fact_check::run_fact_check_pass,
            fact_check::get_fact_check_summary,
            typst_engine::compile_typst_pdf,
            typst_engine::rasterize_pdf_pages,
            typst_engine::analyze_visual_qa,
            typst_engine::verify_sidecars,
            orchestrator::execute_apply_workflow,
            orchestrator::cancel_apply,
            orchestrator::commit_application,
            orchestrator::get_honesty_stats,
            orchestrator::report_output_problem,
            orchestrator::export_incident_log,
            orchestrator::record_honesty_review,
            orchestrator::get_ai_disclosure_state,
            orchestrator::acknowledge_ai_disclosure,
            orchestrator::acknowledge_output_disclosure,
            orchestrator::get_artifact_preview,
            orchestrator::open_application_folder,
            orchestrator::save_text_via_dialog,
            orchestrator::get_application_folder,
            orchestrator::run_company_research,
            orchestrator::draft_followups,
            orchestrator::list_followup_candidates,
            orchestrator::generate_weekly_report,
            orchestrator::analyze_logs,
            orchestrator::get_application_records,
            orchestrator::update_application_status,
            orchestrator::open_resume_file,
            orchestrator::export_ats_resume,
            orchestrator::get_master_resume,
            orchestrator::generate_ats_resume_markdown,
            orchestrator::check_ats_compliance,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}