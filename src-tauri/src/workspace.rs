use std::fs;
use git2::Repository;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::AppState;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WorkspaceStatus {
    pub root_path: String,
    pub initialized: bool,
    pub git_active: bool,
}

#[tauri::command]
pub fn initialize_workspace(state: State<'_, AppState>) -> Result<WorkspaceStatus, String> {
    let docs_dir = dirs::document_dir().ok_or("Failed to locate Documents folder")?;
    let workspace_root = docs_dir.join("CareerOS");

    let subdirs = vec![
        "templates",
        ".careeros/skills",
        ".careeros/agents",
        "master",
        "applications",
    ];

    for dir in subdirs {
        let path = workspace_root.join(dir);
        if !path.exists() {
            fs::create_dir_all(&path).map_err(|e| format!("Failed to create {:?}: {}", path, e))?;
        }
    }

    let csv_path = workspace_root.join("applications.csv");
    if !csv_path.exists() {
        let headers = "id,date,employer,role,status,job_url,folder_path,req_id,base_resume_version,materials_version,claims_used,approved_wording,gaps_named,salary_entered,notes\n";
        fs::write(&csv_path, headers).map_err(|e| e.to_string())?;
    }

    let system_md = workspace_root.join("SYSTEM.md");
    if !system_md.exists() {
        let content = "# Master Career Context\n\n## Target Roles\n- Senior Product Manager\n\n## Tone Constraints\n- Direct, active voice, zero jargon.\n";
        fs::write(&system_md, content).map_err(|e| e.to_string())?;
    }

    let daily_log = workspace_root.join("daily-log.md");
    if !daily_log.exists() {
        let content = format!(
            "# Daily Log\n\n## {} - CareerOS Initialized\n- Workspace created\n- SQLCipher Claims Ledger initialized\n",
            chrono::Utc::now().format("%Y-%m-%d")
        );
        fs::write(&daily_log, content).map_err(|e| e.to_string())?;
    }

    let git_active = match Repository::open(&workspace_root) {
        Ok(_) => true,
        Err(_) => Repository::init(&workspace_root).is_ok(),
    };

    let db_path = workspace_root.join("careeros.db");
    *state.db_path.lock().unwrap() = Some(db_path.to_string_lossy().into_owned());

    Ok(WorkspaceStatus {
        root_path: workspace_root.to_string_lossy().into_owned(),
        initialized: true,
        git_active,
    })
}

/// Append an activity line to the daily log without truncating earlier entries.
pub fn append_daily_log(activity: &str) -> Result<(), String> {
    let docs_dir = dirs::document_dir().ok_or("Failed to locate Documents folder")?;
    let workspace_root = docs_dir.join("CareerOS");
    fs::create_dir_all(&workspace_root)
        .map_err(|e| format!("Failed to create workspace: {}", e))?;
    let log_path = workspace_root.join("daily-log.md");

    let mut content = if log_path.exists() {
        fs::read_to_string(&log_path).map_err(|e| format!("Failed to read daily log: {}", e))?
    } else {
        "# Daily Log\n".to_string()
    };

    if !content.ends_with('\n') {
        content.push('\n');
    }

    let heading = format!("## {}", chrono::Utc::now().format("%Y-%m-%d"));
    if !content.lines().any(|line| line.trim() == heading) {
        content.push_str(&format!("\n{}\n", heading));
    }

    content.push_str(&format!(
        "- {} - {}\n",
        chrono::Local::now().format("%H:%M"),
        activity.trim()
    ));

    fs::write(&log_path, content).map_err(|e| format!("Failed to update daily log: {}", e))
}

#[tauri::command]
pub fn get_workspace_status(state: State<'_, AppState>) -> Result<WorkspaceStatus, String> {
    let docs_dir = dirs::document_dir().ok_or("Failed to locate Documents folder")?;
    let workspace_root = docs_dir.join("CareerOS");

    let git_active = match Repository::open(&workspace_root) {
        Ok(_) => true,
        Err(_) => false,
    };

    let db_path = workspace_root.join("careeros.db");
    *state.db_path.lock().unwrap() = Some(db_path.to_string_lossy().into_owned());

    Ok(WorkspaceStatus {
        root_path: workspace_root.to_string_lossy().into_owned(),
        initialized: workspace_root.exists(),
        git_active,
    })
}