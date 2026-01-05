//! OpenAI-compatible API provider
//!
//! Supports OpenAI, Groq, XAI, DeepAI, Zai and other OpenAI-compatible APIs

use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::ProviderConfig;
use crate::infrastructure::llm::{Context, Error, Response, TokenUsage};

/// HTTP client reused across requests
fn http_client() -> &'static Client {
    use std::sync::OnceLock;
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .expect("Failed to create HTTP client")
    })
}

/// OpenAI API request format
#[derive(Debug, Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Debug, Serialize)]
struct OpenAIMessage {
    role: String,
    content: String,
}

/// OpenAI API response format
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAIResponse {
    id: String,
    model: String,
    choices: Vec<OpenAIChoice>,
    usage: OpenAIUsage,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAIChoice {
    message: OpenAIChoiceMessage,
    finish_reason: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAIChoiceMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

/// Execute a chat request using OpenAI-compatible API
pub async fn chat(config: ProviderConfig, context: Context) -> Result<Response, Error> {
    let base_url = config
        .base_url
        .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
    let model = context.model.unwrap_or_else(|| {
        if config.default_model.is_empty() {
            "gpt-4o".to_string()
        } else {
            config.default_model.clone()
        }
    });

    let url = format!("{}/chat/completions", base_url);

    // Build request
    let request = OpenAIRequest {
        model: model.clone(),
        messages: context
            .messages
            .into_iter()
            .map(|msg| OpenAIMessage {
                role: msg.role.as_str().to_string(),
                content: msg.content,
            })
            .collect(),
        temperature: context.temperature,
        max_tokens: context.max_tokens,
    };

    // Make HTTP request
    let mut request_builder = http_client()
        .post(&url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Content-Type", "application/json")
        .json(&request);

    if let Some(timeout_secs) = config.timeout {
        request_builder = request_builder.timeout(std::time::Duration::from_secs(timeout_secs));
    }

    let response = request_builder
        .send()
        .await
        .map_err(|e| Error::new("openai", format!("HTTP request failed: {}", e)))?;

    let status = response.status();

    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unable to read error response".to_string());

        // Try to parse error message from response
        if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&error_text) {
            if let Some(error_msg) = error_json
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
            {
                return Err(Error::new("openai", error_msg));
            }
        }

        return Err(Error::new(
            "openai",
            format!("HTTP {}: {}", status, error_text),
        ));
    }

    // Parse response
    let openai_response: OpenAIResponse = response
        .json()
        .await
        .map_err(|e| Error::new("openai", format!("Failed to parse response: {}", e)))?;

    if openai_response.choices.is_empty() {
        return Err(Error::new("openai", "No choices in response"));
    }

    let choice = &openai_response.choices[0];

    Ok(Response {
        content: choice.message.content.clone(),
        model: openai_response.model,
        usage: TokenUsage {
            prompt_tokens: openai_response.usage.prompt_tokens,
            completion_tokens: openai_response.usage.completion_tokens,
            total_tokens: openai_response.usage.total_tokens,
            cached_tokens: None, // OpenAI doesn't report cached tokens in standard API
        },
        cached: false,
    })
}
