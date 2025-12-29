//! DeepAI provider implementation
//!
//! This module provides integration with DeepAI's API for basic AI tasks.
//!
//! ## Supported Models
//! - `standard` - Standard text generation model (default)
//! - `text-generator` - Text generation model
//!
//! ## Configuration
//! Set the `DEEPAI_API_KEY` environment variable or provide `api_key` in config.
//!
//! ## Example
//! ```yaml
//! agents:
//!   deepai:
//!     protocol: "deepai"
//!     model: "standard"
//!     requests_per_minute: 10  # Rate limiting (optional)
//! ```

use serde::Deserialize;

use crate::agent::AgentContext;
use crate::agent::rate_limiter::RateLimiter;
use crate::config::AgentConfig;

/// Default model for DeepAI provider
pub const DEFAULT_MODEL: &str = "standard";

/// Response structure from DeepAI API
#[derive(Deserialize)]
struct DeepAIResponse {
    output: String,
}

/// Execute a prompt using the DeepAI provider
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
    _model_name: &str,
) -> Result<String, String> {
    let rate_limiter = RateLimiter::from_config(config, 3);
    let config_clone = config.clone();
    let prompt_clone = context.prompt.clone();

    rate_limiter
        .execute_with_retry(
            move || async {
                let api_key = if let Some(key) = &config_clone.api_key {
                    key.clone()
                } else {
                    std::env::var("DEEPAI_API_KEY").map_err(|_| "Missing DEEPAI_API_KEY")?
                };

                let client = reqwest::Client::new();
                let resp = client
                    .post("https://api.deepai.org/api/text-generator")
                    .header("api-key", api_key)
                    .form(&[("text", &prompt_clone)])
                    .send()
                    .await
                    .map_err(|e| {
                        crate::strings::STRINGS
                            .messages
                            .deepai_request_failed
                            .replace("{}", &e.to_string())
                    })?;

                if !resp.status().is_success() {
                    return Err(crate::strings::STRINGS
                        .messages
                        .deepai_api_error
                        .replace("{}", &resp.status().to_string()));
                }

                let body: DeepAIResponse = resp.json().await.map_err(|e| {
                    crate::strings::STRINGS
                        .messages
                        .deepai_parse_error
                        .replace("{}", &e.to_string())
                })?;

                Ok(body.output)
            },
            context,
            "deepai",
        )
        .await
}

/// Get the default model name for DeepAI
pub fn get_default_model() -> String {
    DEFAULT_MODEL.to_string()
}

/// Check if DeepAI is properly configured
pub fn is_configured() -> bool {
    std::env::var("DEEPAI_API_KEY").is_ok() || std::env::var("DEEPAI_API_KEY").is_ok()
}
