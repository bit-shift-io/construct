use crate::domain::config::AppConfig;
use crate::application::state::{BotState};
use crate::domain::traits::ChatProvider;
use tokio::sync::Mutex;
use std::sync::Arc;
use crate::interface::commands::wizard;

pub async fn handle_agent<C>(
    config: &AppConfig,
    state: &Arc<Mutex<BotState>>,
    chat: &C,
    _args: &str, // Arguments ignored, always start wizard for now (or could parse if we wanted to support both)
) -> anyhow::Result<()>
where
    C: ChatProvider + Send + Sync + 'static,
{
    // Start Wizard
    wizard::start_agent_wizard(config, state, chat).await
}
