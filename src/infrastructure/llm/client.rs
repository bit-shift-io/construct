//! Simple LLM client for multiple providers with native caching support

//! # LLM Client
//!
//! Provides the `Client` struct, which acts as the main entry point for LLM interactions.
//! It routes requests to the appropriate provider based on configuration and handles response processing.

use crate::domain::config::AppConfig;
use crate::infrastructure::llm::providers;
use crate::infrastructure::llm::{Context, Error, Provider, Response};
use crate::domain::traits::LlmProvider;
use async_trait::async_trait;

/// Simple LLM client
pub struct Client {
    app_config: AppConfig,
}

impl Client {
    /// Create a new client from application configuration
    pub fn new(app_config: AppConfig) -> Self {
        Self { app_config }
    }

    /// Send a simple prompt to an agent
    ///
    /// # Arguments
    /// * `agent_name` - The agent name (e.g., "zai", "my-claude", "prod-gemini")
    /// * `prompt` - The prompt text
    ///
    /// # Example
    /// ```rust
    /// let response = client.prompt("zai", "Hello, world!").await?;
    /// println!("Response: {}", response.content);
    /// ```
    pub async fn prompt(&self, agent_name: &str, prompt: &str) -> Result<Response, Error> {
        // Look up agent configuration by agent name
        let agent_config = self
            .app_config
            .agents
            .get(agent_name)
            .ok_or_else(|| Error::new(agent_name, "Agent not found"))?;

        // Get provider type from agent config (e.g., "openai", "anthropic", "gemini")
        let provider_type = Provider::from_str(&agent_config.provider)
            .ok_or_else(|| Error::new(&agent_config.provider, "Unknown provider"))?;

        // Get provider config from agent config (reads api_key, endpoint, default_model)
        let provider_config = providers::ProviderConfig::from_agent_config(agent_config)?;

        // Build context and call provider
        let context = Context::prompt(prompt);
        providers::chat(provider_type, provider_config, context).await
    }

    /// Send a prompt with a specific model
    ///
    /// # Arguments
    /// * `agent_name` - The agent name
    /// * `model` - The model to use (overrides agent default)
    /// * `prompt` - The prompt text
    ///
    /// # Example
    /// ```rust
    /// let response = client.prompt_with_model("gemini", "gemini-1.5-pro", "Hello").await?;
    /// println!("Response: {}", response.content);
    /// ```
    pub async fn prompt_with_model(
        &self,
        agent_name: &str,
        model: &str,
        prompt: &str,
    ) -> Result<Response, Error> {
        // Look up agent configuration by agent name
        let agent_config = self
            .app_config
            .agents
            .get(agent_name)
            .ok_or_else(|| Error::new(agent_name, "Agent not found"))?;

        // Get provider type from agent config
        let provider_type = Provider::from_str(&agent_config.provider)
            .ok_or_else(|| Error::new(&agent_config.provider, "Unknown provider"))?;

        // Get provider config from agent config
        let provider_config = providers::ProviderConfig::from_agent_config(agent_config)?;

        // Build context with model override and call provider
        let context = Context::prompt(prompt).with_model(model.to_string());
        providers::chat(provider_type, provider_config, context).await
    }




}

#[async_trait]
impl LlmProvider for Client {
    async fn completion(&self, prompt: &str, agent_name: &str) -> Result<String, String> {
        self.prompt(agent_name, prompt)
            .await
            .map(|r| r.content)
            .map_err(|e| e.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_from_str() {
        assert_eq!(Provider::from_str("openai"), Some(Provider::OpenAI));
        assert_eq!(Provider::from_str("anthropic"), Some(Provider::Anthropic));
        assert_eq!(Provider::from_str("claude"), Some(Provider::Anthropic));
        assert_eq!(Provider::from_str("gemini"), Some(Provider::Gemini));
        assert_eq!(Provider::from_str("groq"), Some(Provider::Groq));
        assert_eq!(Provider::from_str("xai"), Some(Provider::XAI));
        assert_eq!(Provider::from_str("deepai"), Some(Provider::DeepAI));
        assert_eq!(Provider::from_str("deep_ai"), Some(Provider::DeepAI));
        assert_eq!(Provider::from_str("zai"), Some(Provider::Zai));
        assert_eq!(Provider::from_str("unknown"), None);
    }

    #[test]
    fn test_provider_as_str() {
        assert_eq!(Provider::OpenAI.as_str(), "openai");
        assert_eq!(Provider::Anthropic.as_str(), "anthropic");
        assert_eq!(Provider::Gemini.as_str(), "gemini");
        assert_eq!(Provider::Groq.as_str(), "groq");
        assert_eq!(Provider::XAI.as_str(), "xai");
        assert_eq!(Provider::DeepAI.as_str(), "deepai");
        assert_eq!(Provider::Zai.as_str(), "zai");
    }
}
