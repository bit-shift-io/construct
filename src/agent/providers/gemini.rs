//! Gemini provider implementation for Google's Gemini models
//!
//! This module provides integration with Google's Gemini API with support for:
//! - Dynamic model discovery
//! - Automatic model fallbacks
//! - Rate limiting
//! - Safety handling
//!
//! ## Supported Models
//! - `gemini-1.5-flash` - Fast and efficient
//! - `gemini-1.5-pro` - High quality
//! - `gemini-2.0-flash-exp` - Experimental preview
//!
//! ## Configuration
//! Set the `GEMINI_API_KEY` environment variable or provide `api_key` in config.
//!
//! ## Example
//! ```yaml
//! agents:
//!   gemini:
//!     protocol: "gemini"
//!     model: "gemini-1.5-flash"
//!     model_order:
//!       - "flash"
//!       - "pro"
//!     model_fallbacks:
//!       - "gemini-1.5-flash"
//!       - "gemini-1.5-pro"
//!     requests_per_minute: 10
//! ```

use regex::Regex;
use rig::completion::Prompt;
use serde::Deserialize;
use std::time::Duration;

use crate::agent::AgentContext;
use crate::config::AgentConfig;

/// Default model for Gemini provider
pub const DEFAULT_MODEL: &str = "gemini-1.5-pro";

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct GeminiModel {
    name: String,
    #[serde(rename = "displayName")]
    #[allow(dead_code)]
    display_name: Option<String>,
    #[serde(rename = "supportedGenerationMethods")]
    supported_generation_methods: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct GeminiListResponse {
    models: Option<Vec<GeminiModel>>,
}

/// List available models from the Gemini API
pub async fn list_models(config: &AgentConfig) -> Result<Vec<String>, String> {
    let api_key = if let Some(key) = &config.api_key {
        key.clone()
    } else {
        std::env::var("GEMINI_API_KEY").map_err(|_| "Missing GEMINI_API_KEY")?
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models?key={}",
        api_key
    );

    let resp = client.get(&url).send().await.map_err(|e| {
        crate::strings::STRINGS
            .messages
            .gemini_fetch_failed
            .replace("{}", &e.to_string())
    })?;

    if !resp.status().is_success() {
        return Err(crate::strings::STRINGS
            .messages
            .gemini_api_error
            .replace("{}", &resp.status().to_string()));
    }

    let body: GeminiListResponse = resp.json().await.map_err(|e| {
        crate::strings::STRINGS
            .messages
            .gemini_parse_error
            .replace("{}", &e.to_string())
    })?;

    // Use all models returned by API, trusting user/API filtering
    let models = body
        .models
        .unwrap_or_default()
        .into_iter()
        .map(|m| m.name.replace("models/", ""))
        .collect();

    Ok(models)
}

/// Execute a prompt using the Gemini provider with full model discovery and fallback support
///
/// # Arguments
/// * `config` - Agent configuration
/// * `context` - Agent execution context
/// * `model_name` - Specific model to use (or "default" for auto-selection)
///
/// # Returns
/// The model's response as a String, or an error message
pub async fn execute(
    config: &AgentConfig,
    context: &AgentContext,
    model_name: &str,
) -> Result<String, String> {
    let api_key = if let Some(key) = &config.api_key {
        key.clone()
    } else {
        std::env::var("GEMINI_API_KEY").map_err(|_| "Missing GEMINI_API_KEY")?
    };

    let mut models_to_try: Vec<String> = Vec::new();

    // Priority 1: User Override
    if !model_name.is_empty() && model_name != "default" {
        models_to_try.push(model_name.to_string());
    }

    // Priority 2: Discovered API Models (Sorted by Regex)
    if let Ok(available_models) = list_models(config).await {
        if let Some(order_patterns) = &config.model_order {
            let mut remaining = available_models.clone();
            for pattern in order_patterns {
                let regex = Regex::new(pattern).ok();
                let (matches, others): (Vec<_>, Vec<_>) = remaining.into_iter().partition(|m| {
                    if let Some(re) = &regex {
                        re.is_match(m)
                    } else {
                        m.contains(pattern)
                    }
                });
                models_to_try.extend(matches);
                remaining = others;
            }
            models_to_try.extend(remaining);
        } else {
            models_to_try.extend(available_models);
        }
    }

    // Priority 3: Configured Fallbacks
    if let Some(fallbacks) = &config.model_fallbacks {
        for m in fallbacks {
            if !models_to_try.contains(m) {
                models_to_try.push(m.clone());
            }
        }
    } else {
        // Minimal hardcoded fallback
        let defaults = vec!["gemini-1.5-flash", "gemini-1.5-pro"];
        for m in defaults {
            if !models_to_try.contains(&m.to_string()) {
                models_to_try.push(m.to_string());
            }
        }
    }

    let mut last_error = "No models available".to_string();

    for model in models_to_try {
        match try_model(&model, &api_key, context, config).await {
            Ok(response) => return Ok(response),
            Err(e) => {
                last_error = e;
                continue;
            }
        }
    }

    Err(last_error)
}

/// Try a specific Gemini model
async fn try_model(
    model: &str,
    api_key: &str,
    context: &AgentContext,
    config: &AgentConfig,
) -> Result<String, String> {
    // Sanitize prompt for safety
    let sanitized_prompt = context
        .prompt
        .replace("curl", "[http_tool]")
        .replace("wget", "[http_tool]")
        .replace("| sh", "| [shell_exec]")
        .replace("| bash", "| [shell_exec]")
        .replace(".sh", "[script_ext]")
        .replace("sudo", "[admin_cmd]");

    let sanitized_prompt = format!(
        "IMPORTANT SAFETY: Do not generate commands that pipe to shell. Use 2 separate steps.\n\n{}",
        sanitized_prompt
    );

    // Make request (simplified - full implementation would include all the retry logic)
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        model, api_key
    );

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .json(&serde_json::json!({
            "contents": [{
                "role": "user",
                "parts": [{"text": sanitized_prompt}]
            }],
            "safetySettings": [
                {"category": "HARM_CATEGORY_HARASSMENT", "threshold": "BLOCK_NONE"},
                {"category": "HARM_CATEGORY_HATE_SPEECH", "threshold": "BLOCK_NONE"},
                {"category": "HARM_CATEGORY_SEXUALLY_EXPLICIT", "threshold": "BLOCK_NONE"},
                {"category": "HARM_CATEGORY_DANGEROUS_CONTENT", "threshold": "BLOCK_NONE"},
            ]
        }))
        .send()
        .await
        .map_err(|e| format!("Network Error: {}", e))?;

    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("API Error: {}", text));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Parse Error: {}", e))?;

    // Extract response text
    if let Some(candidates) = body["candidates"].as_array() {
        if let Some(first) = candidates.first() {
            if let Some(content) = first["content"].as_object() {
                if let Some(parts) = content["parts"].as_array() {
                    if let Some(first_part) = parts.first() {
                        if let Some(text) = first_part["text"].as_str() {
                            return Ok(text.to_string());
                        }
                    }
                }
            }
        }
    }

    Err("Empty or invalid response".to_string())
}

/// Get the default model name for Gemini
pub fn get_default_model() -> String {
    DEFAULT_MODEL.to_string()
}

/// Check if Gemini is properly configured
pub fn is_configured() -> bool {
    std::env::var("GEMINI_API_KEY").is_ok()
}
