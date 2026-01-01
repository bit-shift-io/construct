//! # Help Command
//!
//! Handles the `.help` command.
//! Displays the main help menu to the user.

use crate::domain::traits::ChatProvider;
use anyhow::Result;

pub async fn handle_help(chat: &impl ChatProvider) -> Result<()> {
    chat.send_message(crate::strings::help::MAIN).await.map(|_| ()).map_err(|e| anyhow::anyhow!(e))
}
