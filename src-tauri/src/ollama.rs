//! Ollama REST client — local LLM engine for Veridicus.
//!
//! Communicates with the local Ollama daemon at http://localhost:11434
//! for model discovery, pull, and text generation (chat/generate endpoints).

use std::sync::OnceLock;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use reqwest::blocking::Client;
use tauri::State;
use crate::AppState;

pub const OLLAMA_BASE_URL: &str = "http://localhost:11434";

/// One HTTP client for the whole process.
///
/// `reqwest::blocking::Client` owns a Tokio runtime. Building one per call and
/// dropping it inside an async command aborts the process with "Cannot drop a
/// runtime in a context where blocking is not allowed", so the client is built
/// once and never dropped; per-request deadlines are set on each request.
fn client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        Client::builder()
            .build()
            .expect("failed to build the Ollama HTTP client")
    })
}

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
    /// Either the string `json` or a JSON Schema object. A schema also constrains
    /// which keys must appear, so a reply cannot legally stop after its first
    /// array the way `json` alone allows.
    pub format: Option<serde_json::Value>,
    /// Top-level chat field, not an option. `Some(false)` keeps a thinking model
    /// from spending the token budget on reasoning it never shows the caller.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub think: Option<bool>,
    pub options: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaChatResponse {
    pub model: String,
    pub created_at: String,
    pub message: OllamaChatMessage,
    pub done: bool,
    /// Why generation ended: `stop` for a natural finish, `length` when the
    /// token budget ran out.
    pub done_reason: Option<String>,
    /// Tokens spent reading the prompt; at or above the window, the prompt was
    /// trimmed rather than answered in full.
    pub prompt_eval_count: Option<u32>,
    /// Tokens spent generating, reasoning included.
    pub eval_count: Option<u32>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelIdentity {
    pub name: String,
    pub digest: Option<String>,
}

/// Default context window for every model call.
///
/// Ollama's own default (2048) is far below what a `/apply` prompt needs: the
/// master resume (32 KB) plus the claims block (24 KB) plus rules and template
/// plus a capped 12 KB posting reaches roughly 73 KB ≈ 18k tokens before the
/// document itself. 24576 covers that with room for a 2000-token reply. A user
/// who lowers it in settings is respected; a value below ~18k silently drops the
/// tail of the prompt.
pub const DEFAULT_NUM_CTX: u32 = 24_576;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OllamaConfig {
    pub fast_model: Option<String>,
    pub deep_model: Option<String>,
    /// Context window sent to Ollama on every call.
    #[serde(default)]
    pub num_ctx: Option<u32>,
}

/// Build the Ollama `options` payload in one place so the chat and generate
/// paths cannot drift apart.
fn request_options(
    temperature: Option<f64>,
    max_tokens: Option<u32>,
    num_ctx: Option<u32>,
) -> serde_json::Value {
    let mut options = serde_json::json!({});
    if let Some(temp) = temperature {
        options["temperature"] = serde_json::json!(temp);
    }
    if let Some(max_tok) = max_tokens {
        options["num_predict"] = serde_json::json!(max_tok);
    }
    if let Some(context) = num_ctx {
        options["num_ctx"] = serde_json::json!(context);
    }
    options
}

/// Check Ollama daemon health.
#[tauri::command]
pub fn ollama_health_check() -> Result<bool, String> {
    let response = client()
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
    let response = client()
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

/// Resolve the model name to the digest Ollama currently serves. A mutable tag
/// such as `:latest` is not a version; the digest is.
pub fn model_identity(model: &str) -> ModelIdentity {
    let digest = client()
        .get(format!("{}/api/tags", OLLAMA_BASE_URL))
        .timeout(Duration::from_secs(5))
        .send()
        .ok()
        .filter(|response| response.status().is_success())
        .and_then(|response| response.json::<OllamaListResponse>().ok())
        .and_then(|list| {
            let exact = list
                .models
                .iter()
                .find(|candidate| candidate.name == model)
                .and_then(|candidate| candidate.digest.clone());
            exact.or_else(|| {
                let latest = format!("{}:latest", model);
                list.models
                    .iter()
                    .find(|candidate| candidate.name == latest)
                    .and_then(|candidate| candidate.digest.clone())
            })
        });

    ModelIdentity {
        name: model.to_string(),
        digest,
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
    chat_with_ollama_ctx(model, messages, temperature, max_tokens, format, None)
}

/// Chat completion with an explicit context window. Generation paths pass
/// `OllamaConfig::num_ctx`; the frontend chat command leaves it unset.
pub fn chat_with_ollama_ctx(
    model: String,
    messages: Vec<OllamaChatMessage>,
    temperature: Option<f64>,
    max_tokens: Option<u32>,
    format: Option<String>,
    num_ctx: Option<u32>,
) -> Result<OllamaChatResponse, String> {
    let options = request_options(temperature, max_tokens, num_ctx);

    // A JSON-contract call needs the answer, not a monologue. Reasoning tokens
    // count against `num_predict` and never reach this function, which is how a
    // review ends up cut off after its first array. Ollama ignores `think` on
    // models that do not support it.
    let format = format.map(|format| match serde_json::from_str::<serde_json::Value>(&format) {
        Ok(value) if value.is_object() => value,
        _ => serde_json::Value::String(format),
    });
    let think = if format.is_some() { Some(false) } else { None };

    let request = OllamaChatRequest {
        model: model.clone(),
        messages: messages.clone(),
        stream: Some(false),
        format: format.clone(),
        think,
        options: Some(options.clone()),
    };

    let mut last_err: Option<String> = None;
    for attempt in 0..3 {
        let response = client()
            .post(format!("{}/api/chat", OLLAMA_BASE_URL))
            .header("Connection", "keep-alive")
            .timeout(Duration::from_secs(300))
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

/// Chat completion, executed off the async worker threads.
///
/// The blocking client parks its thread until Ollama answers, so async callers
/// must reach it through this wrapper rather than calling the sync function
/// directly from an async command.
pub async fn chat_with_ollama_async(
    model: String,
    messages: Vec<OllamaChatMessage>,
    temperature: Option<f64>,
    max_tokens: Option<u32>,
    format: Option<String>,
    num_ctx: Option<u32>,
) -> Result<OllamaChatResponse, String> {
    tokio::task::spawn_blocking(move || {
        chat_with_ollama_ctx(model, messages, temperature, max_tokens, format, num_ctx)
    })
    .await
    .map_err(|error| format!("Ollama chat task failed: {}", error))?
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
    generate_with_ollama_ctx(model, prompt, system, temperature, max_tokens, format, None)
}

/// Text generation with an explicit context window.
pub fn generate_with_ollama_ctx(
    model: String,
    prompt: String,
    system: Option<String>,
    temperature: Option<f64>,
    max_tokens: Option<u32>,
    format: Option<String>,
    num_ctx: Option<u32>,
) -> Result<OllamaGenerateResponse, String> {
    let options = request_options(temperature, max_tokens, num_ctx);

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
        let response = client()
            .post(format!("{}/api/generate", OLLAMA_BASE_URL))
            .header("Connection", "keep-alive")
            .timeout(Duration::from_secs(300))
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

/// Generation, executed off the async worker threads.
pub async fn generate_with_ollama_async(
    model: String,
    prompt: String,
    system: Option<String>,
    temperature: Option<f64>,
    max_tokens: Option<u32>,
    format: Option<String>,
    num_ctx: Option<u32>,
) -> Result<OllamaGenerateResponse, String> {
    tokio::task::spawn_blocking(move || {
        generate_with_ollama_ctx(model, prompt, system, temperature, max_tokens, format, num_ctx)
    })
    .await
    .map_err(|error| format!("Ollama generate task failed: {}", error))?
}

/// Resolve a model digest without blocking the async worker threads.
pub async fn model_identity_async(model: String) -> ModelIdentity {
    let name = model.clone();
    match tokio::task::spawn_blocking(move || model_identity(&model)).await {
        Ok(identity) => identity,
        Err(_) => ModelIdentity {
            name,
            digest: None,
        },
    }
}

/// Save Ollama model role configuration (fast/deep) to app_settings.
#[tauri::command]
pub fn save_ollama_config(
    fast_model: Option<String>,
    deep_model: Option<String>,
    num_ctx: Option<u32>,
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
    if let Some(context) = num_ctx {
        let context = if context < 2048 { 2048 } else { context };
        conn.execute(
            "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('ollama_num_ctx', ?1)",
            [context.to_string()],
        ).map_err(|e| format!("Failed to save context window: {}", e))?;
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

    let saved_num_ctx: Option<u32> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'ollama_num_ctx'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok());
    let num_ctx = saved_num_ctx.unwrap_or(DEFAULT_NUM_CTX);
    if saved_num_ctx.is_none() {
        let _ = conn.execute(
            "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('ollama_num_ctx', ?1)",
            [num_ctx.to_string()],
        );
    }

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
            deep_model: Some(default_model),
            num_ctx: Some(num_ctx),
        });
    }

    Ok(OllamaConfig {
        fast_model,
        deep_model,
        num_ctx: Some(num_ctx),
    })
}

#[cfg(test)]
mod tests {
    use super::{request_options, DEFAULT_NUM_CTX};

    #[test]
    fn num_ctx_reaches_the_options_payload() {
        let options = request_options(Some(0.3), Some(2000), Some(16_384));
        assert_eq!(options["num_ctx"], serde_json::json!(16_384));
        assert_eq!(options["temperature"], serde_json::json!(0.3));
        assert_eq!(options["num_predict"], serde_json::json!(2000));
    }

    #[test]
    fn num_ctx_is_omitted_when_unset() {
        let options = request_options(None, None, None);
        assert!(options.get("num_ctx").is_none());
    }

    #[test]
    fn default_context_covers_the_prompt_ceiling() {
        // 32 KB resume + 24 KB claims + 12 KB posting is roughly 18k tokens.
        assert!(DEFAULT_NUM_CTX >= 20_480);
    }
}

/// Stream model pull progress via text output (sequential lines).
#[tauri::command]
pub fn pull_ollama_model_progress(model_name: String) -> Result<String, String> {
    let response = client()
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