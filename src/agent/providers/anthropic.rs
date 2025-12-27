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
//! ```

use rig::client::CompletionClient;
use rig::providers::anthropic;

use crate::agent::AgentContext;
use crate::config::AgentConfig;

/// Default model for Anthropic provider
pub const DEFAULT_MODEL: &str = "claude-3-5-sonnet-20241022";

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
    let client = anthropic::Client::from_env();
    let agent = client.agent(model_name).build();

    agent
        .prompt(&context.prompt)
        .await
        .map_err(|e| e.to_string())
}

/// Get the default model name for Anthropic
pub fn get_default_model() -> String {
    DEFAULT_MODEL.to_string()
}

/// Check if Anthropic is properly configured
pub fn is_configured() -> bool {
    std::env::var("ANTHROPIC_API_KEY").is_ok()
}
