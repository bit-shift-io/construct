use super::{Agent, AgentContext};
use crate::config::AgentConfig;
use async_trait::async_trait;
use rig::{
    completion::Prompt,
    providers::{anthropic, gemini, openai},
};

pub struct UnifiedAgent {
    pub provider: String,
    pub config: Option<AgentConfig>,
}

use serde::Deserialize;

#[derive(Deserialize)]
struct DeepAIResponse {
    output: String,
}

#[async_trait]
impl Agent for UnifiedAgent {
    async fn execute(&self, context: &AgentContext) -> Result<String, String> {
        let config = self.config.as_ref().ok_or("Agent not configured")?;

        // Model Resolution Logic (keep existing Gemini/Claude logic + others)
        // Model Resolution Logic (keep existing Gemini/Claude logic + others)
        let resolved_model = context.model.clone().or(if !config.model.is_empty() {
            Some(config.model.clone())
        } else {
            None
        });

        let model_name = resolved_model.unwrap_or(match self.provider.as_str() {
            "gemini" => {
                // Dynamic discovery for Gemini to avoid 404s
                // 1. Get API Key first (needed for discovery)
                let api_key = if let Some(key) = &config.api_key {
                    Some(key.clone())
                } else {
                    // Fallback to env usually handled by client, but we need string for discovery
                    std::env::var("GEMINI_API_KEY").ok()
                };

                if let Some(key) = api_key {
                    match crate::agent::discovery::list_gemini_models_web(&key).await {
                        Ok(models) if !models.is_empty() => {
                            // Prefer 2.0-flash if available, else first one
                            if models.contains(&"gemini-2.0-flash".to_string()) {
                                "gemini-2.0-flash".to_string()
                            } else if let Some(flash) = models.iter().find(|m| m.contains("flash"))
                            {
                                flash.clone()
                            } else {
                                models[0].clone()
                            }
                        }
                        _ => "gemini-2.0-flash".to_string(), // Fallback
                    }
                } else {
                    "gemini-2.0-flash".to_string()
                }
            }
            "claude" | "anthropic" => "claude-3-5-sonnet-20241022".to_string(),
            "openai" => "gpt-4o".to_string(),
            "xai" => "grok-beta".to_string(),
            "copilot" => "default".to_string(),
            "deepai" | "deep_ai" => "standard".to_string(),
            _ => "default".to_string(),
        });

        match self.provider.as_str() {
            "gemini" => {
                let client = if let Some(key) = &config.api_key {
                    gemini::Client::new(key)
                } else {
                    gemini::Client::from_env()
                };
                let agent = client.agent(&model_name).build();
                agent.prompt(&context.prompt).await.map_err(|e| {
                    let err_msg = e.to_string();
                    if err_msg.contains("429") || err_msg.to_lowercase().contains("too many requests") {
                        use chrono::Utc;
                        let now = Utc::now();
                        // Google daily quotas reset at midnight Pacific Time.
                        // Pacific Standard Time is UTC-8, so midnight PST is 08:00 UTC.
                        // Pacific Daylight Time is UTC-7, so midnight PDT is 07:00 UTC.
                        // We target 08:00 UTC as a conservative estimate (latest possible reset).
                        let mut next_reset = now.date_naive().and_hms_opt(8, 0, 0).unwrap().and_utc();
                        if now > next_reset {
                            next_reset = next_reset + chrono::Duration::days(1);
                        }
                        let duration = next_reset.signed_duration_since(now);
                        let hours = duration.num_hours();
                        let minutes = duration.num_minutes() % 60;

                        format!("Out of usage for this model: {} (Rate limit exceeded. Resets in approx. {}h {}m at midnight PT)", model_name, hours, minutes)
                    } else if err_msg.contains("404") {
                        format!("{} (Hint: Check if 'Generative Language API' is enabled in Google Cloud Console, and ensure you are using an AI Studio key, not Vertex AI. Model: {})", err_msg, model_name)
                    } else {
                        err_msg
                    }
                })
            }
            "claude" | "anthropic" => {
                let client = if let Some(key) = &config.api_key {
                    anthropic::Client::new(key, "https://api.anthropic.com/v1", None, "2023-06-01")
                } else {
                    anthropic::Client::from_env()
                };
                let agent = client.agent(&model_name).build();
                agent
                    .prompt(&context.prompt)
                    .await
                    .map_err(|e| e.to_string())
            }
            "deepai" | "deep_ai" => {
                // Resolved API Key for DeepAI
                let api_key = if let Some(k) = &config.api_key {
                    k.clone()
                } else {
                    std::env::var("DEEPAI_API_KEY").map_err(|_| "Missing DEEPAI_API_KEY")?
                };

                let client = reqwest::Client::new();
                let resp = client
                    .post("https://api.deepai.org/api/text-generator")
                    .header("api-key", api_key)
                    .form(&[("text", &context.prompt)])
                    .send()
                    .await
                    .map_err(|e| format!("DeepAI Request Failed: {}", e))?;

                if !resp.status().is_success() {
                    return Err(format!("DeepAI API Error: {}", resp.status()));
                }

                let body: DeepAIResponse = resp
                    .json()
                    .await
                    .map_err(|e| format!("DeepAI Parse Error: {}", e))?;
                Ok(body.output)
            }
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
                crate::util::run_command(&cmd, context.working_dir.as_deref()).await
            }
            "openai" => {
                let client = if let Some(key) = config.api_key.as_ref().filter(|k| !k.is_empty()) {
                    openai::Client::new(key)
                } else {
                    openai::Client::from_env()
                };
                let agent = client.agent(&model_name).build();
                agent
                    .prompt(&context.prompt)
                    .await
                    .map_err(|e| e.to_string())
            }
            "xai" => {
                let api_key = if let Some(key) = config.api_key.as_ref().filter(|k| !k.is_empty()) {
                    key.clone()
                } else {
                    std::env::var("XAI_API_KEY").unwrap_or_default()
                };
                let client = openai::Client::from_url(&api_key, "https://api.x.ai/v1");
                let agent = client.agent(&model_name).build();
                agent
                    .prompt(&context.prompt)
                    .await
                    .map_err(|e| e.to_string())
            }
            _ => Err(format!("Unsupported Unified provider: {}", self.provider)),
        }
    }

    fn name(&self) -> &str {
        &self.provider
    }
}
