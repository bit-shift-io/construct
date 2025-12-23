use async_trait::async_trait;

mod adapter;
pub mod discovery;
mod factory;

pub use self::adapter::UnifiedAgent;
pub use self::factory::get_agent;

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
    #[allow(dead_code)]
    fn name(&self) -> &str;
}
