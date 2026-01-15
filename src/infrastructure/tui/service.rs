use async_trait::async_trait;
use crate::domain::traits::ChatProvider;

#[derive(Clone)]
#[allow(dead_code)]
pub struct TuiService {
    pub room_id: String,
}

impl TuiService {
    pub fn new(room_id: String) -> Self {
        Self { room_id }
    }
}

#[async_trait]
impl ChatProvider for TuiService {
    async fn send_message(&self, _content: &str) -> Result<String, String> {
        // In TUI, "sending" a message usually means the bot is replying.
        // The FeedManager handles the actual display logic.
        // Here we might just return a dummy ID, or log it?
        // For now, TUI doesn't have a native "Message History" separate from Feed, 
        // so we just acknowledge it.
        Ok("tui-msg-id".to_string())
    }

    async fn edit_message(&self, _message_id: &str, _content: &str) -> Result<(), String> {
        // TUI might support editing if we tracked message IDs in a buffer?
        // For prototype, no-op.
        Ok(())
    }

    async fn send_notification(&self, _content: &str) -> Result<(), String> {
        Ok(())
    }

    async fn typing(&self, _active: bool) -> Result<(), String> {
        Ok(())
    }

    async fn get_latest_event_id(&self) -> Result<Option<String>, String> {
        Ok(None)
    }

    fn room_id(&self) -> String {
        self.room_id.clone()
    }
}
