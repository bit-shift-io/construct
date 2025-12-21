use async_trait::async_trait;

mod adapter;
pub mod discovery;

pub use self::adapter::UnifiedAgent;

#[derive(Debug, Clone)]
pub struct AgentContext {
    pub prompt: String,
    pub working_dir: Option<String>,
    pub model: Option<String>,
}

#[async_trait]
pub trait Agent: Send + Sync {
    /// Executes a prompt in the given context and returns the response.
    async fn execute(&self, context: &AgentContext) -> Result<String, String>;
    
    /// Returns the name of the agent (e.g., "gemini", "claude").
    fn name(&self) -> &str;
}

/// Helper to run a shell command and return stdout/stderr.
pub async fn run_command(command: &str, folder: Option<&str>) -> Result<String, String> {
    use tokio::process::Command;
    
    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        return Err("Empty command".to_string());
    }
    
    let binary = parts[0];
    let args = &parts[1..];
    
    let output = Command::new(binary)
        .args(args)
        .current_dir(folder.unwrap_or("."))
        .output()
        .await
        .map_err(|e| format!("Failed to run command: {}", e))?;
        
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let err = String::from_utf8_lossy(&output.stderr).to_string();
        if !err.is_empty() {
            Err(err)
        } else {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        }
    }
}

/// Legacy wrapper for the existing `run_agent_process` style, defaulting to `aichat`.
pub async fn run_agent_process(command: &str, working_dir: Option<&str>) -> String {
    match run_command(command, working_dir).await {
        Ok(out) => out,
        Err(err) => err,
    }
}

pub fn get_agent(name: &str, config: &crate::config::AppConfig) -> Box<dyn Agent> {
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
