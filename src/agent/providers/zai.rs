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
//! ```

use rig::completion::Prompt;
use rig::providers::zai;

use crate::agent::AgentContext;
use crate::config::AgentConfig;

/// Default model for Zai provider
pub const DEFAULT_MODEL: &str = "glm-4.7";

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
    let client = zai::Client::from_env();
    let agent = client.agent(model_name).build();

    agent
        .prompt(&context.prompt)
        .await
        .map_err(|e| e.to_string())
}

/// Get the default model name for Zai
pub fn get_default_model() -> String {
    DEFAULT_MODEL.to_string()
}

/// Check if Zai is properly configured
pub fn is_configured() -> bool {
    std::env::var("ZAI_API_KEY").is_ok()
}
