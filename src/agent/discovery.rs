use crate::config::AppConfig;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct GeminiModel {
    name: String,
    #[serde(rename = "displayName")]
    #[allow(dead_code)]
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiListResponse {
    models: Option<Vec<GeminiModel>>,
}

#[derive(Debug, Deserialize)]
struct AnthropicModel {
    id: String,
    // Other fields omitted
}

#[derive(Debug, Deserialize)]
struct AnthropicListResponse {
    data: Vec<AnthropicModel>,
}

pub async fn list_gemini_models(config: &AppConfig) -> Result<Vec<String>, String> {
    // Find any agent with protocol "gemini"
    let agent_config = config.agents.values()
        .find(|c| c.protocol == "gemini")
        .ok_or_else(|| "No gemini agent configured".to_string())?;

    let api_key = if let Some(k) = &agent_config.api_key {
        k.clone()
    } else if let Some(env_var) = &agent_config.api_key_env {
        std::env::var(env_var).map_err(|_| crate::prompts::STRINGS.messages.missing_env_var.replace("{}", env_var))?
    } else {
        std::env::var("GEMINI_API_KEY").map_err(|_| "Missing GEMINI_API_KEY")?
    };

    list_gemini_models_web(&api_key).await
}

pub async fn list_gemini_models_web(api_key: &str) -> Result<Vec<String>, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!("https://generativelanguage.googleapis.com/v1beta/models?key={}", api_key);
    
    let resp = client.get(&url)
        .send()
        .await
        .map_err(|e| crate::prompts::STRINGS.messages.gemini_fetch_failed.replace("{}", &e.to_string()))?;

    if !resp.status().is_success() {
        return Err(crate::prompts::STRINGS.messages.gemini_api_error.replace("{}", &resp.status().to_string()));
    }

    let body: GeminiListResponse = resp.json().await.map_err(|e| crate::prompts::STRINGS.messages.gemini_parse_error.replace("{}", &e.to_string()))?;
    
    // Filter for generateContent supported models usually, but for now just list all "models/"
    // The API returns names like "models/gemini-1.5-flash"
    let models = body.models.unwrap_or_default().into_iter()
        .map(|m| m.name.replace("models/", ""))
        .filter(|n| n.contains("gemini")) // Simple filter
        .collect();

    Ok(models)
}

pub async fn list_anthropic_models(config: &AppConfig) -> Result<Vec<String>, String> {
    // Find any agent with protocol "claude" or "anthropic"
    let agent_config = config.agents.values()
        .find(|c| c.protocol == "claude" || c.protocol == "anthropic")
        .ok_or_else(|| "No anthropic agent configured".to_string())?;

    let api_key = if let Some(k) = &agent_config.api_key {
        k.clone()
    } else if let Some(env_var) = &agent_config.api_key_env {
        std::env::var(env_var).map_err(|_| crate::prompts::STRINGS.messages.missing_env_var.replace("{}", env_var))?
    } else {
        std::env::var("ANTHROPIC_API_KEY").map_err(|_| "Missing ANTHROPIC_API_KEY")?
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client.get("https://api.anthropic.com/v1/models")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .send()
        .await
        .map_err(|e| crate::prompts::STRINGS.messages.anthropic_fetch_failed.replace("{}", &e.to_string()))?;
        
    if !resp.status().is_success() {
        return Err(crate::prompts::STRINGS.messages.anthropic_api_error.replace("{}", &resp.status().to_string()));
    }
    
    let body: AnthropicListResponse = resp.json().await.map_err(|e| crate::prompts::STRINGS.messages.anthropic_parse_error.replace("{}", &e.to_string()))?;
    
    Ok(body.data.into_iter().map(|m| m.id).collect())
}
