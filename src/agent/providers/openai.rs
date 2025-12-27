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
//! ```

use rig::completion::Prompt;
use rig::providers::openai;

use crate::agent::AgentContext;
use crate::config::AgentConfig;

/// Default model for OpenAI provider
pub const DEFAULT_MODEL: &str = "gpt-4o";

/// Execute a prompt using the OpenAI provider
///
/// # Arguments
/// * `config` - Agent configuration
/// * `context` - Agent execution context
/// * `model_name` - Specific model to use
///
/// # Returns
/// The model's response as a String, or an error message
pub async fn execute(
    _config: &AgentConfig,
    context: &AgentContext,
    model_name: &str,
) -> Result<String, String> {
    let client = openai::Client::from_env();
    let agent = client.agent(model_name).build();

    agent
        .prompt(&context.prompt)
        .await
        .map_err(|e| e.to_string())
}

/// Get the default model name for OpenAI
pub fn get_default_model() -> String {
    DEFAULT_MODEL.to_string()
}

/// Check if OpenAI is properly configured
pub fn is_configured() -> bool {
    std::env::var("OPENAI_API_KEY").is_ok()
}
