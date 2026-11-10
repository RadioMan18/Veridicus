use std::fs;
use std::path::Path;
use git2::Repository;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::AppState;


/// Folder and file names that make up a workspace, kept in one place so a future
/// rename touches this list instead of every module.
pub const WORKSPACE_DIR: &str = "Veridicus";
pub const DOT_DIR: &str = ".veridicus";
pub const DB_FILE: &str = "veridicus.db";
const LEGACY_WORKSPACE_DIR: &str = "CareerOS";
const LEGACY_DOT_DIR: &str = ".careeros";
const LEGACY_DB_FILE: &str = "careeros.db";

/// Move a workspace created before the rename onto the current names, carrying
/// the SQLCipher key and database with it so existing claims still open. Runs
/// before any command, and only when the destination does not already exist, so
/// it is safe to call on every launch.
pub fn migrate_legacy_workspace() {
    if let Some(docs_dir) = dirs::document_dir() {
        migrate_workspace_in(&docs_dir);
    }
}

fn migrate_workspace_in(docs_dir: &Path) {
    let root = docs_dir.join(WORKSPACE_DIR);
    let legacy_root = docs_dir.join(LEGACY_WORKSPACE_DIR);
    if !root.exists() && legacy_root.exists() && fs::rename(&legacy_root, &root).is_err() {
        // Leave the old workspace untouched rather than half-migrating it.
        return;
    }
    if !root.exists() {
        return;
    }

    for (current, legacy) in [(DOT_DIR, LEGACY_DOT_DIR), (DB_FILE, LEGACY_DB_FILE)] {
        let destination = root.join(current);
        let source = root.join(legacy);
        if !destination.exists() && source.exists() {
            let _ = fs::rename(&source, &destination);
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WorkspaceStatus {
    pub root_path: String,
    pub initialized: bool,
    pub git_active: bool,
}

#[tauri::command]
pub fn initialize_workspace(state: State<'_, AppState>) -> Result<WorkspaceStatus, String> {
    migrate_legacy_workspace();

    let docs_dir = dirs::document_dir().ok_or("Failed to locate Documents folder")?;
    let workspace_root = docs_dir.join("Veridicus");

    let subdirs = vec![
        "templates",
        ".veridicus/skills",
        ".veridicus/agents",
        "master",
        "applications",
    ];

    for dir in subdirs {
        let path = workspace_root.join(dir);
        if !path.exists() {
            fs::create_dir_all(&path).map_err(|e| format!("Failed to create {:?}: {}", path, e))?;
        }
    }

    // Skill templates are the only source of generation prompts; keep them present and current.
    let _ = crate::skills::ensure_default_skill_templates(&workspace_root);

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
            "# Daily Log\n\n## {} - Veridicus Initialized\n- Workspace created\n- SQLCipher Claims Ledger initialized\n",
            chrono::Utc::now().format("%Y-%m-%d")
        );
        fs::write(&daily_log, content).map_err(|e| e.to_string())?;
    }

    let git_active = match Repository::open(&workspace_root) {
        Ok(_) => true,
        Err(_) => Repository::init(&workspace_root).is_ok(),
    };

    let db_path = workspace_root.join("veridicus.db");
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
    let workspace_root = docs_dir.join("Veridicus");
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

#[derive(Serialize, Debug, Clone)]
pub struct TrustStatus {
    pub ollama_online: bool,
    pub model: Option<String>,
    pub last_commit_seconds: Option<i64>,
}

/// Quiet trust signals for the top bar: whether Ollama answers, which model is
/// selected, and how recently the workspace committed.
#[tauri::command]
pub fn get_trust_status(state: State<'_, AppState>) -> Result<TrustStatus, String> {
    let ollama_online = crate::ollama::ollama_health_check().unwrap_or(false);
    let model = crate::ollama::get_ollama_config(state)
        .ok()
        .and_then(|config| config.deep_model.or(config.fast_model));

    let last_commit_seconds = dirs::document_dir().and_then(|docs| {
        let repo = Repository::open(docs.join("Veridicus")).ok()?;
        let mut revwalk = repo.revwalk().ok()?;
        revwalk.push_head().ok()?;
        let oid = revwalk.next()?.ok()?;
        let commit = repo.find_commit(oid).ok()?;
        Some(chrono::Utc::now().timestamp() - commit.time().seconds())
    });

    Ok(TrustStatus {
        ollama_online,
        model,
        last_commit_seconds,
    })
}

#[tauri::command]
pub fn get_workspace_status(state: State<'_, AppState>) -> Result<WorkspaceStatus, String> {
    let docs_dir = dirs::document_dir().ok_or("Failed to locate Documents folder")?;
    let workspace_root = docs_dir.join("Veridicus");

    let git_active = match Repository::open(&workspace_root) {
        Ok(_) => true,
        Err(_) => false,
    };

    let db_path = workspace_root.join("veridicus.db");
    *state.db_path.lock().unwrap() = Some(db_path.to_string_lossy().into_owned());

    Ok(WorkspaceStatus {
        root_path: workspace_root.to_string_lossy().into_owned(),
        initialized: workspace_root.exists(),
        git_active,
    })
}


#[cfg(test)]
mod migration_tests {
    use super::migrate_workspace_in;
    use std::fs;

    #[test]
    fn a_pre_rename_workspace_moves_onto_the_new_names_with_its_key() {
        let root = std::env::temp_dir().join(format!("veridicus-migration-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let legacy = root.join("CareerOS");
        fs::create_dir_all(legacy.join(".careeros")).expect("legacy workspace");
        fs::write(legacy.join(".careeros").join(".db_key"), "00ff").expect("key");
        fs::write(legacy.join("careeros.db"), b"encrypted").expect("db");
        fs::write(legacy.join("applications.csv"), "id
").expect("csv");

        migrate_workspace_in(&root);

        let moved = root.join("Veridicus");
        assert!(!legacy.exists(), "the old folder is gone");
        assert!(moved.join(".veridicus").join(".db_key").exists(), "the key follows the database");
        assert!(moved.join("veridicus.db").exists(), "the database is renamed");
        assert!(moved.join("applications.csv").exists(), "other files come along");

        // A second launch must be a no-op rather than a second migration.
        migrate_workspace_in(&root);
        assert!(moved.join("veridicus.db").exists());
        assert!(!moved.join("careeros.db").exists());

        let _ = fs::remove_dir_all(&root);
    }
}
