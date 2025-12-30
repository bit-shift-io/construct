//! Anthropic/Claude provider implementation
//!
//! This module provides integration with Anthropic's Claude models.
//!
//! ## Supported Models
//! - `claude-3-5-sonnet-20241022` - Latest Sonnet model (recommended)
//! - `claude-3-5-haiku-20241022` - Fast and efficient
//! - `claude-3-opus-20240229` - Most capable
//!
//! ## Configuration
//! Set the `ANTHROPIC_API_KEY` environment variable.
//!
//! ## Example
//! ```yaml
//! agents:
//!   claude:
//!     protocol: "claude"  # or "anthropic"
//!     model: "claude-3-5-sonnet-20241022"
//!     requests_per_minute: 50  # Rate limiting (optional)
//! ```

use rig::client::CompletionClient;
use rig::client::ProviderClient;
use rig::completion::Prompt;
use rig::providers::anthropic;
use serde::Deserialize;
use std::time::Duration;

use crate::agent::AgentContext;
use crate::agent::rate_limiter::RateLimiter;
use crate::config::AgentConfig;

/// Default model for Anthropic provider
pub const DEFAULT_MODEL: &str = "claude-3-5-sonnet-20241022";

#[derive(Debug, Deserialize)]
struct AnthropicModel {
    id: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicListResponse {
    data: Vec<AnthropicModel>,
}

/// List available models from the Anthropic API
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
        std::env::var("ANTHROPIC_API_KEY").map_err(|_| "Missing ANTHROPIC_API_KEY")?
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get("https://api.anthropic.com/v1/models")
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

/// Execute a prompt using the Anthropic provider
///
/// # Arguments
/// * `config` - Agent configuration
/// * `context` - Agent execution context
/// * `model_name` - Specific model to use
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
                let client = anthropic::Client::from_env();
                let agent = client.agent(model_name).build();

                agent
                    .prompt(&context.prompt)
                    .await
                    .map_err(|e| e.to_string())
            },
            context,
            "anthropic",
        )
        .await
}

/// Get the default model name for Anthropic
pub fn get_default_model() -> String {
    DEFAULT_MODEL.to_string()
}

/// Check if Anthropic is properly configured
pub fn is_configured() -> bool {
    std::env::var("ANTHROPIC_API_KEY").is_ok()
}
