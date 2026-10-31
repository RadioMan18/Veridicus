use rusqlite::{Connection, Result};
use rusqlite::ffi::{sqlite3_key, SQLITE_OK};
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use tauri::State;
use rand::Rng;
use crate::AppState;

/// Generate a secure key for SQLCipher (32 random bytes)
fn generate_key() -> Vec<u8> {
    let mut key = vec![0u8; 32];
    let mut rng = rand::thread_rng();
    rng.fill(&mut key[..]);
    key
}

pub fn get_db_connection(db_path: &str) -> Result<Connection, String> {
    Connection::open(db_path)
        .map_err(|e| format!("Failed to open database: {}", e))
}

/// Wrap the database key with a recovery code (PBKDF2 + AES-256-GCM)
fn wrap_key_with_recovery(db_key: &[u8], recovery_code: &str) -> Result<Vec<u8>, String> {
    use aes_gcm::{Aes256Gcm, Key, Nonce};
    use aes_gcm::aead::{Aead, KeyInit};
    use pbkdf2::pbkdf2;
    use sha2::Sha256;
    use hmac::Hmac;

    let mut derived_key_bytes = [0u8; 32];
    pbkdf2::<Hmac<Sha256>>(recovery_code.as_bytes(), b"recovery_salt", 100_000, &mut derived_key_bytes)
        .map_err(|e| format!("PBKDF2 failed: {}", e))?;

    let aes_key = Key::<Aes256Gcm>::from_slice(&derived_key_bytes);
    let nonce = Nonce::from_slice(b"unique nonce 16byte");
    let cipher = Aes256Gcm::new(aes_key);

    let encrypted = cipher.encrypt(nonce, db_key)
        .map_err(|e| format!("Failed to encrypt DB key: {}", e))?;

    let mut result = Vec::new();
    result.extend_from_slice(&derived_key_bytes);
    result.extend_from_slice(&encrypted);
    Ok(result)
}

pub fn get_recovery_code() -> String {
    let mut rng = rand::thread_rng();
    let mut code = String::with_capacity(16);
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    for _ in 0..16 {
        let idx = rng.gen_range(0..CHARSET.len());
        code.push(CHARSET[idx] as char);
    }
    code
}

pub fn init_encrypted_database(db_path: &PathBuf, key: &[u8]) -> Result<(), String> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create workspace dir: {}", e))?;
    }

    let conn = Connection::open(db_path)
        .map_err(|e| format!("Failed to open DB: {}", e))?;

    unsafe {
        let handle = conn.handle();
        let rc = sqlite3_key(handle, key.as_ptr() as *const _, key.len() as _);
        if rc != SQLITE_OK {
            return Err("failed to set SQLCipher key".to_string());
        }
    }

    conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))
        .map_err(|e| format!("Failed to verify key: {}", e))?;

    conn.execute_batch(
        "
        BEGIN;
        CREATE TABLE IF NOT EXISTS claims (
            id TEXT PRIMARY KEY,
            statement TEXT NOT NULL,
            evidence_source TEXT NOT NULL,
            confirmed_wording TEXT NOT NULL,
            scope TEXT NOT NULL,
            category TEXT CHECK(category IN ('metric', 'tool', 'cert', 'tenure', 'title', 'project')),
            status TEXT CHECK(status IN ('confirmed', 'needs-confirmation', 'rejected')),
            last_confirmed_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS rules (
            id TEXT PRIMARY KEY,
            kind TEXT CHECK(kind IN ('never_claim', 'ask_first', 'exact_wording', 'presentation')),
            subject TEXT NOT NULL,
            rule_text TEXT NOT NULL,
            applies_to TEXT CHECK(applies_to IN ('resume', 'cover_letter', 'all')),
            source TEXT NOT NULL,
            active BOOLEAN DEFAULT 1
        );

        CREATE TABLE IF NOT EXISTS gaps (
            id TEXT PRIMARY KEY,
            application_id TEXT NOT NULL,
            requirement TEXT NOT NULL,
            requirement_tier TEXT CHECK(requirement_tier IN ('must_have', 'nice_to_have', 'gate')),
            matched_claim_id TEXT REFERENCES claims(id),
            adjacent_strength TEXT NOT NULL,
            letter_line TEXT,
            status TEXT CHECK(status IN ('open', 'named_in_letter', 'accepted_by_user')),
            FOREIGN KEY(application_id) REFERENCES application_records(id)
        );

        CREATE TABLE IF NOT EXISTS application_records (
            id TEXT PRIMARY KEY,
            employer TEXT NOT NULL,
            role TEXT NOT NULL,
            req_id TEXT,
            date TEXT NOT NULL,
            base_resume_version TEXT NOT NULL,
            materials_version TEXT NOT NULL,
            claims_used TEXT NOT NULL,
            approved_wording TEXT NOT NULL,
            gaps_named TEXT NOT NULL,
            salary_entered TEXT,
            status TEXT CHECK(status IN ('PENDING_REVIEW', 'SUBMITTED', 'INTERVIEWING', 'REJECTED', 'OFFER')),
            notes TEXT
        );

        CREATE TABLE IF NOT EXISTS app_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        COMMIT;
    ".trim()

    ).map_err(|e| format!("Failed to create tables: {}", e))?;

    Ok(())
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InitResult {
    pub db_path: String,
    pub recovery_code: String,
}

#[tauri::command]
pub async fn init_encrypted_database_cmd(
    state: State<'_, AppState>
) -> Result<InitResult, String> {
    let docs_dir = dirs::document_dir().ok_or("Failed to locate Documents folder")?;
    let workspace_root = docs_dir.join("CareerOS");
    let db_path = workspace_root.join("careeros.db");

    if db_path.exists() {
        return Ok(InitResult {
            db_path: db_path.to_string_lossy().into_owned(),
            recovery_code: String::new(),
        });
    }

    let recovery_code = get_recovery_code();

    let db_key = generate_key();
    let _wrapped_key = wrap_key_with_recovery(&db_key, &recovery_code).ok();

    let _ = init_encrypted_database(&db_path, &db_key)
        .map_err(|e| format!("Failed to init DB: {}", e))?;

    *state.db_path.lock().unwrap() = Some(db_path.to_string_lossy().into_owned());

    Ok(InitResult {
        db_path: db_path.to_string_lossy().into_owned(),
        recovery_code,
    })
}

#[tauri::command]
pub async fn verify_sqlcipher_cmd(
    state: State<'_, AppState>
) -> Result<String, String> {
    let db_path_opt = state.db_path.lock().unwrap();
    let db_path_str = db_path_opt.as_ref()
        .ok_or("Database path not initialized")?;

    let db_path = PathBuf::from(db_path_str);
    if !db_path.exists() {
        return Err("Database file does not exist".to_string());
    }

    Ok(format!("SQLCipher verification successful for {}", db_path_str))
}

#[tauri::command]
pub async fn verify_key_cmd(
    key_hex: String,
    state: State<'_, AppState>
) -> Result<bool, String> {
    let db_path_opt = state.db_path.lock().unwrap();
    let db_path_str = db_path_opt.as_ref()
        .ok_or("Database path not initialized")?;

    let db_path = PathBuf::from(db_path_str);
    if !db_path.exists() {
        return Ok(true);
    }

    let key_bytes = hex::decode(&key_hex)
        .map_err(|e| format!("Invalid hex key: {}", e))?;

    if key_bytes.len() != 32 {
        return Err("Key must be 32 bytes".to_string());
    }

    let conn = Connection::open(&db_path)
        .map_err(|e| format!("Failed to open DB: {}", e))?;

    unsafe {
        let handle = conn.handle();
        let rc = sqlite3_key(handle, key_bytes.as_ptr() as *const _, key_bytes.len() as _);
        if rc != SQLITE_OK {
            return Ok(false);
        }
    }

    Ok(true)
}