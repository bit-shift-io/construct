use super::{Agent, UnifiedAgent};
use crate::config::AppConfig;

pub fn get_agent(name: &str, config: &AppConfig) -> Box<dyn Agent> {
    // 1. Try to find the agent config by name in the HashMap
    if let Some(agent_conf) = config.agents.get(name) {
        return Box::new(UnifiedAgent {
            provider: agent_conf.protocol.clone(),
            config: Some(agent_conf.clone())
        });
    }

    // 2. Legacy/Alias Fallback (e.g. user types "gemini" but config has "gemini_cli" or vice versa)
    // We iterate to find a config with matching protocol if exact name match failed
    for (k, _v) in &config.agents {
         if k == name {
             return get_agent(k, config);
         }
    }

    // 3. Simple Alias Handling for common names if not explicitly keyed
    if name == "copilot" && config.agents.contains_key("github_copilot") {
         return get_agent("github_copilot", config);
    }

    // Try to find a default "deep_ai" or "deepai" config to use as fallback
    let default_config = config.agents.get("deep_ai").or(config.agents.get("deepai")).cloned();

    // Default Fallback - UnifiedAgent with provider "deepai" (default)
    Box::new(UnifiedAgent {
        provider: "deepai".to_string(),
        config: default_config
    })
}
