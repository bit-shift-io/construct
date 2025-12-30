use super::{Agent, AgentContext};
use crate::config::AgentConfig;
use async_trait::async_trait;

pub struct UnifiedAgent {
    pub provider: String,
    pub config: Option<AgentConfig>,
}

#[async_trait]
impl Agent for UnifiedAgent {
    async fn execute(&self, context: &AgentContext) -> Result<String, String> {
        // Use shared logging utility
        crate::utils::log_to_agent_file(
            "AGENT_START",
            &self.provider,
            &format!("Starting agent execution with model: {:?}", context.model),
        );
        crate::utils::log_to_agent_file("PROMPT", &self.provider, &context.prompt);

        let result = async {
            let config = self.config.as_ref().ok_or("Agent not configured")?;

            // Model Resolution Logic
            let resolved_model = context.model.clone().or(if !config.model.is_empty() {
                Some(config.model.clone())
            } else {
                None
            });

            let model_name = resolved_model.unwrap_or(match self.provider.as_str() {
                "gemini" => "gemini-1.5-pro".to_string(), // Force pro for stability
                "claude" | "anthropic" => "claude-3-5-sonnet-20241022".to_string(),
                "openai" => "gpt-4o".to_string(),
                "xai" => "grok-beta".to_string(),
                "groq" => "llama-3.3-70b-versatile".to_string(),
                "copilot" => "default".to_string(),
                "deepai" | "deep_ai" => "standard".to_string(),
                "zai" => "glm-4.7".to_string(),
                _ => "default".to_string(),
            });

            match self.provider.as_str() {
                "copilot" | "github_copilot" => {
                    // Wrapper for CLI until Rig supports it natively
                    let binary = config.command.as_deref().unwrap_or("github-copilot-cli");
                    let model_flag = if !model_name.is_empty() && model_name != "default" {
                        format!(" --model {}", model_name)
                    } else {
                        String::new()
                    };
                    // Need run_command access
                    // Refactor to use crate::util::run_command
                    let escaped_prompt = context.prompt.replace("\"", "\\\"");
                    let cmd = format!("{}{} \"{}\"", binary, model_flag, escaped_prompt);
                    crate::utils::run_command(&cmd, context.working_dir.as_deref()).await
                }
                _ => {
                    crate::agent::providers::execute_provider(
                        &self.provider,
                        config,
                        context,
                        &model_name,
                    )
                    .await
                }
            }
        }
        .await;

        match &result {
            Ok(content) => crate::utils::log_to_agent_file("RESPONSE", &self.provider, content),
            Err(e) => crate::utils::log_to_agent_file("ERROR", &self.provider, e),
        }

        result
    }

    fn name(&self) -> &str {
        &self.provider
    }
}
