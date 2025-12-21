use serde::Deserialize;
use std::collections::HashMap;

/// Main application configuration structure.
/// Matches the layout of `data/config.yaml`.
#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub services: ServicesConfig,
    #[serde(default)]
    pub agents: AgentsConfig,
    #[serde(default)]
    pub bridges: HashMap<String, Vec<BridgeEntry>>,
    pub commands: HashMap<String, HashMap<String, String>>,
    #[serde(default)]
    pub system: SystemConfig,
}

/// System-level settings for the bot.
#[derive(Debug, Default, Deserialize, Clone)]
pub struct SystemConfig {
    #[serde(default)]
    pub projects_dir: Option<String>,
}

/// Represents a specific bridge entry connecting a service to a channel.
#[derive(Debug, Deserialize, Clone)]
pub struct BridgeEntry {
    pub service: String,
    pub channel: Option<String>,
}

/// Configuration for various connected services.
#[derive(Debug, Deserialize, Clone)]
pub struct ServicesConfig {
    pub matrix: MatrixConfig,
}

pub type AgentsConfig = HashMap<String, AgentConfig>;

#[derive(Debug, Deserialize, Clone)]
pub struct AgentConfig {
    #[serde(default)]
    pub protocol: String,
    #[serde(default)]
    pub command: Option<String>, // Legacy CLI command
    #[serde(default)]
    pub model: String,           // Required for Rig
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>, // e.g. "GEMINI_API_KEY"
}

/// Specific configuration for the Matrix service.
#[derive(Debug, Deserialize, Clone)]
pub struct MatrixConfig {
    #[serde(default)]
    pub protocol: String,
    pub username: String,
    pub password: String,
    pub homeserver: String,
    #[serde(default)]
    pub display_name: Option<String>,
}
