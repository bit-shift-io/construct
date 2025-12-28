use crate::config::AppConfig;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct GeminiModel {
    name: String,
    #[serde(rename = "displayName")]
    #[allow(dead_code)]
    display_name: Option<String>,
    #[serde(rename = "supportedGenerationMethods")]
    supported_generation_methods: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct GeminiListResponse {
    models: Option<Vec<GeminiModel>>,
}

#[derive(Debug, Deserialize)]
struct AnthropicModel {
    id: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicListResponse {
    data: Vec<AnthropicModel>,
}

pub async fn list_gemini_models(config: &AppConfig) -> Result<Vec<String>, String> {
    // ... (unchanged)
    let agent_config = config
        .agents
        .values()
        .find(|c| c.protocol == "gemini")
        .ok_or_else(|| "No gemini agent configured".to_string())?;

    let api_key = if let Some(k) = &agent_config.api_key {
        k.clone()
    } else if let Some(env_var) = &agent_config.api_key_env {
        std::env::var(env_var).map_err(|_| {
            crate::strings::STRINGS
                .messages
                .missing_env_var
                .replace("{}", env_var)
        })?
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

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models?key={}",
        api_key
    );

    let resp = client.get(&url).send().await.map_err(|e| {
        crate::strings::STRINGS
            .messages
            .gemini_fetch_failed
            .replace("{}", &e.to_string())
    })?;

    if !resp.status().is_success() {
        return Err(crate::strings::STRINGS
            .messages
            .gemini_api_error
            .replace("{}", &resp.status().to_string()));
    }

    let body: GeminiListResponse = resp.json().await.map_err(|e| {
        crate::strings::STRINGS
            .messages
            .gemini_parse_error
            .replace("{}", &e.to_string())
    })?;

    // Use all models returned by API, trusting user/API filtering
    let models = body
        .models
        .unwrap_or_default()
        .into_iter()
        .map(|m| m.name.replace("models/", ""))
        .collect();

    Ok(models)
}

pub async fn list_anthropic_models(config: &AppConfig) -> Result<Vec<String>, String> {
    // Find any agent with protocol "claude" or "anthropic"
    let agent_config = config
        .agents
        .values()
        .find(|c| c.protocol == "claude" || c.protocol == "anthropic")
        .ok_or_else(|| "No anthropic agent configured".to_string())?;

    let api_key = if let Some(k) = &agent_config.api_key {
        k.clone()
    } else if let Some(env_var) = &agent_config.api_key_env {
        std::env::var(env_var).map_err(|_| {
            crate::strings::STRINGS
                .messages
                .missing_env_var
                .replace("{}", env_var)
        })?
    } else {
        std::env::var("ANTHROPIC_API_KEY").map_err(|_| "Missing ANTHROPIC_API_KEY")?
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get("https://api.anthropic.com/v1/models")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .send()
        .await
        .map_err(|e| {
            crate::strings::STRINGS
                .messages
                .anthropic_fetch_failed
                .replace("{}", &e.to_string())
        })?;

    if !resp.status().is_success() {
        return Err(crate::strings::STRINGS
            .messages
            .anthropic_api_error
            .replace("{}", &resp.status().to_string()));
    }

    let body: AnthropicListResponse = resp.json().await.map_err(|e| {
        crate::strings::STRINGS
            .messages
            .anthropic_parse_error
            .replace("{}", &e.to_string())
    })?;

    Ok(body.data.into_iter().map(|m| m.id).collect())
}

#[derive(Debug, Deserialize)]
struct OpenAIModel {
    id: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIListResponse {
    data: Vec<OpenAIModel>,
}

pub async fn list_groq_models(config: &AppConfig) -> Result<Vec<String>, String> {
    let agent_config = config
        .agents
        .values()
        .find(|c| c.protocol == "groq")
        // Try fallback to openai protocol if user aliased it?
        // But user explicitly used "groq" protocol in example.
        .ok_or_else(|| "No groq agent configured".to_string())?;

    let api_key = if let Some(k) = &agent_config.api_key {
        k.clone()
    } else if let Some(env_var) = &agent_config.api_key_env {
        std::env::var(env_var).map_err(|_| {
            crate::strings::STRINGS
                .messages
                .missing_env_var
                .replace("{}", env_var)
        })?
    } else {
        std::env::var("GROQ_API_KEY").map_err(|_| "Missing GROQ_API_KEY")?
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get("https://api.groq.com/openai/v1/models")
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .map_err(|_e| "Failed to fetch Groq models".to_string())?;

    if !resp.status().is_success() {
        return Err(format!("Groq API Error: {}", resp.status()));
    }

    let body: OpenAIListResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Groq response: {}", e))?;

    Ok(body.data.into_iter().map(|m| m.id).collect())
}

pub async fn list_zai_models(config: &AppConfig) -> Result<Vec<String>, String> {
    let agent_config = config
        .agents
        .values()
        .find(|c| c.protocol == "zai")
        .ok_or_else(|| "No zai agent configured".to_string())?;

    let api_key = if let Some(k) = &agent_config.api_key {
        k.clone()
    } else if let Some(env_var) = &agent_config.api_key_env {
        std::env::var(env_var).map_err(|_| {
            crate::strings::STRINGS
                .messages
                .missing_env_var
                .replace("{}", env_var)
        })?
    } else {
        std::env::var("ZAI_API_KEY").map_err(|_| "Missing ZAI_API_KEY")?
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get("https://api.z.ai/api/anthropic/v1/models")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .send()
        .await
        .map_err(|e| {
            crate::strings::STRINGS
                .messages
                .anthropic_fetch_failed
                .replace("{}", &e.to_string())
        })?;

    if !resp.status().is_success() {
        return Err(crate::strings::STRINGS
            .messages
            .anthropic_api_error
            .replace("{}", &resp.status().to_string()));
    }

    let body: AnthropicListResponse = resp.json().await.map_err(|e| {
        crate::strings::STRINGS
            .messages
            .anthropic_parse_error
            .replace("{}", &e.to_string())
    })?;

    Ok(body.data.into_iter().map(|m| m.id).collect())
}
