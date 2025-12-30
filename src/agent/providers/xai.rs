//! xAI (Grok) provider implementation
//!
//! This module provides integration with xAI's Grok models through an OpenAI-compatible API.
//!
//! ## Supported Models
//! - `grok-beta` - Latest Grok model (recommended)
//! - `grok-vision-beta` - Grok with vision capabilities
//!
//! ## Configuration
//! Set the `XAI_API_KEY` environment variable.
//!
//! ## Example
//! ```yaml
//! agents:
//!   xai:
//!     protocol: "xai"
//!     model: "grok-beta"
//!     requests_per_minute: 50  # Rate limiting (optional)
//! ```

use rig::client::CompletionClient;
use rig::client::ProviderClient;
use rig::completion::Prompt;
use rig::providers::openai;

use crate::agent::AgentContext;
use crate::agent::rate_limiter::RateLimiter;
use crate::config::AgentConfig;

/// Default model for xAI provider
pub const DEFAULT_MODEL: &str = "grok-beta";

/// Execute a prompt using the xAI provider
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
                let api_key = std::env::var("XAI_API_KEY").map_err(|_| "Missing XAI_API_KEY")?;

                unsafe {
                    std::env::set_var("OPENAI_BASE_URL", "https://api.x.ai/v1");
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
            "xai",
        )
        .await
}

/// Get the default model name for xAI
pub fn get_default_model() -> String {
    DEFAULT_MODEL.to_string()
}

/// Check if xAI is properly configured
pub fn is_configured() -> bool {
    std::env::var("XAI_API_KEY").is_ok()
}
