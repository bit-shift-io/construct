//! Provider implementations for LLM API wrapper
//!
//! This module contains implementations for different LLM providers:
//! - OpenAI-compatible API (OpenAI, Groq, XAI, DeepAI, Zai)
//! - Anthropic (Claude) with native prompt caching
//! - Gemini with native context caching

mod anthropic;
mod gemini;
mod openai;

use crate::core::config::AgentConfig;
use crate::llm::{Context, Error, Provider, Response};

/// Configuration for a provider
#[derive(Clone)]
pub struct ProviderConfig {
    /// API key
    pub api_key: String,
    /// Base URL (for non-default endpoints)
    pub base_url: Option<String>,
    /// Default model
    pub default_model: String,
}

impl ProviderConfig {
    pub fn from_agent_config(config: &AgentConfig) -> Result<Self, Error> {
        let api_key = if let Some(key) = &config.api_key {
            key.clone()
        } else if let Some(env_var) = &config.api_key_env {
            std::env::var(env_var).map_err(|e| {
                Error::new(
                    &config.provider,
                    format!("API key env var {} not set: {}", env_var, e),
                )
            })?
        } else {
            return Err(Error::new(
                &config.provider,
                "No API key provided - set api_key or api_key_env",
            ));
        };

        Ok(Self {
            api_key,
            base_url: config.endpoint.clone(),
            default_model: config.model.clone(),
        })
    }


}

/// Execute a chat request with the specified provider
pub async fn chat(
    provider: Provider,
    config: ProviderConfig,
    context: Context,
) -> Result<Response, Error> {
    match provider {
        Provider::OpenAI => openai::chat(config, context).await,
        Provider::Groq => {
            // Groq uses OpenAI-compatible API
            let config_with_url = ProviderConfig {
                base_url: Some("https://api.groq.com/openai/v1".to_string()),
                ..config
            };
            openai::chat(config_with_url, context).await
        }
        Provider::XAI => {
            // xAI uses OpenAI-compatible API
            let config_with_url = ProviderConfig {
                base_url: Some("https://api.x.ai/v1".to_string()),
                ..config
            };
            openai::chat(config_with_url, context).await
        }
        Provider::DeepAI => {
            // DeepAI uses OpenAI-compatible API
            let config_with_url = ProviderConfig {
                base_url: Some("https://api.deepai.com/v1".to_string()),
                ..config
            };
            openai::chat(config_with_url, context).await
        }
        Provider::Zai => {
            // Zai uses OpenAI-compatible API with custom endpoint
            let base_url = config
                .base_url
                .unwrap_or_else(|| "https://api.z.ai/api/coding/paas".to_string());
            let config_with_url = ProviderConfig {
                base_url: Some(format!("{}/v4/responses", base_url)),
                ..config
            };
            openai::chat(config_with_url, context).await
        }
        Provider::Anthropic => anthropic::chat(config, context).await,
        Provider::Gemini => gemini::chat(config, context).await,
    }
}
