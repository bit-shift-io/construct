//! # Admin Command
//!
//! Handles the `.run` or `.exec` command.
//! Allows authorized admins to execute arbitrary shell commands via the MCP client.

use crate::domain::config::AppConfig;
use crate::domain::traits::ChatProvider;
use crate::infrastructure::mcp::client::SharedMcpClient;
use anyhow::Result;

pub async fn handle_admin(
    config: &AppConfig,
    mcp: SharedMcpClient,
    chat: &impl ChatProvider,
    sender: &str,
    command: &str,
    workdir: Option<&str>
) -> Result<()> {
    // Check Permissions
    let is_admin = config.system.admin.iter().any(|a| a.to_lowercase() == sender.to_lowercase());
    if !is_admin {
        let _ = chat.send_notification("Authorization Denied.").await;
        return Ok(());
    }
    
    // Execute Shell Command via MCP
    let mut client = mcp.lock().await;
    match client.execute_command(command, Some(60), workdir).await {
        Ok(output) => {
            let _ = chat.send_message(&format!("```sh\n$ {}\n{}\n```", command, output)).await;
        }
        Err(e) => {
            let _ = chat.send_notification(&format!("Command Failed: {}", e)).await;
        }
    }
    
    Ok(())
}
