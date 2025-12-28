use async_trait::async_trait;

mod adapter;
pub mod discovery;
mod factory;

pub use self::adapter::UnifiedAgent;
pub use self::factory::get_agent;

#[derive(Clone)]
pub struct AgentContext {
    pub prompt: String,
    pub working_dir: Option<String>,
    pub model: Option<String>,
    pub status_callback: Option<std::sync::Arc<dyn Fn(String) + Send + Sync>>,
    #[allow(dead_code)] // May not be used by all agents yet
    pub abort_signal: Option<tokio::sync::watch::Receiver<bool>>,
    /// Optional project state manager for logging status messages
    #[allow(dead_code)]
    pub project_state_manager: Option<std::sync::Arc<crate::project_state::ProjectStateManager>>,
}

impl std::fmt::Debug for AgentContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentContext")
            .field("prompt", &self.prompt)
            .field("working_dir", &self.working_dir)
            .field("model", &self.model)
            .field(
                "abort_signal",
                &self.abort_signal.as_ref().map(|_| "Some(Rx)"),
            )
            .field(
                "status_callback",
                &self.status_callback.as_ref().map(|_| "Some(Fn)"),
            )
            .finish()
    }
}

#[async_trait]
pub trait Agent: Send + Sync {
    /// Executes a prompt in the given context and returns the response.
    async fn execute(&self, context: &AgentContext) -> Result<String, String>;

    /// Returns the name of the agent (e.g., "gemini", "claude").
    #[allow(dead_code)]
    fn name(&self) -> &str;
}
