use anyhow::Result;
use async_trait::async_trait;

pub mod matrix;

#[async_trait]
pub trait ChatService: Send + Sync {
    /// Returns the unique identifier for the room/channel.
    fn room_id(&self) -> String;

    /// Sends a markdown formatted message and returns the event ID.
    async fn send_markdown(&self, content: &str) -> Result<String>;

    /// Sends a plain text message and returns the event ID.
    async fn send_plain(&self, content: &str) -> Result<String>;

    /// Edits a message by event ID.
    async fn edit_markdown(&self, event_id: &str, new_content: &str) -> Result<()>;


    /// Sets the typing status.
    async fn typing(&self, active: bool) -> Result<()>;
}
