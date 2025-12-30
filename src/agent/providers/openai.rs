//! OpenAI provider implementation
//!
//! This module provides integration with OpenAI's GPT models.
//!
//! ## Supported Models
//! - `gpt-4o` - Latest GPT-4 model (recommended)
//! - `gpt-4o-mini` - Smaller, faster GPT-4
//! - `gpt-4-turbo` - GPT-4 Turbo
//! - `gpt-3.5-turbo` - GPT-3.5 Turbo
//!
//! ## Configuration
//! Set the `OPENAI_API_KEY` environment variable.
//!
//! ## Example
//! ```yaml
//! agents:
//!   openai:
//!     protocol: "openai"
//!     model: "gpt-4o"
//!     requests_per_minute: 50  # Rate limiting (optional)
//! ```

use regex::Regex;
use rig::client::CompletionClient;
use rig::client::ProviderClient;
use rig::completion::Prompt;
use rig::providers::openai;
use serde::Deserialize;
use std::time::Duration;

use crate::agent::AgentContext;
use crate::agent::rate_limiter::RateLimiter;
use crate::config::AgentConfig;

/// Default model for OpenAI provider
pub const DEFAULT_MODEL: &str = "gpt-4o";

#[derive(Debug, Deserialize)]
struct OpenAIModel {
    id: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIListResponse {
    data: Vec<OpenAIModel>,
}

/// List available models from the OpenAI-compatible endpoint
pub async fn list_models(config: &AgentConfig) -> Result<Vec<String>, String> {
    let api_key = if let Some(key) = &config.api_key {
        key.clone()
    } else if let Some(env_var) = &config.api_key_env {
        std::env::var(env_var).unwrap_or_default()
    } else {
        std::env::var("OPENAI_API_KEY").unwrap_or_default()
    };

    if api_key.is_empty() {
        return Err("Missing OpenAI API Key".to_string());
    }

    let endpoint = config
        .endpoint
        .clone()
        .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
    let base_url = endpoint.trim_end_matches('/');
    let url = format!("{}/models", base_url);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .map_err(|e| format!("Failed to fetch OpenAI models: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("OpenAI API Error: {}", resp.status()));
    }

    let body: OpenAIListResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse OpenAI response: {}", e))?;

    Ok(body.data.into_iter().map(|m| m.id).collect())
}

/// Execute a prompt using the OpenAI provider
pub async fn execute(
    config: &AgentConfig,
    context: &AgentContext,
    model_name: &str,
) -> Result<String, String> {
    let rate_limiter = RateLimiter::from_config(config, 3);

    let api_key = if let Some(key) = &config.api_key {
        key.clone()
    } else if let Some(env_var) = &config.api_key_env {
        std::env::var(env_var).unwrap_or_default()
    } else {
        std::env::var("OPENAI_API_KEY").unwrap_or_default()
    };

    if api_key.is_empty() {
        return Err("Missing OpenAI API Key".to_string());
    }

    let endpoint_url = config
        .endpoint
        .clone()
        .unwrap_or_else(|| "https://api.openai.com/v1".to_string());

    let mut models_to_try = Vec::new();

    // Priority 1: User Override
    if !model_name.is_empty() && model_name != "default" {
        models_to_try.push(model_name.to_string());
    }

    // Priority 2: Discovered API Models
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

    // Priority 3: Fallbacks
    if let Some(fallbacks) = &config.model_fallbacks {
        for m in fallbacks {
            if !models_to_try.contains(m) {
                models_to_try.push(m.clone());
            }
        }
    } else {
        // Minimal hardcoded fallback ONLY if using standard OpenAI endpoint
        if config.endpoint.is_none() {
            if !models_to_try.contains(&"gpt-4o".to_string()) {
                models_to_try.push("gpt-4o".to_string());
            }
        }
    }

    let mut last_error = "No models available".to_string();

    for model in models_to_try {
        let api_key = api_key.clone();
        let endpoint = endpoint_url.clone();
        let prompt = context.prompt.clone();
        let model_clone = model.clone();

        let result = rate_limiter
            .execute_with_retry(
                move || {
                    let api_key = api_key.clone();
                    let endpoint = endpoint.clone();
                    let model = model_clone.clone();
                    let prompt = prompt.clone();
                    async move {
                        unsafe {
                            std::env::set_var("OPENAI_BASE_URL", endpoint);
                            std::env::set_var("OPENAI_API_KEY", &api_key);
                        }

                        let client = openai::Client::from_env();
                        let agent = client.agent(&model).build();

                        agent.prompt(&prompt).await.map_err(|e| e.to_string())
                    }
                },
                context,
                "openai",
            )
            .await;

        match result {
            Ok(response) => return Ok(response),
            Err(e) => {
                last_error = format!("Model {} failed: {}", model, e);
                continue;
            }
        }
    }

    Err(last_error)
}

/// Get the default model name for OpenAI
pub fn get_default_model() -> String {
    DEFAULT_MODEL.to_string()
}

/// Check if OpenAI is properly configured
pub fn is_configured() -> bool {
    std::env::var("OPENAI_API_KEY").is_ok()
}
