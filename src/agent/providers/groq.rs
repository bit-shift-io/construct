//! Groq provider implementation
//!
//! This module provides integration with Groq's fast inference API.
//!
//! ## Supported Models
//! - `llama-3.3-70b-versatile` - Latest Llama (recommended)
//! - `llama-3.1-70b-versatile` - Llama 3.1 70B
//! - `llama-3.1-8b-instant` - Fast and lightweight
//! - `mixtral-8x7b-32768` - Mixtral model
//!
//! ## Configuration
//! Set the `GROQ_API_KEY` environment variable.
//!
//! ## Example
//! ```yaml
//! agents:
//!   groq:
//!     protocol: "groq"
//!     model: "llama-3.3-70b-versatile"
//!     requests_per_minute: 30
//! ```

use rig::client::CompletionClient;
use rig::client::ProviderClient;
use rig::completion::Prompt;
use rig::providers::openai;
use serde::Deserialize;
use std::time::Duration;

use crate::agent::AgentContext;
use crate::agent::rate_limiter::RateLimiter;
use crate::config::AgentConfig;

/// Default model for Groq provider
pub const DEFAULT_MODEL: &str = "llama-3.3-70b-versatile";

#[derive(Debug, Deserialize)]
struct OpenAIModel {
    id: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIListResponse {
    data: Vec<OpenAIModel>,
}

/// List available models from the Groq API
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
        std::env::var("GROQ_API_KEY").map_err(|_| "Missing GROQ_API_KEY")?
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get("https://api.groq.com/openai/v1/models")
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .map_err(|_e| "Failed to fetch Groq models".to_string())?;

    if !resp.status().is_success() {
        return Err(format!("Groq API Error: {}", resp.status()));
    }

    let body: OpenAIListResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Groq response: {}", e))?;

    Ok(body.data.into_iter().map(|m| m.id).collect())
}

/// Execute a prompt using the Groq provider
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
                let api_key = std::env::var("GROQ_API_KEY").map_err(|_| "Missing GROQ_API_KEY")?;

                unsafe {
                    std::env::set_var("OPENAI_BASE_URL", "https://api.groq.com/openai/v1");
                    std::env::set_var("OPENAI_API_KEY", &api_key);
                }

                let client = openai::Client::from_env();
                let agent = client.agent(model_name).build();

                agent
                    .prompt(&context.prompt)
                    .await
                    .map_err(|e| e.to_string())
            },
            context,
            "groq",
        )
        .await
}

/// Get the default model name for Groq
pub fn get_default_model() -> String {
    DEFAULT_MODEL.to_string()
}

/// Check if Groq is properly configured
pub fn is_configured() -> bool {
    std::env::var("GROQ_API_KEY").is_ok()
}
