use rusqlite::{Connection, OptionalExtension, params, Row};
use uuid::Uuid;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::AppState;
use crate::db::get_db_connection;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Claim {
    pub id: String,
    pub statement: String,
    pub evidence_source: String,
    pub confirmed_wording: String,
    pub scope: String,
    pub category: String,
    pub status: String,
    pub last_confirmed_at: String,
    pub evidence_type: String,
    pub evidence_sha256: String,
    pub verified_by: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Rule {
    pub id: String,
    pub kind: String,
    pub subject: String,
    pub rule_text: String,
    pub applies_to: String,
    pub source: String,
    pub active: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CreateClaimRequest {
    pub statement: String,
    pub evidence_source: String,
    pub confirmed_wording: String,
    pub scope: String,
    pub category: String,
    pub status: String,
    #[serde(default)]
    pub evidence_type: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateClaimRequest {
    pub statement: Option<String>,
    pub evidence_source: Option<String>,
    pub confirmed_wording: Option<String>,
    pub scope: Option<String>,
    pub category: Option<String>,
    pub status: Option<String>,
    pub evidence_type: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CreateRuleRequest {
    pub kind: String,
    pub subject: String,
    pub rule_text: String,
    pub applies_to: String,
    pub source: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateRuleRequest {
    pub kind: Option<String>,
    pub subject: Option<String>,
    pub rule_text: Option<String>,
    pub applies_to: Option<String>,
    pub source: Option<String>,
    pub active: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClaimIntegrityMismatch {
    pub id: String,
    pub statement: String,
    pub stored: String,
    pub recomputed: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClaimIntegrityReport {
    pub checked: usize,
    pub verified: usize,
    pub mismatches: Vec<ClaimIntegrityMismatch>,
}

fn get_conn(state: &State<'_, AppState>) -> Result<Connection, String> {
    let db_path_str = {
        let db_path_opt = state.db_path.lock().unwrap();
        db_path_opt.as_ref()
            .ok_or("Database not initialized")?
            .clone()
    };
    get_db_connection(&db_path_str)
}

/// Human-readable identity for the person who confirmed a claim. Veridicus is a
/// single-user desktop app, so the profile name is the strongest identity available.
pub fn verifier_name(conn: &Connection) -> String {
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = 'user_profile'",
        [],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .ok()
    .flatten()
    .and_then(|json| serde_json::from_str::<serde_json::Value>(&json).ok())
    .and_then(|value| {
        value["full_name"]
            .as_str()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_string)
    })
    .unwrap_or_else(|| "local-user".to_string())
}

pub fn normalized_evidence_type(value: &str) -> String {
    let trimmed = value.trim().to_lowercase().replace(' ', "_");
    if trimmed.is_empty() {
        "other".to_string()
    } else {
        trimmed
    }
}

/// Fingerprint of the provenance record and the fact it supports. It is a
/// tamper-evidence check, not a hash of an external document.
pub fn claim_evidence_sha256(
    statement: &str,
    evidence_source: &str,
    evidence_type: &str,
    scope: &str,
    category: &str,
) -> String {
    let canonical = [
        statement,
        evidence_source,
        evidence_type,
        scope,
        category,
    ]
    .iter()
    .map(|value| value.trim().to_lowercase())
    .collect::<Vec<_>>()
    .join("\u{1f}");
    crate::compliance::sha256_hex(canonical.as_bytes())
}

pub fn is_weak_provenance(claim: &Claim) -> bool {
    let evidence_type = claim.evidence_type.trim().to_lowercase();
    evidence_type.is_empty()
        || evidence_type == "unspecified"
        || evidence_type == "other"
        || claim.evidence_source.trim().len() < 4
        || claim.evidence_sha256.trim().is_empty()
        || claim.verified_by.trim().is_empty()
}

fn load_claim(conn: &Connection, id: &str) -> Result<Claim, String> {
    conn.query_row(
        "SELECT id, statement, evidence_source, confirmed_wording, scope, category, status, last_confirmed_at, evidence_type, evidence_sha256, verified_by
         FROM claims WHERE id = ?1",
        params![id],
        |row| row_to_claim(row),
    )
    .map_err(|e| format!("Claim not found: {}", e))
}

#[tauri::command]
pub fn create_claim(
    request: CreateClaimRequest,
    state: State<'_, AppState>
) -> Result<Claim, String> {
    let conn = get_conn(&state)?;

    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let evidence_type = normalized_evidence_type(&request.evidence_type);
    let evidence_sha256 = claim_evidence_sha256(
        &request.statement,
        &request.evidence_source,
        &evidence_type,
        &request.scope,
        &request.category,
    );
    let verified_by = if request.status == "confirmed" {
        verifier_name(&conn)
    } else {
        String::new()
    };

    conn.execute(
        "INSERT INTO claims (id, statement, evidence_source, confirmed_wording, scope, category, status, last_confirmed_at, evidence_type, evidence_sha256, verified_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![id, request.statement, request.evidence_source, request.confirmed_wording,
                request.scope, request.category, request.status, now, evidence_type, evidence_sha256, verified_by],
    ).map_err(|e| format!("Failed to create claim: {}", e))?;

    Ok(Claim {
        id,
        statement: request.statement,
        evidence_source: request.evidence_source,
        confirmed_wording: request.confirmed_wording,
        scope: request.scope,
        category: request.category,
        status: request.status,
        last_confirmed_at: now,
        evidence_type,
        evidence_sha256,
        verified_by,
    })
}

#[tauri::command]
pub fn get_claim(
    id: String,
    state: State<'_, AppState>
) -> Result<Claim, String> {
    let conn = get_conn(&state)?;
    load_claim(&conn, &id)
}

#[tauri::command]
pub fn list_claims(
    state: State<'_, AppState>
) -> Result<Vec<Claim>, String> {
    let conn = get_conn(&state)?;

    let mut stmt = conn.prepare(
        "SELECT id, statement, evidence_source, confirmed_wording, scope, category, status, last_confirmed_at, evidence_type, evidence_sha256, verified_by
         FROM claims ORDER BY last_confirmed_at DESC"
    ).map_err(|e| format!("Failed to prepare statement: {}", e))?;

    let claims = stmt.query_map([], |row| row_to_claim(row))
        .map_err(|e| format!("Failed to query claims: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to collect claims: {}", e))?;

    Ok(claims)
}

#[tauri::command]
pub fn update_claim(
    id: String,
    request: UpdateClaimRequest,
    state: State<'_, AppState>
) -> Result<Claim, String> {
    let conn = get_conn(&state)?;
    let current = load_claim(&conn, &id)?;

    let statement = request.statement.unwrap_or(current.statement);
    let evidence_source = request.evidence_source.unwrap_or(current.evidence_source);
    let confirmed_wording = request.confirmed_wording.unwrap_or(current.confirmed_wording);
    let scope = request.scope.unwrap_or(current.scope);
    let category = request.category.unwrap_or(current.category);
    let status = request.status.unwrap_or(current.status);
    let evidence_type = normalized_evidence_type(
        &request.evidence_type.unwrap_or(current.evidence_type),
    );
    let now = Utc::now().to_rfc3339();
    let evidence_sha256 = claim_evidence_sha256(
        &statement,
        &evidence_source,
        &evidence_type,
        &scope,
        &category,
    );
    let verified_by = if status == "confirmed" {
        verifier_name(&conn)
    } else {
        String::new()
    };

    conn.execute(
        "UPDATE claims SET statement = ?1, evidence_source = ?2, confirmed_wording = ?3,
         scope = ?4, category = ?5, status = ?6, last_confirmed_at = ?7,
         evidence_type = ?8, evidence_sha256 = ?9, verified_by = ?10 WHERE id = ?11",
        params![
            statement,
            evidence_source,
            confirmed_wording,
            scope,
            category,
            status,
            now,
            evidence_type,
            evidence_sha256,
            verified_by,
            id
        ],
    ).map_err(|e| format!("Failed to update claim: {}", e))?;

    load_claim(&conn, &id)
}

#[tauri::command]
pub fn delete_claim(
    id: String,
    state: State<'_, AppState>
) -> Result<bool, String> {
    let conn = get_conn(&state)?;

    conn.execute("DELETE FROM claims WHERE id = ?1", params![id])
        .map_err(|e| format!("Failed to delete claim: {}", e))?;

    Ok(true)
}

/// Recompute every claim's provenance fingerprint and report rows that no longer
/// match. This makes the stored hash auditable without trusting the write path.
#[tauri::command]
pub fn verify_claim_evidence(
    state: State<'_, AppState>
) -> Result<ClaimIntegrityReport, String> {
    let conn = get_conn(&state)?;
    let mut stmt = conn.prepare(
        "SELECT id, statement, evidence_source, confirmed_wording, scope, category, status, last_confirmed_at, evidence_type, evidence_sha256, verified_by
         FROM claims ORDER BY last_confirmed_at DESC"
    ).map_err(|e| format!("Failed to prepare claim integrity query: {}", e))?;

    let claims = stmt.query_map([], |row| row_to_claim(row))
        .map_err(|e| format!("Failed to query claims: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to collect claims: {}", e))?;

    let mut mismatches = Vec::new();
    for claim in &claims {
        let recomputed = claim_evidence_sha256(
            &claim.statement,
            &claim.evidence_source,
            &claim.evidence_type,
            &claim.scope,
            &claim.category,
        );
        if recomputed != claim.evidence_sha256 {
            mismatches.push(ClaimIntegrityMismatch {
                id: claim.id.clone(),
                statement: claim.statement.clone(),
                stored: claim.evidence_sha256.clone(),
                recomputed,
            });
        }
    }

    Ok(ClaimIntegrityReport {
        checked: claims.len(),
        verified: claims.len() - mismatches.len(),
        mismatches,
    })
}

#[tauri::command]
pub fn create_rule(
    request: CreateRuleRequest,
    state: State<'_, AppState>
) -> Result<Rule, String> {
    let conn = get_conn(&state)?;

    let id = Uuid::new_v4().to_string();
    let source = format!("user-stated {}", Utc::now().format("%Y-%m-%d"));

    conn.execute(
        "INSERT INTO rules (id, kind, subject, rule_text, applies_to, source, active)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1)",
        params![id, request.kind, request.subject, request.rule_text, request.applies_to, source],
    ).map_err(|e| format!("Failed to create rule: {}", e))?;

    Ok(Rule {
        id,
        kind: request.kind,
        subject: request.subject,
        rule_text: request.rule_text,
        applies_to: request.applies_to,
        source,
        active: true,
    })
}

#[tauri::command]
pub fn list_rules(
    state: State<'_, AppState>
) -> Result<Vec<Rule>, String> {
    let conn = get_conn(&state)?;

    let mut stmt = conn.prepare(
        "SELECT id, kind, subject, rule_text, applies_to, source, active
         FROM rules ORDER BY kind, subject"
    ).map_err(|e| format!("Failed to prepare statement: {}", e))?;

    let rules = stmt.query_map([], |row| row_to_rule(row))
        .map_err(|e| format!("Failed to query rules: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to collect rules: {}", e))?;

    Ok(rules)
}

#[tauri::command]
pub fn update_rule(
    id: String,
    request: UpdateRuleRequest,
    state: State<'_, AppState>
) -> Result<Rule, String> {
    let conn = get_conn(&state)?;

    let mut query = String::from("UPDATE rules SET");
    let mut params_vec: Vec<String> = Vec::new();

    for (field, val) in [
        ("kind", request.kind.as_deref()),
        ("subject", request.subject.as_deref()),
        ("rule_text", request.rule_text.as_deref()),
        ("applies_to", request.applies_to.as_deref()),
        ("source", request.source.as_deref()),
    ] {
        if let Some(v) = val {
            if !params_vec.is_empty() {
                query.push_str(",");
            }
            query.push_str(&format!(" {} = ?", field));
            params_vec.push(v.to_string());
        }
    }
    if let Some(active) = request.active {
        if !params_vec.is_empty() {
            query.push_str(",");
        }
        query.push_str(" active = ?");
        params_vec.push(if active { "1" } else { "0" }.to_string());
    }

    query.push_str(" WHERE id = ?");
    params_vec.push(id.clone());

    let params_ref: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
    conn.execute(&query, params_ref.as_slice())
        .map_err(|e| format!("Failed to update rule: {}", e))?;

    get_rule(id, state)
}

#[tauri::command]
pub fn get_rule(
    id: String,
    state: State<'_, AppState>
) -> Result<Rule, String> {
    let conn = get_conn(&state)?;

    let rule = conn.query_row(
        "SELECT id, kind, subject, rule_text, applies_to, source, active
         FROM rules WHERE id = ?1",
        params![id],
        |row| row_to_rule(row),
    ).map_err(|e| format!("Rule not found: {}", e))?;

    Ok(rule)
}

#[tauri::command]
pub fn delete_rule(
    id: String,
    state: State<'_, AppState>
) -> Result<bool, String> {
    let conn = get_conn(&state)?;

    conn.execute("DELETE FROM rules WHERE id = ?1", params![id])
        .map_err(|e| format!("Failed to delete rule: {}", e))?;

    Ok(true)
}

#[tauri::command]
pub fn get_presentation_rules(
    state: State<'_, AppState>
) -> Result<Vec<Rule>, String> {
    let conn = get_conn(&state)?;

    let mut stmt = conn.prepare(
        "SELECT id, kind, subject, rule_text, applies_to, source, active
         FROM rules WHERE kind = 'presentation' ORDER BY subject"
    ).map_err(|e| format!("Failed to prepare statement: {}", e))?;

    let rules = stmt.query_map([], |row| row_to_rule(row))
        .map_err(|e| format!("Failed to query rules: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to collect rules: {}", e))?;

    Ok(rules)
}

#[tauri::command]
pub fn save_presentation_rule(
    section: String,
    date_policy: String,
    tenure_phrasing: String,
    employer_neutral_metrics: bool,
    state: State<'_, AppState>
) -> Result<Rule, String> {
    let conn = get_conn(&state)?;

    let id = Uuid::new_v4().to_string();
    let source = format!("user-stated {}", Utc::now().format("%Y-%m-%d"));
    let rule_text = format!(
        "section={},date_policy={},tenure_phrasing={},employer_neutral={}",
        section, date_policy, tenure_phrasing, employer_neutral_metrics
    );

    conn.execute(
        "INSERT INTO rules (id, kind, subject, rule_text, applies_to, source, active)
         VALUES (?1, 'presentation', ?2, ?3, 'all', ?4, 1)",
        params![id, section, rule_text, source],
    ).map_err(|e| format!("Failed to save presentation rule: {}", e))?;

    Ok(Rule {
        id,
        kind: "presentation".to_string(),
        subject: section,
        rule_text,
        applies_to: "all".to_string(),
        source,
        active: true,
    })
}

fn row_to_claim(row: &Row) -> rusqlite::Result<Claim> {
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
}

fn row_to_rule(row: &Row) -> rusqlite::Result<Rule> {
    Ok(Rule {
        id: row.get(0)?,
        kind: row.get(1)?,
        subject: row.get(2)?,
        rule_text: row.get(3)?,
        applies_to: row.get(4)?,
        source: row.get(5)?,
        active: row.get(6)?,
    })
}

#[cfg(test)]
mod tests {
    use super::{claim_evidence_sha256, is_weak_provenance, Claim};

    fn claim() -> Claim {
        Claim {
            id: "c1".to_string(),
            statement: "Reduced processing time by 30%".to_string(),
            evidence_source: "2024 performance review".to_string(),
            confirmed_wording: "Reduced processing time by 30%".to_string(),
            scope: "within the team".to_string(),
            category: "metric".to_string(),
            status: "confirmed".to_string(),
            last_confirmed_at: "2026-01-01T00:00:00Z".to_string(),
            evidence_type: "performance_review".to_string(),
            evidence_sha256: "abc".to_string(),
            verified_by: "Jane".to_string(),
        }
    }

    #[test]
    fn evidence_hash_changes_when_the_provenance_changes() {
        let first = claim_evidence_sha256(
            "Reduced processing time by 30%",
            "2024 performance review",
            "performance_review",
            "within the team",
            "metric",
        );
        let second = claim_evidence_sha256(
            "Reduced processing time by 30%",
            "2024 performance review",
            "manager_review",
            "within the team",
            "metric",
        );
        assert_eq!(first.len(), 64);
        assert_ne!(first, second);
    }

    #[test]
    fn weak_provenance_requires_a_type_reference_hash_and_verifier() {
        let strong = claim();
        assert!(!is_weak_provenance(&strong));

        let mut weak = strong.clone();
        weak.evidence_type = "unspecified".to_string();
        assert!(is_weak_provenance(&weak));

        let mut weak = strong.clone();
        weak.verified_by.clear();
        assert!(is_weak_provenance(&weak));

        let mut weak = strong;
        weak.evidence_sha256.clear();
        assert!(is_weak_provenance(&weak));
    }
}
