use rusqlite::{Connection, params, Row};
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
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateClaimRequest {
    pub statement: Option<String>,
    pub evidence_source: Option<String>,
    pub confirmed_wording: Option<String>,
    pub scope: Option<String>,
    pub category: Option<String>,
    pub status: Option<String>,
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

fn get_conn(state: &State<'_, AppState>) -> Result<Connection, String> {
    let db_path_str = {
        let db_path_opt = state.db_path.lock().unwrap();
        db_path_opt.as_ref()
            .ok_or("Database not initialized")?
            .clone()
    };
    get_db_connection(&db_path_str)
}

#[tauri::command]
pub fn create_claim(
    request: CreateClaimRequest,
    state: State<'_, AppState>
) -> Result<Claim, String> {
    let conn = get_conn(&state)?;
    
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO claims (id, statement, evidence_source, confirmed_wording, scope, category, status, last_confirmed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![id, request.statement, request.evidence_source, request.confirmed_wording,
                request.scope, request.category, request.status, now],
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
    })
}

#[tauri::command]
pub fn get_claim(
    id: String,
    state: State<'_, AppState>
) -> Result<Claim, String> {
    let conn = get_conn(&state)?;
    
    let claim = conn.query_row(
        "SELECT id, statement, evidence_source, confirmed_wording, scope, category, status, last_confirmed_at
         FROM claims WHERE id = ?1",
        params![id],
        |row| row_to_claim(row),
    ).map_err(|e| format!("Claim not found: {}", e))?;

    Ok(claim)
}

#[tauri::command]
pub fn list_claims(
    state: State<'_, AppState>
) -> Result<Vec<Claim>, String> {
    let conn = get_conn(&state)?;
    
    let mut stmt = conn.prepare(
        "SELECT id, statement, evidence_source, confirmed_wording, scope, category, status, last_confirmed_at
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
    
    let now = Utc::now().to_rfc3339();
    let mut query = String::from("UPDATE claims SET last_confirmed_at = ?1");
    let mut params_vec = vec![now.to_string()];

    if let Some(statement) = request.statement {
        query.push_str(", statement = ?");
        params_vec.push(statement);
    }
    if let Some(evidence_source) = request.evidence_source {
        query.push_str(", evidence_source = ?");
        params_vec.push(evidence_source);
    }
    if let Some(confirmed_wording) = request.confirmed_wording {
        query.push_str(", confirmed_wording = ?");
        params_vec.push(confirmed_wording);
    }
    if let Some(scope) = request.scope {
        query.push_str(", scope = ?");
        params_vec.push(scope);
    }
    if let Some(category) = request.category {
        query.push_str(", category = ?");
        params_vec.push(category);
    }
    if let Some(status) = request.status {
        query.push_str(", status = ?");
        params_vec.push(status);
    }

    query.push_str(" WHERE id = ?");
    params_vec.push(id.clone());

    let params_ref: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
    conn.execute(&query, params_ref.as_slice())
        .map_err(|e| format!("Failed to update claim: {}", e))?;

    get_claim(id, state)
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