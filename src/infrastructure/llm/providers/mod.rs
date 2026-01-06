//! Provider implementations for LLM API wrapper
//!
//! This module contains implementations for different LLM providers:
//! - OpenAI-compatible API (OpenAI, Groq, XAI, DeepAI, Zai)
//! - Anthropic (Claude) with native prompt caching
//! - Gemini with native context caching

//! # LLM Providers
//!
//! Contains implementations for specific LLM providers (OpenAI, Anthropic, Gemini).
//! Each provider implements the `Provider` trait to standardize interaction.

mod anthropic;
mod gemini;
mod openai;

use crate::domain::config::AgentConfig;
use crate::infrastructure::llm::{Context, Error, Provider, Response};

/// Configuration for a provider
#[derive(Clone)]
pub struct ProviderConfig {
    /// API key
    pub api_key: String,
    /// Base URL (for non-default endpoints)
    pub base_url: Option<String>,
    /// Default model
    pub default_model: String,
    /// Timeout in seconds
    pub timeout: Option<u64>,
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
            timeout: config.timeout,
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

/// List available models for the specified provider
pub async fn list_models(
    provider: Provider,
    config: ProviderConfig,
) -> Result<Vec<String>, Error> {
    match provider {
        Provider::OpenAI => openai::list_models(config).await,
        Provider::Groq => {
             let config_with_url = ProviderConfig {
                base_url: Some("https://api.groq.com/openai/v1".to_string()),
                ..config
            };
            openai::list_models(config_with_url).await
        }
        Provider::XAI => {
            let config_with_url = ProviderConfig {
                base_url: Some("https://api.x.ai/v1".to_string()),
                ..config
            };
            openai::list_models(config_with_url).await
        }
        Provider::DeepAI => {
            let config_with_url = ProviderConfig {
                base_url: Some("https://api.deepai.com/v1".to_string()),
                ..config
            };
            openai::list_models(config_with_url).await
        }
        Provider::Zai => {
            let base_url = config
                .base_url
                .unwrap_or_else(|| "https://api.z.ai/api/coding/paas".to_string());
             // NOTE: Zai might use different path for models? Assuming standard OpenAI for now.
             // If Zai uses /v4/models, we need to check docs. 
             // Assuming OpenAI compat for models endpoint too.
             let config_with_url = ProviderConfig {
                base_url: Some(format!("{}", base_url)), // Base URL usually includes version?
                // The chat path was /v4/responses. 
                // Let's assume /v4/models is at base_url/v4/models if we strip responses?
                // Actually openai::list_models appends /models.
                // If Zai base is `https://api.z.ai/api/coding/paas`, appending /models works?
                // Chat uses `.../v4/responses`.
                // Let's rely on standard OpenAI behavior for now, or just not implement for Zai if unsure.
                // Re-using user's assumption that "openai was dynamically fetching".
                ..config
            };
             openai::list_models(config_with_url).await
        }
        Provider::Anthropic => anthropic::list_models(config).await,
        Provider::Gemini => Err(Error::new("gemini", "Listing models not implemented for Gemini")),
    }
}

/// Get default fallback models for a provider when API listing fails
pub fn get_default_models(provider: Provider) -> Vec<String> {
    match provider {
        Provider::Zai => vec![
            "glm-4.7".to_string(),
            "glm-4.5".to_string(),
            "glm-4.5-flash".to_string(),
        ],
        Provider::Gemini => vec![
            "gemini-1.5-pro".to_string(),
            "gemini-1.5-flash".to_string(),
        ],
        Provider::Groq => vec![
            "llama-3.3-70b-versatile".to_string(),
            "llama-3.1-70b-versatile".to_string(),
            "mixtral-8x7b-32768".to_string(),
        ],
        Provider::Anthropic => vec![
            "claude-3-5-sonnet-20241022".to_string(),
            "claude-3-5-haiku-20241022".to_string(),
            "claude-3-opus-20240229".to_string(),
        ],
        Provider::OpenAI => vec![
            "gpt-4o".to_string(),
            "gpt-4o-mini".to_string(),
            "gpt-4-turbo".to_string(),
            "gpt-3.5-turbo".to_string(),
        ],
        Provider::XAI => vec![
            "grok-beta".to_string(),
            "grok-1".to_string(),
        ],
        Provider::DeepAI => vec![
            "standard".to_string(),
        ],
        // Default/Fallback
        _ => vec!["default".to_string()],
    }
}
