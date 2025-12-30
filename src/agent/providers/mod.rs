//! AI Provider implementations
//!
//! This module contains implementations for various AI providers.
//! Each provider module exports:
//! - `execute()` - Main execution function
//! - `get_default_model()` - Default model for the provider
//! - `is_configured()` - Check if provider has required credentials
//! - `DEFAULT_MODEL` - Constant for default model name

pub mod anthropic;
pub mod deepai;
pub mod gemini;
pub mod groq;
pub mod openai;
pub mod xai;
pub mod zai;

use crate::agent::AgentContext;
use crate::config::AgentConfig;

/// Provider trait for unified interface
pub trait Provider {
    /// Execute a prompt using this provider
    async fn execute(
        &self,
        config: &AgentConfig,
        context: &AgentContext,
        model: &str,
    ) -> Result<String, String>;

    /// Get the default model for this provider
    fn get_default_model(&self) -> String;

    /// Check if this provider is properly configured
    fn is_configured(&self) -> bool;
}

/// Get the default model for a given provider protocol
pub fn get_default_model(protocol: &str) -> String {
    match protocol {
        "zai" => zai::get_default_model(),
        "gemini" => gemini::get_default_model(),
        "claude" | "anthropic" => anthropic::get_default_model(),
        "openai" => openai::get_default_model(),
        "groq" => groq::get_default_model(),
        "xai" => xai::get_default_model(),
        "deepai" | "deep_ai" => deepai::get_default_model(),
        _ => "default".to_string(),
    }
}

/// Execute a prompt using the specified provider protocol
pub async fn execute_provider(
    protocol: &str,
    config: &AgentConfig,
    context: &AgentContext,
    model: &str,
) -> Result<String, String> {
    match protocol {
        "zai" => zai::execute(config, context, model).await,
        "gemini" => gemini::execute(config, context, model).await,
        "claude" | "anthropic" => anthropic::execute(config, context, model).await,
        "openai" => openai::execute(config, context, model).await,
        "groq" => groq::execute(config, context, model).await,
        "xai" => xai::execute(config, context, model).await,
        "deepai" | "deep_ai" => deepai::execute(config, context, model).await,
        _ => Err(format!("Unsupported provider: {}", protocol)),
    }
}

/// Check if a provider is properly configured
pub fn is_provider_configured(protocol: &str) -> bool {
    match protocol {
        "zai" => zai::is_configured(),
        "gemini" => gemini::is_configured(),
        "claude" | "anthropic" => anthropic::is_configured(),
        "openai" => openai::is_configured(),
        "groq" => groq::is_configured(),
        "xai" => xai::is_configured(),
        "deepai" | "deep_ai" => deepai::is_configured(),
        _ => false,
    }
}

/// List available models for a given provider protocol
pub async fn list_models(protocol: &str, config: &AgentConfig) -> Result<Vec<String>, String> {
    match protocol {
        "zai" => zai::list_models(config).await,
        "gemini" => gemini::list_models(config).await,
        "claude" | "anthropic" => anthropic::list_models(config).await,
        "openai" => openai::list_models(config).await,
        "groq" => groq::list_models(config).await,
        // xai, deepai, etc. might not support discovery yet or follow different patterns
        // For now, return empty or implement if possible.
        // deepai doesn't seem to have list_models.
        // xai uses openai protocol but might not have listing endpoint in xai.rs yet?
        // Let's check xai.rs content if needed. Assuming no for now to be safe.
        _ => Err(format!(
            "Model discovery not supported for provider: {}",
            protocol
        )),
    }
}
