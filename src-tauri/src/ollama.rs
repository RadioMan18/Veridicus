//! Ollama REST client — local LLM engine for CareerOS.
//!
//! Communicates with the local Ollama daemon at http://localhost:11434
//! for model discovery, pull, and text generation (chat/generate endpoints).

use std::time::Duration;
use serde::{Deserialize, Serialize};
use reqwest::blocking::Client;
use tauri::State;
use crate::AppState;

pub const OLLAMA_BASE_URL: &str = "http://localhost:11434";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaModelInfo {
    pub name: String,
    pub size: u64,
    pub modified_at: String,
    pub digest: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaListResponse {
    pub models: Vec<OllamaModelInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaPullRequest {
    pub name: String,
    pub insecure: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaPullResponse {
    pub status: String,
    pub digest: Option<String>,
    pub total: Option<u64>,
    pub completed: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaChatRequest {
    pub model: String,
    pub messages: Vec<OllamaChatMessage>,
    pub stream: Option<bool>,
    pub format: Option<String>,
    pub options: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaChatResponse {
    pub model: String,
    pub created_at: String,
    pub message: OllamaChatMessage,
    pub done: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaGenerateRequest {
    pub model: String,
    pub prompt: String,
    pub system: Option<String>,
    pub stream: Option<bool>,
    pub format: Option<String>,
    pub options: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaGenerateResponse {
    pub model: String,
    pub created_at: String,
    pub response: String,
    pub done: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OllamaConfig {
    pub fast_model: Option<String>,
    pub deep_model: Option<String>,
}

/// Check Ollama daemon health.
#[tauri::command]
pub fn ollama_health_check() -> Result<bool, String> {
    let client = Client::new();
    let response = client
        .get(format!("{}/api/tags", OLLAMA_BASE_URL))
        .timeout(Duration::from_secs(5))
        .send();

    match response {
        Ok(resp) => Ok(resp.status().is_success()),
        Err(e) => Err(format!("Ollama daemon unreachable: {}", e)),
    }
}

/// List available models from local Ollama daemon.
#[tauri::command]
pub fn get_ollama_models() -> Result<Vec<OllamaModelInfo>, String> {
    let client = Client::new();
    let response = client
        .get(format!("{}/api/tags", OLLAMA_BASE_URL))
        .timeout(Duration::from_secs(10))
        .send();

    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                let list: OllamaListResponse = resp.json().map_err(|e| e.to_string())?;
                Ok(list.models)
            } else {
                Ok(vec![])
            }
        }
        Err(e) => Err(format!("Failed to fetch Ollama models: {}", e)),
    }
}

/// Send a chat completion request to Ollama (non-streaming).
#[tauri::command]
pub fn chat_with_ollama(
    model: String,
    messages: Vec<OllamaChatMessage>,
    temperature: Option<f64>,
    max_tokens: Option<u32>,
    format: Option<String>,
) -> Result<OllamaChatResponse, String> {
    let mut options = serde_json::json!({});
    if let Some(temp) = temperature {
        options["temperature"] = serde_json::json!(temp);
    }
    if let Some(max_tok) = max_tokens {
        options["num_predict"] = serde_json::json!(max_tok);
    }

    let request = OllamaChatRequest {
        model: model.clone(),
        messages: messages.clone(),
        stream: Some(false),
        format: format.clone(),
        options: Some(options.clone()),
    };

    let mut last_err: Option<String> = None;
    for attempt in 0..3 {
        let client = Client::builder()
            .timeout(Duration::from_secs(300))
            .build()
            .map_err(|e| format!("Failed to build client: {}", e))?;
        let response = client
            .post(format!("{}/api/chat", OLLAMA_BASE_URL))
            .header("Connection", "keep-alive")
            .json(&request)
            .send();

        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    let chat_resp: OllamaChatResponse = resp.json().map_err(|e| e.to_string())?;
                    return Ok(chat_resp);
                } else {
                    let status = resp.status();
                    let text = resp.text().unwrap_or_default();
                    last_err = Some(format!("Ollama chat error ({}): {}", status, text));
                }
            }
            Err(e) => {
                last_err = Some(format!("Ollama chat request failed: {}", e));
                std::thread::sleep(std::time::Duration::from_millis(500 * (attempt + 1)));
                continue;
            }
        }
    }
    Err(last_err.unwrap_or_else(|| "Ollama chat request failed".to_string()))
}

/// Send a text generation request to Ollama (non-streaming).
#[tauri::command]
pub fn generate_with_ollama(
    model: String,
    prompt: String,
    system: Option<String>,
    temperature: Option<f64>,
    max_tokens: Option<u32>,
    format: Option<String>,
) -> Result<OllamaGenerateResponse, String> {
    let mut options = serde_json::json!({});
    if let Some(temp) = temperature {
        options["temperature"] = serde_json::json!(temp);
    }
    if let Some(max_tok) = max_tokens {
        options["num_predict"] = serde_json::json!(max_tok);
    }

    let request = OllamaGenerateRequest {
        model: model.clone(),
        prompt,
        system,
        stream: Some(false),
        format,
        options: Some(options.clone()),
    };

    let mut last_err: Option<String> = None;
    for attempt in 0..3 {
        let client = Client::builder()
            .timeout(Duration::from_secs(300))
            .build()
            .map_err(|e| format!("Failed to build client: {}", e))?;
        let response = client
            .post(format!("{}/api/generate", OLLAMA_BASE_URL))
            .header("Connection", "keep-alive")
            .json(&request)
            .send();

        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    let gen_resp: OllamaGenerateResponse = resp.json().map_err(|e| e.to_string())?;
                    return Ok(gen_resp);
                } else {
                    let status = resp.status();
                    let text = resp.text().unwrap_or_default();
                    last_err = Some(format!("Ollama generate error ({}): {}", status, text));
                }
            }
            Err(e) => {
                last_err = Some(format!("Ollama generate request failed: {}", e));
                std::thread::sleep(std::time::Duration::from_millis(500 * (attempt + 1)));
                continue;
            }
        }
    }
    Err(last_err.unwrap_or_else(|| "Ollama generate request failed".to_string()))
}

/// Save Ollama model role configuration (fast/deep) to app_settings.
#[tauri::command]
pub fn save_ollama_config(
    fast_model: Option<String>,
    deep_model: Option<String>,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let db_path_opt = state.db_path.lock().unwrap();
    let db_path_str = db_path_opt.as_ref()
        .ok_or("Database not initialized")?;

    let conn = crate::db::get_db_connection(db_path_str)?;

    if let Some(fm) = fast_model {
        conn.execute(
            "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('ollama_fast_model', ?1)",
            [fm],
        ).map_err(|e| format!("Failed to save fast model: {}", e))?;
    }
    if let Some(dm) = deep_model {
        conn.execute(
            "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('ollama_deep_model', ?1)",
            [dm],
        ).map_err(|e| format!("Failed to save deep model: {}", e))?;
    }

    Ok(true)
}

/// Get saved Ollama model role configuration.
#[tauri::command]
pub fn get_ollama_config(
    state: State<'_, AppState>,
) -> Result<OllamaConfig, String> {
    let db_path_opt = state.db_path.lock().unwrap();
    let db_path_str = db_path_opt.as_ref()
        .ok_or("Database not initialized")?;

    let conn = crate::db::get_db_connection(db_path_str)?;

    let fast_model: Option<String> = conn.query_row(
        "SELECT value FROM app_settings WHERE key = 'ollama_fast_model'",
        [],
        |row| row.get(0),
    ).ok();

    let deep_model: Option<String> = conn.query_row(
        "SELECT value FROM app_settings WHERE key = 'ollama_deep_model'",
        [],
        |row| row.get(0),
    ).ok();

    // Auto-set defaults if not configured
    if fast_model.is_none() || deep_model.is_none() {
        let default_model = "llama3.2:latest".to_string();
        if fast_model.is_none() {
            let _ = conn.execute(
                "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('ollama_fast_model', ?1)",
                [&default_model],
            );
        }
        if deep_model.is_none() {
            let _ = conn.execute(
                "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('ollama_deep_model', ?1)",
                [&default_model],
            );
        }
        return Ok(OllamaConfig {
            fast_model: Some(default_model.clone()),
            deep_model: Some(default_model)
        });
    }

    Ok(OllamaConfig { fast_model, deep_model })
}

/// Stream model pull progress via text output (sequential lines).
#[tauri::command]
pub fn pull_ollama_model_progress(model_name: String) -> Result<String, String> {
    let client = Client::new();
    let response = client
        .post(format!("{}/api/pull", OLLAMA_BASE_URL))
        .json(&serde_json::json!({ "name": model_name, "stream": true }))
        .timeout(Duration::from_secs(900))
        .send();

    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                let text = resp.text().map_err(|e| e.to_string())?;
                Ok(text)
            } else {
                Err(format!("Ollama pull failed: {}", resp.status()))
            }
        }
        Err(e) => Err(format!("Ollama pull request failed: {}", e)),
    }
}