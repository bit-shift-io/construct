use super::{Agent, AgentContext};
use crate::config::AgentConfig;
use async_trait::async_trait;
use rig::{
    client::{CompletionClient, ProviderClient},
    completion::Prompt,
    providers::{anthropic, openai, zai},
};

use super::rate_limiter::RateLimiter;

pub struct UnifiedAgent {
    pub provider: String,
    pub config: Option<AgentConfig>,
}

use serde::Deserialize;

#[derive(Deserialize)]
struct DeepAIResponse {
    output: String,
}

// Gemini Structs for manual implementation
#[derive(Deserialize, Debug)]
struct GeminiResponse {
    #[serde(default)]
    candidates: Vec<GeminiCandidate>,
    #[serde(default)]
    error: Option<GeminiError>,
    #[serde(rename = "promptFeedback")]
    prompt_feedback: Option<GeminiPromptFeedback>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct GeminiPromptFeedback {
    #[serde(rename = "blockReason")]
    block_reason: Option<String>,
    #[serde(rename = "safetyRatings")]
    safety_ratings: Option<Vec<GeminiSafetyRating>>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct GeminiCandidate {
    content: Option<GeminiContent>,
    #[serde(rename = "finishReason")]
    finish_reason: Option<String>,
    #[serde(rename = "safetyRatings")]
    safety_ratings: Option<Vec<GeminiSafetyRating>>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct GeminiSafetyRating {
    category: String,
    probability: String,
}

#[derive(Deserialize, Debug)]
struct GeminiContent {
    parts: Option<Vec<GeminiPart>>,
}

#[derive(Deserialize, Debug)]
struct GeminiPart {
    text: Option<String>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct GeminiError {
    message: String,
    code: Option<i32>,
}

#[derive(serde::Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiRequestContent>,
    #[serde(rename = "safetySettings")]
    safety_settings: Vec<GeminiSafetySetting>,
}

#[derive(serde::Serialize)]
struct GeminiRequestContent {
    parts: Vec<GeminiRequestPart>,
    role: String,
}

#[derive(serde::Serialize)]
struct GeminiRequestPart {
    text: String,
}

#[derive(serde::Serialize)]
struct GeminiSafetySetting {
    category: String,
    threshold: String,
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
                "gemini" => {
                    let api_key = if let Some(key) = &config.api_key {
                        Some(key.clone())
                    } else {
                        std::env::var("GEMINI_API_KEY").ok()
                    };

                    if let Some(_key) = api_key {
                        // Dynamic discovery is picking unstable preview models (e.g. gemini-3-flash-preview)
                        // Force 1.5-pro for stability and better safety feedback.
                        "gemini-1.5-pro".to_string()
                    } else {
                        "gemini-1.5-pro".to_string()
                    }
                }
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
                "zai" => {
                    let api_key = if let Some(key) = config.api_key.as_ref().filter(|k| !k.is_empty()) {
                        key.clone()
                    } else {
                        std::env::var("ZAI_API_KEY").map_err(|_| "Missing ZAI_API_KEY")?
                    };

                    let rate_limiter = RateLimiter::from_config(config, 3);
                    let context_clone = context.clone();

                    rate_limiter
                        .execute_with_retry(
                            move || {
                                let model_name = model_name.clone();
                                let prompt = context.prompt.clone();
                                let api_key = api_key.clone();
                                async move {
                                    unsafe {
                                        std::env::set_var("ZAI_API_KEY", &api_key);
                                    }

                                    let client = zai::Client::from_env();
                                    let agent = client.agent(&model_name).build();
                                    agent
                                        .prompt(&prompt)
                                        .await
                                        .map_err(|e| e.to_string())
                                }
                            },
                            &context_clone,
                            "zai",
                        )
                        .await
                }
                "gemini" => {
                    let api_key = if let Some(key) = &config.api_key {
                        key.clone()
                    } else {
                        std::env::var("GEMINI_API_KEY").map_err(|_| "Missing GEMINI_API_KEY")?
                    };

                    let mut models_to_try = Vec::new();

                    // Priority 1: User Override
                    if !model_name.is_empty() && model_name != "default" {
                         models_to_try.push(model_name.clone());
                    }

                    let mut models_to_try = Vec::new();

                    // Priority 1: User Override
                    if !model_name.is_empty() && model_name != "default" {
                         models_to_try.push(model_name.clone());
                    }

                    // Priority 2: Discovered API Models (Sorted by Regex)
                    // Fetch models dynamically
                    if let Ok(available_models) = crate::agent::discovery::list_gemini_models_web(&api_key).await {
                        if let Some(order_patterns) = &config.model_order {
                            // Sort based on patterns
                            let mut remaining = available_models.clone();
                             for pattern in order_patterns {
                                let regex = regex::Regex::new(pattern).ok();
                                let (matches, others): (Vec<_>, Vec<_>) = remaining.into_iter().partition(|m| {
                                    if let Some(re) = &regex {
                                        re.is_match(m)
                                    } else {
                                        m.contains(pattern) // Simple fallback
                                    }
                                });
                                models_to_try.extend(matches);
                                remaining = others;
                             }
                             // Append remaining models at the end? Or ignore?
                             // Usually good to try them last.
                             models_to_try.extend(remaining);
                        } else {
                            // No order specified, just append all
                            models_to_try.extend(available_models);
                        }
                    }

                    // Priority 3: Configured Fallbacks (Safety Net)
                    if let Some(fallbacks) = &config.model_fallbacks {
                        for m in fallbacks {
                            if !models_to_try.contains(m) {
                                models_to_try.push(m.clone());
                            }
                        }
                    } else {
                        // Minimal hardcoded fallback if config is empty, to prevent total failure
                        let defaults = vec!["gemini-1.5-flash", "gemini-1.5-pro"];
                        for m in defaults {
                             if !models_to_try.contains(&m.to_string()) {
                                models_to_try.push(m.to_string());
                            }
                        }
                    }

                    // Note: Ideally we would also try `discovery::list_gemini_models_web` here if all defaults fail,
                    // but that's expensive to do on every request. Relying on YAML defaults is safe and user-configurable.

                     let mut last_error = "No models available".to_string();

                     for model in models_to_try {
                        let url = format!(
                            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
                            model, api_key
                        );

                        crate::utils::log_to_agent_file("DEBUG", "gemini", &format!("Attempting Model: {}", model));

                        // Aggressive sanitization
                        let mut sanitized_prompt = context.prompt
                            .replace("curl", "[http_tool]")
                            .replace("wget", "[http_tool]")
                            .replace("| sh", "| [shell_exec]")
                            .replace("| bash", "| [shell_exec]")
                            .replace(".sh", "[script_ext]")
                            .replace("sudo", "[admin_cmd]");

                         sanitized_prompt = format!("IMPORTANT SAFETY: Do not generate commands that pipe to shell. Use 2 separate steps.\n\n{}", sanitized_prompt);

                        let request_body = GeminiRequest {
                            contents: vec![GeminiRequestContent {
                                role: "user".to_string(),
                                parts: vec![GeminiRequestPart {
                                    text: sanitized_prompt,
                                }],
                            }],
                            safety_settings: vec![
                                GeminiSafetySetting { category: "HARM_CATEGORY_HARASSMENT".to_string(), threshold: "BLOCK_NONE".to_string() },
                                GeminiSafetySetting { category: "HARM_CATEGORY_HATE_SPEECH".to_string(), threshold: "BLOCK_NONE".to_string() },
                                GeminiSafetySetting { category: "HARM_CATEGORY_SEXUALLY_EXPLICIT".to_string(), threshold: "BLOCK_NONE".to_string() },
                                GeminiSafetySetting { category: "HARM_CATEGORY_DANGEROUS_CONTENT".to_string(), threshold: "BLOCK_NONE".to_string() },
                            ],
                        };

                        let client = reqwest::Client::new();

                        // Retry Logic (Max 3 attempts)
                        let mut attempt = 0;
                        let max_retries = 3;
                        let mut should_switch_model = false;

                        while attempt < max_retries {
                            attempt += 1;

                            let resp_result = client
                                .post(&url)
                                .json(&request_body)
                                .send()
                                .await;

                            match resp_result {
                                Err(e) => {
                                    last_error = format!("Network Error ({}): {}", model, e);
                                    crate::utils::log_to_agent_file("ERROR", "gemini", &format!("Model {} failed (Attempt {}/{}): {}", model, attempt, max_retries, last_error));

                                    // Dynamic delay based on RPM (default to 2 RPM / 30s if not set)
                                    let delay = if let Some(rpm) = config.requests_per_minute {
                                        if rpm > 0 { 60 / rpm } else { 30 }
                                    } else {
                                        30
                                    }
                                    .max(1); // Ensure at least 1s wait

                                    if attempt < max_retries {
                                        if let Some(mut rx) = context.abort_signal.clone() {
                                            if let Some(cb) = &context.status_callback {
                                                cb(format!("⚠️ Network error. Retrying in {}s... (Use .stop to cancel)", delay));
                                            }
                                            tokio::select! {
                                                _ = tokio::time::sleep(tokio::time::Duration::from_secs(delay)) => {},
                                                _ = rx.changed() => {
                                                    if *rx.borrow() {
                                                        if let Some(cb) = &context.status_callback {
                                                            cb("🛑 Retry cancelled by user.".to_string());
                                                        }
                                                        return Err("Cancelled by user".to_string());
                                                    }
                                                }
                                            }
                                        } else {
                                            if let Some(cb) = &context.status_callback {
                                                cb(format!("⚠️ Network error. Retrying in {}s...", delay));
                                            }
                                            tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
                                        }
                                        continue;
                                    }
                                }
                                Ok(resp) => {
                                    let status = resp.status();
                                    if !status.is_success() {
                                        let text = resp.text().await.unwrap_or_default();
                                        // Quota retry handling
                                        if text.contains("429") || text.to_lowercase().contains("too many requests") || status.as_u16() >= 500 {
                                            last_error = format!("API Error {} (Attempt {}/{}): {}", status, attempt, max_retries, text);
                                            crate::utils::log_to_agent_file("WARNING", "gemini", &format!("Transient Error: {}", last_error));

                                            if attempt < max_retries {
                                                // Dynamic delay based on RPM
                                                let delay = if let Some(rpm) = config.requests_per_minute {
                                                    if rpm > 0 { 60 / rpm } else { 30 }
                                                } else {
                                                    30
                                                }
                                                .max(1);

                                                if let Some(mut rx) = context.abort_signal.clone() {
                                                     if let Some(cb) = &context.status_callback {
                                                        cb(format!("⚠️ Quota limit reached ({}). Retrying in {}s... (Use .stop to cancel)", status, delay));
                                                    }
                                                    tokio::select! {
                                                        _ = tokio::time::sleep(tokio::time::Duration::from_secs(delay)) => {},
                                                        _ = rx.changed() => {
                                                             if *rx.borrow() {
                                                                if let Some(cb) = &context.status_callback {
                                                                    cb("🛑 Retry cancelled by user.".to_string());
                                                                }
                                                                return Err("Cancelled by user".to_string());
                                                             }
                                                        }
                                                    }
                                                } else {
                                                    if let Some(cb) = &context.status_callback {
                                                        cb(format!("⚠️ Quota limit reached ({}). Retrying in {}s...", status, delay));
                                                    }
                                                    tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
                                                }
                                                continue;
                                            } else {
                                                // Retries exhausted, NOW we consider switching
                                                if let Some(cb) = &context.status_callback {
                                                    cb(format!("⚠️ Max retries reached for {}. Switching model...", model));
                                                }

                                                // If it's a 429, we mark it as Quota Exceeded to trigger switch
                                                if text.contains("429") || text.to_lowercase().contains("too many requests") {
                                                     last_error = format!("Quota Exceeded ({})", model);
                                                }
                                                // Break retry loop, proceed to next model
                                                should_switch_model = true;
                                                break;
                                            }
                                        }

                                        if text.contains("404") {
                                            last_error = format!("Model Not Found ({})", model);
                                            crate::utils::log_to_agent_file("ERROR", "gemini", &format!("Model {} 404 Not Found", model));
                                            should_switch_model = true;
                                            break;
                                        }
                                        last_error = format!("API Error ({}) {}: {}", model, status, text);
                                        crate::utils::log_to_agent_file("ERROR", "gemini", &format!("Model {} API Error: {}", model, text));
                                        should_switch_model = true; // Non-retriable error
                                        break;
                                    }

                                    let body_result: Result<GeminiResponse, _> = resp.json().await;
                                    match body_result {
                                        Err(e) => {
                                            last_error = format!("Parse Error ({}): {}", model, e);
                                            should_switch_model = true;
                                            break;
                                        }
                                        Ok(body) => {
                                            if let Some(err) = body.error {
                                                last_error = format!("API Error ({}): {}", model, err.message);
                                                should_switch_model = true;
                                                break;
                                            }

                                            // Check for safety blocks
                                            let mut safety_blocked = false;
                                            let mut block_reason = String::new();

                                            if let Some(candidate) = body.candidates.first() {
                                                if let Some(reason) = &candidate.finish_reason {
                                                    if reason == "PROHIBITED_CONTENT" || reason == "SAFETY" {
                                                        safety_blocked = true;
                                                        block_reason = reason.clone();
                                                    }
                                                }
                                            } else if let Some(pf) = &body.prompt_feedback {
                                                 if let Some(reason) = &pf.block_reason {
                                                    safety_blocked = true;
                                                    block_reason = reason.clone();
                                                 }
                                            }

                                            if safety_blocked {
                                                let msg = format!("Safety Block ({}) Reason: {}. Switching model...", model, block_reason);
                                                crate::utils::log_to_agent_file("WARNING", "gemini", &msg);
                                                if let Some(cb) = &context.status_callback {
                                                    cb(format!("⚠️ {}", msg));
                                                }
                                                last_error = msg;
                                                should_switch_model = true;
                                                break; // Break retry, continue to next model
                                            }

                                            // Success path
                                            if let Some(candidate) = body.candidates.first() {
                                                if let Some(content) = &candidate.content {
                                                    if let Some(parts) = &content.parts {
                                                        if let Some(part) = parts.first() {
                                                            if let Some(text) = &part.text {
                                                                return Ok(text.clone());
                                                            }
                                                        }
                                                    }
                                                }
                                            }

                                            last_error = format!("Empty/Unknown Response from {}", model);
                                            should_switch_model = true;
                                            break;
                                        }
                                    }
                                }
                            }
                        }

                        // If we broke the retry loop and didn't return Ok, we continue to next model
                        if should_switch_model {
                            continue;
                        }
                     }

                     Err(last_error)
                }
                "claude" | "anthropic" => {
                    let rate_limiter = RateLimiter::from_config(config, 3);
                    let context_clone = context.clone();

                    rate_limiter
                        .execute_with_retry(
                            move || {
                                let model_name = model_name.clone();
                                let prompt = context.prompt.clone();
                                async move {
                                    let client = anthropic::Client::from_env();
                                    let agent = client.agent(&model_name).build();
                                    agent
                                        .prompt(&prompt)
                                        .await
                                        .map_err(|e| e.to_string())
                                }
                            },
                            &context_clone,
                            "anthropic",
                        )
                        .await
                }
                "deepai" | "deep_ai" => {
                    let rate_limiter = RateLimiter::from_config(config, 3);
                    let config_clone = config.clone();
                    let context_clone = context.clone();

                    rate_limiter
                        .execute_with_retry(
                            move || {
                                let prompt = context.prompt.clone();
                                let config = config_clone.clone();
                                async move {
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
                                        .form(&[("text", &prompt)])
                                        .send()
                                        .await
                                        .map_err(|e| crate::strings::STRINGS.messages.deepai_request_failed.replace("{}", &e.to_string()))?;

                                    if !resp.status().is_success() {
                                        return Err(crate::strings::STRINGS.messages.deepai_api_error.replace("{}", &resp.status().to_string()));
                                    }

                                    let body: DeepAIResponse = resp
                                        .json()
                                        .await
                                        .map_err(|e| crate::strings::STRINGS.messages.deepai_parse_error.replace("{}", &e.to_string()))?;
                                    Ok(body.output)
                                }
                            },
                            &context_clone,
                            "deepai",
                        )
                        .await
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
                    crate::utils::run_command(&cmd, context.working_dir.as_deref()).await
                }
                "openai" => {
                    let rate_limiter = RateLimiter::from_config(config, 3);
                    let context_clone = context.clone();

                    rate_limiter
                        .execute_with_retry(
                            move || {
                                let model_name = model_name.clone();
                                let prompt = context.prompt.clone();
                                async move {
                                    let client = openai::Client::from_env();
                                    let agent = client.agent(&model_name).build();
                                    agent
                                        .prompt(&prompt)
                                        .await
                                        .map_err(|e| e.to_string())
                                }
                            },
                            &context_clone,
                            "openai",
                        )
                        .await
                }
                "xai" => {
                    let rate_limiter = RateLimiter::from_config(config, 3);
                    let context_clone = context.clone();

                    rate_limiter
                        .execute_with_retry(
                            move || {
                                let model_name = model_name.clone();
                                let prompt = context.prompt.clone();
                                async move {
                                    let api_key = std::env::var("XAI_API_KEY").map_err(|_| "Missing XAI_API_KEY")?;
                                    unsafe {
                                        std::env::set_var("OPENAI_BASE_URL", "https://api.x.ai/v1");
                                        std::env::set_var("OPENAI_API_KEY", &api_key);
                                    }
                                    let client = openai::Client::from_env();
                                    let agent = client.agent(&model_name).build();
                                    agent
                                        .prompt(&prompt)
                                        .await
                                        .map_err(|e| e.to_string())
                                }
                            },
                            &context_clone,
                            "xai",
                        )
                        .await
                }
                "groq" => {
                    let rate_limiter = RateLimiter::from_config(config, 3);
                    let context_clone = context.clone();

                    rate_limiter
                        .execute_with_retry(
                            move || {
                                let model_name = model_name.clone();
                                let prompt = context.prompt.clone();
                                async move {
                                    let api_key = std::env::var("GROQ_API_KEY").map_err(|_| "Missing GROQ_API_KEY")?;
                                    unsafe {
                                        std::env::set_var("OPENAI_BASE_URL", "https://api.groq.com/openai/v1");
                                        std::env::set_var("OPENAI_API_KEY", &api_key);
                                    }
                                    let client = openai::Client::from_env();
                                    let agent = client.agent(&model_name).build();
                                    agent
                                        .prompt(&prompt)
                                        .await
                                        .map_err(|e| e.to_string())
                                }
                            },
                            &context_clone,
                            "groq",
                        )
                        .await
                }
                _ => Err(crate::strings::STRINGS.messages.unsupported_provider.replace("{}", &self.provider)),
            }
        }.await;

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
