use async_trait::async_trait;
use anyhow::Result;

pub mod matrix;

#[async_trait]
pub trait ChatService: Send + Sync {
    /// Returns the unique identifier for the room/channel.
    fn room_id(&self) -> String;

    /// Sends a markdown formatted message.
    async fn send_markdown(&self, content: &str) -> Result<()>;

    /// Sends a plain text message.
    async fn send_plain(&self, content: &str) -> Result<()>;

    /// Sets the typing status.
    async fn typing(&self, active: bool) -> Result<()>;
}
