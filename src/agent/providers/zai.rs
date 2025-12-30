//! Zai provider implementation for GLM models
//!
//! This module provides integration with Zai's GLM (General Language Model) models
//! through an Anthropic-compatible API endpoint.
//!
//! ## Supported Models
//! - `glm-4.7` - Latest flagship model (64K tokens, best for complex tasks)
//! - `glm-4.6` - High performance with 200K context
//! - `glm-4.5` - Base model (32K tokens)
//! - `glm-4.5-x` - Enhanced version
//! - `glm-4.5-air` - Lightweight
//! - `glm-4.5-airx` - Ultra-lightweight
//! - `glm-4.5-flash` - Fast responses (8K tokens)
//!
//! ## Configuration
//! Set the `ZAI_API_KEY` environment variable with your Zai API key.
//!
//! ## Example
//! ```yaml
//! agents:
//!   zai:
//!     protocol: "zai"
//!     model: "glm-4.7"
//!     requests_per_minute: 60  # Rate limiting (optional)
//! ```

use rig::client::CompletionClient;
use rig::client::ProviderClient;
use rig::completion::Prompt;
use rig::providers::zai;
use serde::Deserialize;
use std::time::Duration;

use crate::agent::AgentContext;
use crate::agent::rate_limiter::RateLimiter;
use crate::config::AgentConfig;

/// Default model for Zai provider
pub const DEFAULT_MODEL: &str = "glm-4.7";

#[derive(Debug, Deserialize)]
struct AnthropicModel {
    id: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicListResponse {
    data: Vec<AnthropicModel>,
}

/// List available models from the Zai API
pub async fn list_models(config: &AgentConfig) -> Result<Vec<String>, String> {
    let api_key = if let Some(k) = &config.api_key {
        k.clone()
    } else if let Some(env_var) = &config.api_key_env {
        std::env::var(env_var).map_err(|_| {
            crate::strings::STRINGS
                .messages
                .missing_env_var
                .replace("{}", env_var)
        })?
    } else {
        std::env::var("ZAI_API_KEY").map_err(|_| "Missing ZAI_API_KEY")?
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get("https://api.z.ai/api/anthropic/v1/models")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .send()
        .await
        .map_err(|e| {
            crate::strings::STRINGS
                .messages
                .anthropic_fetch_failed
                .replace("{}", &e.to_string())
        })?;

    if !resp.status().is_success() {
        return Err(crate::strings::STRINGS
            .messages
            .anthropic_api_error
            .replace("{}", &resp.status().to_string()));
    }

    let body: AnthropicListResponse = resp.json().await.map_err(|e| {
        crate::strings::STRINGS
            .messages
            .anthropic_parse_error
            .replace("{}", &e.to_string())
    })?;

    Ok(body.data.into_iter().map(|m| m.id).collect())
}

/// Execute a prompt using the Zai provider
///
/// # Arguments
/// * `config` - Agent configuration
/// * `context` - Agent execution context
/// * `model_name` - Specific model to use (e.g., "glm-4.7")
///
/// # Returns
/// The model's response as a String, or an error message
pub async fn execute(
    config: &AgentConfig,
    context: &AgentContext,
    model_name: &str,
) -> Result<String, String> {
    let rate_limiter = RateLimiter::from_config(config, 3);

    rate_limiter
        .execute_with_retry(
            || async {
                let client = zai::Client::from_env();
                let agent = client.agent(model_name).build();

                agent
                    .prompt(&context.prompt)
                    .await
                    .map_err(|e| e.to_string())
            },
            context,
            "zai",
        )
        .await
}

/// Get the default model name for Zai
pub fn get_default_model() -> String {
    DEFAULT_MODEL.to_string()
}

/// Check if Zai is properly configured
pub fn is_configured() -> bool {
    std::env::var("ZAI_API_KEY").is_ok()
}
