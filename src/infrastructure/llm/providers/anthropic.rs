//! Anthropic (Claude) provider with native prompt caching
//!
//! Supports Claude models with Anthropic's prompt caching feature
//! which can save up to 90% on long contexts.

use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::ProviderConfig;
use crate::infrastructure::llm::{Context, Error, Message, MessageRole, Response, TokenUsage};

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

/// Anthropic API request format
#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    stop_sequences: Vec<String>,
}

/// Anthropic message format
#[derive(Debug, Serialize)]
struct AnthropicMessage {
    role: String,
    content: Vec<AnthropicContentBlock>,
}

/// Anthropic content block
#[derive(Debug, Serialize)]
struct AnthropicContentBlock {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    content_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_control: Option<CacheControl>,
}

/// Cache control for native prompt caching
#[derive(Debug, Serialize)]
struct CacheControl {
    #[serde(rename = "type")]
    cache_type: String,
}

/// Anthropic API response format
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AnthropicResponse {
    id: String,
    model: String,
    role: String,
    content: Vec<AnthropicResponseContent>,
    stop_reason: String,
    #[serde(rename = "stop_sequence")]
    stop_sequence: Option<String>,
    usage: AnthropicUsage,
}

/// Anthropic response content
#[derive(Debug, Deserialize)]
struct AnthropicResponseContent {
    #[serde(rename = "type")]
    content_type: String,
    text: String,
}

/// Anthropic usage information
#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
    #[serde(rename = "cache_creation_input_tokens")]
    cache_creation_input_tokens: Option<u32>,
    #[serde(rename = "cache_read_input_tokens")]
    cache_read_input_tokens: Option<u32>,
}

/// Anthropic Models API response
#[derive(Debug, Deserialize)]
struct AnthropicModelList {
    data: Vec<AnthropicModelInfo>,
}

#[derive(Debug, Deserialize)]
struct AnthropicModelInfo {
    id: String,
}

/// Execute a chat request using Anthropic's API
pub async fn chat(config: ProviderConfig, context: Context) -> Result<Response, Error> {
    let base_url = config
        .base_url
        .unwrap_or_else(|| "https://api.anthropic.com".to_string());
    let model = context.model.unwrap_or_else(|| {
        if config.default_model.is_empty() {
            "claude-3-5-sonnet-20241022".to_string()
        } else {
            config.default_model.clone()
        }
    });

    let url = format!("{}/v1/messages", base_url);
    let api_version = "2023-06-01";

    // Extract system messages and convert user/assistant messages
    let (system_message, chat_messages): (Option<String>, Vec<&Message>) = {
        let mut system = None;
        let mut messages = Vec::new();

        for msg in &context.messages {
            if msg.role == MessageRole::System {
                system = Some(msg.content.clone());
            } else {
                messages.push(msg);
            }
        }

        (system, messages)
    };

    // Build messages with caching support
    let enable_caching = context.cache.is_some();
    let mut anthropic_messages = Vec::new();

    for msg in chat_messages {
        let role = match msg.role {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::System => "user", // Convert system to user (shouldn't happen here)
        };

        let mut content_blocks = vec![AnthropicContentBlock {
            content_type: Some("text".to_string()),
            text: Some(msg.content.clone()),
            cache_control: None,
        }];

        // Add cache control to first user message or system message if caching enabled
        if enable_caching && (msg.role == MessageRole::User || msg.role == MessageRole::System) {
            content_blocks[0].cache_control = Some(CacheControl {
                cache_type: "ephemeral".to_string(),
            });
        }

        anthropic_messages.push(AnthropicMessage {
            role: role.to_string(),
            content: content_blocks,
        });
    }

    // Build request
    let max_tokens = context.max_tokens.unwrap_or(4096);
    let request = AnthropicRequest {
        model: model.clone(),
        max_tokens,
        messages: anthropic_messages,
        system: system_message,
        temperature: context.temperature,
        stop_sequences: vec![],
    };

    // Make HTTP request with retry logic
    let mut last_error = Error::new("anthropic", "Unknown error");
    
    for attempt in 0..3 {
        // Clone request builder for each attempt since send consumes it
        let mut request_builder = http_client()
            .post(&url)
            .header("x-api-key", config.api_key.clone())
            .header("anthropic-version", api_version)
            .header("Content-Type", "application/json")
            .json(&request);

        if let Some(timeout_secs) = config.timeout {
            request_builder = request_builder.timeout(std::time::Duration::from_secs(timeout_secs));
        }

        match request_builder.send().await {
            Ok(resp) => {
                 let status = resp.status();
                 if status.is_success() || status.is_client_error() {
                     // Don't retry client errors (4xx) unless it's a timeout/rate limit?
                     // Verify status inside the response handling below.
                     // Actually, we should process it. If it fails inside, we might want to retry 5xx.
                     // For now, let's return the response and let the status check handle it.
                     // But wait, the original code checks status AFTER this block.
                     // We need to move the response processing inside or return it.
                     // Let's break the loop with the response.
                     let response = resp;
                     
                     // If server error (5xx), maybe retry?
                     if status.is_server_error() {
                         let error_text = response.text().await.unwrap_or_default();
                         last_error = Error::new("anthropic", format!("HTTP {}: {}", status, error_text));
                         tracing::warn!("Anthropic API error (attempt {}): {}", attempt + 1, last_error.message);
                         tokio::time::sleep(std::time::Duration::from_millis(500 * (attempt + 1) as u64)).await;
                         continue;
                     }
                     
                     // Success or Client Error -> Process it
                     // We need to return the response to the outer scope to preserve original flow structure 
                     // OR handle it here.
                     // Refactoring strictly to wrapping the *send* might be cleaner if we just return the Result<Response>.
                     
                     // Let's refactor slightly to return the response from the block.
                     // But we can't easily jump out of the loop with a value to a variable defined outside without specific structure.
                     // Let's use a labeled block or just match properly.
                
                     // Check status here to decide on retry
                     // Original code checks !status.is_success() later.
                     // We only want to retry on NETWORK errors or 5xx.
                     // 4xx should NOT be retried (except 429).
                     
                     return process_response(response).await;
                 }
            }
            Err(e) => {
                last_error = Error::new("anthropic", format!("HTTP request failed: {}", e));
                tracing::warn!("Anthropic network error (attempt {}): {}", attempt + 1, e);
            }
        }
        
        // Exponential backoff
        tokio::time::sleep(std::time::Duration::from_millis(500 * (attempt + 1) as u64)).await;
    }
    
    return Err(last_error);
    
} // End of chat function wrapper? No, this replaces the request_builder block.

// Helper to process response to avoid deep nesting
async fn process_response(response: reqwest::Response) -> Result<Response, Error> {
    let status = response.status();

    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unable to read error response".to_string());

        // Try to parse error message from response
        if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&error_text)
            && let Some(error) = error_json.get("error")
            && let Some(error_type) = error.get("type")
            && let Some(error_msg) = error.get("message")
        {
            return Err(Error::new(
                "anthropic",
                format!("{}: {}", error_type, error_msg),
            ));
        }

        return Err(Error::new(
            "anthropic",
            format!("HTTP {}: {}", status, error_text),
        ));
    }

    // Parse response
    let anthropic_response: AnthropicResponse = response
        .json()
        .await
        .map_err(|e| Error::new("anthropic", format!("Failed to parse response: {}", e)))?;

    // Extract text from content blocks
    let content: String = anthropic_response
        .content
        .into_iter()
        .filter_map(|block| {
            if block.content_type == "text" {
                Some(block.text)
            } else {
                None
            }
        })
        .collect();

    // Calculate cached tokens if available
    let cached_tokens = anthropic_response
        .usage
        .cache_read_input_tokens
        .or(anthropic_response.usage.cache_creation_input_tokens);

    Ok(Response {
        content,
        model: anthropic_response.model,
        usage: TokenUsage {
            prompt_tokens: anthropic_response.usage.input_tokens,
            completion_tokens: anthropic_response.usage.output_tokens,
            total_tokens: anthropic_response.usage.input_tokens
                + anthropic_response.usage.output_tokens,
            cached_tokens,
        },
        cached: cached_tokens.is_some(),
    })
}

/// List available models from Anthropic API
pub async fn list_models(config: ProviderConfig) -> Result<Vec<String>, Error> {
    let base_url = config
        .base_url
        .unwrap_or_else(|| "https://api.anthropic.com".to_string());

    let url = format!("{}/v1/models", base_url);
    let api_version = "2023-06-01";

    let response = http_client()
        .get(&url)
        .header("x-api-key", config.api_key)
        .header("anthropic-version", api_version)
        .send()
        .await
        .map_err(|e| Error::new("anthropic", format!("HTTP request failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(Error::new(
            "anthropic",
            format!("HTTP {}", response.status()),
        ));
    }

    let model_list: AnthropicModelList = response
        .json()
        .await
        .map_err(|e| Error::new("anthropic", format!("Failed to parse response: {}", e)))?;

    Ok(model_list.data.into_iter().map(|m| m.id).collect())
}
