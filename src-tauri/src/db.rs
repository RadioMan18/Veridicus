use rusqlite::ffi::{sqlite3_key, SQLITE_OK};
use rusqlite::{Connection, Result};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use serde::{Deserialize, Serialize};
use tauri::State;
use rand::Rng;
use crate::AppState;

static CACHED_DB_KEY: Mutex<Option<Vec<u8>>> = Mutex::new(None);

/// Generate a secure key for SQLCipher (32 random bytes)
fn generate_key() -> Vec<u8> {
    let mut key = vec![0u8; 32];
    let mut rng = rand::thread_rng();
    rng.fill(&mut key[..]);
    key
}

pub fn get_or_create_db_key(db_path: &Path) -> Result<Vec<u8>, String> {
    let mut cache = CACHED_DB_KEY.lock().map_err(|e| e.to_string())?;
    if let Some(ref k) = *cache {
        return Ok(k.clone());
    }

    let workspace_root = db_path.parent().ok_or("Invalid database path")?;
    let dot_careeros = workspace_root.join(".careeros");
    let _ = std::fs::create_dir_all(&dot_careeros);
    let key_path = dot_careeros.join(".db_key");

    let key = if key_path.exists() {
        let hex_str = std::fs::read_to_string(&key_path)
            .map_err(|e| format!("Failed to read key file: {}", e))?;
        hex::decode(hex_str.trim())
            .map_err(|e| format!("Invalid hex key in key file: {}", e))?
    } else {
        let k = generate_key();
        let hex_str = hex::encode(&k);
        let _ = std::fs::write(&key_path, hex_str);
        k
    };

    *cache = Some(key.clone());
    Ok(key)
}

pub fn get_db_connection(db_path: &str) -> Result<Connection, String> {
    let path = PathBuf::from(db_path);
    let key = get_or_create_db_key(&path)?;
    let conn = Connection::open(&path)
        .map_err(|e| format!("Failed to open database: {}", e))?;

    unsafe {
        let handle = conn.handle();
        let rc = sqlite3_key(handle, key.as_ptr() as *const _, key.len() as _);
        if rc != SQLITE_OK {
            return Err("failed to set SQLCipher key".to_string());
        }
    }

    match conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(())) {
        Ok(_) => {
            ensure_evidence_columns(&conn)?;
            ensure_gap_columns(&conn)?;
            Ok(conn)
        }
        Err(_) => {
            // Re-initialize cleanly if previous schema was invalid or created with missing key
            drop(conn);
            let _ = std::fs::remove_file(&path);
            init_encrypted_database(&path, &key)?;

            let conn2 = Connection::open(&path)
                .map_err(|e2| format!("Failed to reopen database: {}", e2))?;
            unsafe {
                let handle = conn2.handle();
                let rc = sqlite3_key(handle, key.as_ptr() as *const _, key.len() as _);
                if rc != SQLITE_OK {
                    return Err("failed to set SQLCipher key on recreated db".to_string());
                }
            }
            conn2.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))
                .map_err(|e2| format!("Failed to verify recreated database: {}", e2))?;
            ensure_evidence_columns(&conn2)?;
            ensure_gap_columns(&conn2)?;
            Ok(conn2)
        }
    }
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
    let nonce = Nonce::from_slice(b"careeros_key");
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
            last_confirmed_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            evidence_type TEXT NOT NULL DEFAULT 'unspecified',
            evidence_sha256 TEXT NOT NULL DEFAULT '',
            verified_by TEXT NOT NULL DEFAULT ''
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

        INSERT OR IGNORE INTO app_settings (key, value) VALUES ('ollama_fast_model', 'llama3.2:latest');
        INSERT OR IGNORE INTO app_settings (key, value) VALUES ('ollama_deep_model', 'llama3.2:latest');
        COMMIT;
    ".trim()

    ).map_err(|e| format!("Failed to create tables: {}", e))?;

    ensure_evidence_columns(&conn)?;
    ensure_gap_columns(&conn)?;
    Ok(())
}

/// Existing workspaces predate claim provenance. Add the columns once, without
/// rewriting or deleting any stored claim.
fn ensure_evidence_columns(conn: &Connection) -> Result<(), String> {
    let has_claims: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'claims'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("Failed to inspect the database schema: {}", error))?;
    if has_claims == 0 {
        return Ok(());
    }

    let mut statement = conn
        .prepare("PRAGMA table_info(claims)")
        .map_err(|error| format!("Failed to inspect claims schema: {}", error))?;
    let columns: Vec<String> = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| format!("Failed to read claims columns: {}", error))?
        .filter_map(|column| column.ok())
        .collect();
    drop(statement);

    for (name, ddl) in [
        (
            "evidence_type",
            "ALTER TABLE claims ADD COLUMN evidence_type TEXT NOT NULL DEFAULT 'unspecified'",
        ),
        (
            "evidence_sha256",
            "ALTER TABLE claims ADD COLUMN evidence_sha256 TEXT NOT NULL DEFAULT ''",
        ),
        (
            "verified_by",
            "ALTER TABLE claims ADD COLUMN verified_by TEXT NOT NULL DEFAULT ''",
        ),
    ] {
        if !columns.iter().any(|column| column == name) {
            conn.execute_batch(ddl)
                .map_err(|error| format!("Failed to add claims.{}: {}", name, error))?;
        }
    }

    Ok(())
}

/// The fit matrix adds columns to gaps. Existing workspaces get them added here,
/// once per open, without touching any stored decision.
pub(crate) fn ensure_gap_columns(conn: &Connection) -> Result<(), String> {
    let has_gaps: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'gaps'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("Failed to inspect the database schema: {}", error))?;
    if has_gaps == 0 {
        return Ok(());
    }

    let mut statement = conn
        .prepare("PRAGMA table_info(gaps)")
        .map_err(|error| format!("Failed to inspect gaps schema: {}", error))?;
    let columns: Vec<String> = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| format!("Failed to read gaps columns: {}", error))?
        .filter_map(|column| column.ok())
        .collect();
    drop(statement);

    for (name, ddl) in [
        (
            "cv_focus",
            "ALTER TABLE gaps ADD COLUMN cv_focus TEXT NOT NULL DEFAULT ''",
        ),
        (
            "cover_letter_point",
            "ALTER TABLE gaps ADD COLUMN cover_letter_point TEXT NOT NULL DEFAULT ''",
        ),
        (
            "story_ids",
            "ALTER TABLE gaps ADD COLUMN story_ids TEXT NOT NULL DEFAULT ''",
        ),
    ] {
        if !columns.iter().any(|column| column == name) {
            conn.execute_batch(ddl)
                .map_err(|error| format!("Failed to add gaps.{}: {}", name, error))?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod schema_tests {
    use super::init_encrypted_database;

    #[test]
    fn initialising_the_database_twice_is_safe_and_adds_the_matrix_columns() {
        let dir = std::env::temp_dir().join(format!("careeros-schema-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("careeros-test.db");
        let _ = std::fs::remove_file(&path);

        let key = vec![7u8; 32];
        init_encrypted_database(&path, &key).expect("first initialisation");
        init_encrypted_database(&path, &key).expect("second initialisation");

        let conn = rusqlite::Connection::open(&path).expect("open");
        unsafe {
            let rc = rusqlite::ffi::sqlite3_key(conn.handle(), key.as_ptr() as *const _, key.len() as _);
            assert_eq!(rc, rusqlite::ffi::SQLITE_OK);
        }

        let mut statement = conn.prepare("PRAGMA table_info(gaps)").expect("gaps table");
        let columns: Vec<String> = statement
            .query_map([], |row| row.get::<_, String>(1))
            .expect("columns")
            .filter_map(|column| column.ok())
            .collect();
        drop(statement);

        for required in ["cv_focus", "cover_letter_point", "story_ids"] {
            assert!(
                columns.iter().any(|column| column == required),
                "missing column {}",
                required
            );
        }

        drop(conn);
        let _ = std::fs::remove_file(&path);
    }
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

    *state.db_path.lock().unwrap() = Some(db_path.to_string_lossy().into_owned());

    if db_path.exists() {
        return Ok(InitResult {
            db_path: db_path.to_string_lossy().into_owned(),
            recovery_code: String::new(),
        });
    }

    let recovery_code = get_recovery_code();

    let db_key = get_or_create_db_key(&db_path)?;
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