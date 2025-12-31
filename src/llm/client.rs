//! Simple LLM client for multiple providers with native caching support

use crate::core::config::AppConfig;
use crate::llm::providers;
use crate::llm::{Context, Error, Provider, Response};

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

    /// Send a chat request with full context
    ///
    /// # Arguments
    /// * `agent_name` - The agent name
    /// * `context` - Full context with messages, model, temperature, cache, etc.
    ///
    /// # Example
    /// ```rust
    /// let context = Context::new()
    ///     .add_system_message("You are a helpful assistant.")
    ///     .add_user_message("What is 2+2?")
    ///     .add_assistant_message("2+2 equals 4.")
    ///     .add_user_message("And what about 3+3?");
    /// let response = client.chat("anthropic", context).await?;
    /// println!("Response: {}", response.content);
    /// ```
    pub async fn chat(&self, agent_name: &str, context: Context) -> Result<Response, Error> {
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

        // Call provider with full context
        providers::chat(provider_type, provider_config, context).await
    }

    /// Get provider configuration for an agent
    ///
    /// # Arguments
    /// * `agent_name` - The agent name
    ///
    /// # Returns
    /// The `ProviderConfig` containing api_key, endpoint, and default_model for this agent
    ///
    /// # Example
    /// ```rust
    /// let config = client.get_provider_config("zai")?;
    /// println!("API Key: {}", config.api_key);
    /// println!("Endpoint: {:?}", config.base_url);
    /// println!("Default Model: {}", config.default_model);
    /// ```
    pub fn get_provider_config(
        &self,
        agent_name: &str,
    ) -> Result<providers::ProviderConfig, Error> {
        // Look up agent configuration by agent name
        let agent_config = self
            .app_config
            .agents
            .get(agent_name)
            .ok_or_else(|| Error::new(agent_name, "Agent not found"))?;

        // Get provider config from agent config (reads api_key, endpoint, default_model)
        providers::ProviderConfig::from_agent_config(agent_config)
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
