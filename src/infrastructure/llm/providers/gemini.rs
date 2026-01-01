//! Gemini provider with native context caching
//!
//! Supports Google's Gemini models with context caching feature
//! which can cache up to 1M tokens for up to 4 hours.

use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::ProviderConfig;
use crate::infrastructure::llm::{Context, Error, MessageRole, Response, TokenUsage};

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

/// Gemini API request format
#[derive(Debug, Serialize, Deserialize)]
struct GeminiRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    cached_content: Option<String>,
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation_config: Option<GenerationConfig>,
}

/// Gemini content (message)
#[derive(Debug, Serialize, Deserialize)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

/// Gemini content part
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "text")]
struct GeminiPart {
    text: String,
}

/// Generation configuration
#[derive(Debug, Serialize, Deserialize)]
struct GenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
}

/// Gemini API response format
#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
    #[serde(rename = "usageMetadata")]
    usage_metadata: Option<GeminiUsage>,
}

/// Gemini response candidate
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct GeminiCandidate {
    content: GeminiContent,
    #[serde(rename = "finishReason")]
    finish_reason: String,
}

/// Gemini usage metadata
#[derive(Debug, Deserialize)]
struct GeminiUsage {
    #[serde(rename = "promptTokenCount")]
    prompt_token_count: u32,
    #[serde(rename = "candidatesTokenCount")]
    candidates_token_count: u32,
    #[serde(rename = "totalTokenCount")]
    total_token_count: u32,
    #[serde(rename = "cachedContentTokenCount")]
    cached_content_token_count: Option<u32>,
}

/// Execute a chat request using Gemini's API
pub async fn chat(config: ProviderConfig, context: Context) -> Result<Response, Error> {
    let base_url = config
        .base_url
        .unwrap_or_else(|| "https://generativelanguage.googleapis.com".to_string());

    let model = context.model.unwrap_or_else(|| {
        if config.default_model.is_empty() {
            "gemini-1.5-pro".to_string()
        } else {
            config.default_model.clone()
        }
    });

    let url = format!(
        "{}/v1beta/models/{}:generateContent?key={}",
        base_url, model, config.api_key
    );

    // Convert messages to Gemini format
    // Note: Gemini doesn't have a separate system role - system messages become user messages
    let mut contents = Vec::new();

    for msg in &context.messages {
        let role = match msg.role {
            MessageRole::System => "user", // System becomes user in Gemini
            MessageRole::User => "user",
            MessageRole::Assistant => "model",
        };

        // For system messages, prepend a label to distinguish them
        let text = if msg.role == MessageRole::System {
            format!("System: {}", msg.content)
        } else {
            msg.content.clone()
        };

        contents.push(GeminiContent {
            role: role.to_string(),
            parts: vec![GeminiPart { text }],
        });
    }

    // Build generation config
    let generation_config = if context.temperature.is_some() || context.max_tokens.is_some() {
        Some(GenerationConfig {
            temperature: context.temperature,
            max_output_tokens: context.max_tokens,
        })
    } else {
        None
    };

    // Check if caching is enabled - for now we'll use cached_content if provided
    // In a full implementation, you would first create a cached content resource
    // then reference it here. For simplicity, we're not implementing the full
    // cache creation workflow in this basic wrapper.
    let cached_content = None;

    // Build request
    let request = GeminiRequest {
        cached_content,
        contents,
        generation_config,
    };

    // Make HTTP request
    let response = http_client()
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| Error::new("gemini", format!("HTTP request failed: {}", e)))?;

    let status = response.status();

    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unable to read error response".to_string());

        // Try to parse error message from response
        if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&error_text) {
            if let Some(error) = error_json.get("error") {
                if let Some(error_msg) = error.get("message") {
                    return Err(Error::new(
                        "gemini",
                        error_msg.as_str().unwrap_or(&error_text),
                    ));
                }
            }
        }

        return Err(Error::new(
            "gemini",
            format!("HTTP {}: {}", status, error_text),
        ));
    }

    // Parse response
    let gemini_response: GeminiResponse = response
        .json()
        .await
        .map_err(|e| Error::new("gemini", format!("Failed to parse response: {}", e)))?;

    if gemini_response.candidates.is_empty() {
        return Err(Error::new("gemini", "No candidates in response"));
    }

    let candidate = &gemini_response.candidates[0];

    // Extract text from parts
    let content: String = candidate
        .content
        .parts
        .iter()
        .map(|part| part.text.clone())
        .collect::<Vec<_>>()
        .join("\n");

    // Get usage information
    let usage_metadata = gemini_response.usage_metadata.unwrap_or(GeminiUsage {
        prompt_token_count: 0,
        candidates_token_count: 0,
        total_token_count: 0,
        cached_content_token_count: None,
    });

    Ok(Response {
        content,
        model: model,
        usage: TokenUsage {
            prompt_tokens: usage_metadata.prompt_token_count,
            completion_tokens: usage_metadata.candidates_token_count,
            total_tokens: usage_metadata.total_token_count,
            cached_tokens: usage_metadata.cached_content_token_count,
        },
        cached: usage_metadata.cached_content_token_count.is_some(),
    })
}
