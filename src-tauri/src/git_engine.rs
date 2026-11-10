use git2::{Repository, IndexAddOption, Signature};
use tauri::State;
use crate::AppState;

#[tauri::command]
pub fn init_repository(_state: State<'_, AppState>) -> Result<(), String> {
    let docs_dir = dirs::document_dir().ok_or("Failed to locate Documents folder")?;
    let workspace_root = docs_dir.join("Veridicus");

    let repo = match Repository::open(&workspace_root) {
        Ok(repo) => repo,
        Err(_) => Repository::init(&workspace_root)
            .map_err(|e| format!("Failed to init git repo: {}", e))?,
    };

    let mut config = repo.config()
        .map_err(|e| format!("Failed to get git config: {}", e))?;
    config.set_str("user.email", "veridicus@localhost")
        .map_err(|e| format!("Failed to set git config: {}", e))?;
    config.set_str("user.name", "Veridicus")
        .map_err(|e| format!("Failed to set git config: {}", e))?;

    Ok(())
}

#[tauri::command]
pub fn commit_changes(
    _state: State<'_, AppState>,
    message: String,
) -> Result<String, String> {
    let docs_dir = dirs::document_dir().ok_or("Failed to locate Documents folder")?;
    let workspace_root = docs_dir.join("Veridicus");

    let repo = Repository::open(&workspace_root)
        .map_err(|e| format!("Failed to open git repo: {}", e))?;

    let mut idx = repo.index()
        .map_err(|e| format!("Failed to get index: {}", e))?;
    idx.add_all(["."], IndexAddOption::DEFAULT, None)
        .map_err(|e| format!("Failed to stage changes: {}", e))?;

    let tree_oid = idx.write_tree()
        .map_err(|e| format!("Failed to write tree: {}", e))?;
    let tree = repo.find_tree(tree_oid)
        .map_err(|e| format!("Failed to find tree: {}", e))?;

    let sig = Signature::now("Veridicus", "veridicus@localhost")
        .map_err(|e| format!("Failed to create signature: {}", e))?;

    let parent = match repo.head() {
        Ok(head) => Some(
            head.peel_to_commit()
                .map_err(|e| format!("Failed to resolve current HEAD commit: {}", e))?,
        ),
        Err(error) if error.code() == git2::ErrorCode::UnbornBranch => None,
        Err(error) => return Err(format!("Failed to resolve current HEAD: {}", error)),
    };
    let parents = parent.as_ref().map(|commit| vec![commit]).unwrap_or_default();

    let oid = repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        &message,
        &tree,
        &parents,
    ).map_err(|e| format!("Failed to commit: {}", e))?;

    Ok(oid.to_string())
}

#[tauri::command]
pub fn get_commit_history(
    _state: State<'_, AppState>
) -> Result<Vec<String>, String> {
    let docs_dir = dirs::document_dir().ok_or("Failed to locate Documents folder")?;
    let workspace_root = docs_dir.join("Veridicus");

    let repo = Repository::open(&workspace_root)
        .map_err(|e| format!("Failed to open git repo: {}", e))?;

    let mut revwalk = repo.revwalk()
        .map_err(|e| format!("Failed to create revwalk: {}", e))?;
    revwalk.push_glob("HEAD").or_else(|_| revwalk.push_glob("*")).ok();

    let mut commits = Vec::new();
    for oid in revwalk {
        let oid = oid.map_err(|e| format!("Revwalk error: {}", e))?;
        let commit = repo.find_commit(oid)
            .map_err(|e| format!("Failed to find commit: {}", e))?;
        let message = commit.message()
            .unwrap_or("")
            .lines()
            .next()
            .unwrap_or("")
            .to_string();
        commits.push(format!("{} ({})", oid, message));
    }

    Ok(commits)
}

#[tauri::command]
pub fn get_git_status(
    _state: State<'_, AppState>
) -> Result<String, String> {
    let docs_dir = dirs::document_dir().ok_or("Failed to locate Documents folder")?;
    let workspace_root = docs_dir.join("Veridicus");

    let repo = Repository::open(&workspace_root)
        .map_err(|e| format!("Failed to open git repo: {}", e))?;

    let statuses = repo.statuses(None)
        .map_err(|e| format!("Failed to get git status: {}", e))?;

    let mut lines = Vec::new();
    for entry in statuses.iter() {
        let status = entry.status();
        let path = entry.path().unwrap_or("unknown").to_string();
        let status_str = match status {
            s if s.is_wt_new() => "new file",
            s if s.is_wt_modified() => "modified",
            s if s.is_wt_deleted() => "deleted",
            s if s.is_wt_renamed() => "renamed",
            _ => "modified",
        };
        lines.push(format!("{} {}", status_str, path));
    }

    Ok(lines.join("\n"))
}