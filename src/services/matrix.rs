use super::ChatService;
use anyhow::Result;
use async_trait::async_trait;
use matrix_sdk::room::Room;
use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;

#[derive(Clone)]
pub struct MatrixService {
    room: Room,
}

impl MatrixService {
    pub fn new(room: Room) -> Self {
        Self { room }
    }

    /// Edit a message by event ID (currently sends new message for compatibility)
    ///
    /// TODO: Implement proper Matrix message editing using m.relates_to with m.replace
    /// For now, this sends a new message with updated content
    /// The event ID tracking allows us to reference the original for future true editing
    pub async fn edit_markdown(&self, _event_id: &str, new_content: &str) -> Result<()> {
        // For now, just send a new message with the edited content
        // Once we figure out the correct Matrix SDK API for edits, we'll update this
        self.send_markdown(new_content).await?;
        Ok(())
    }
}

#[async_trait]
impl ChatService for MatrixService {
    fn room_id(&self) -> String {
        self.room.room_id().as_str().to_string()
    }

    async fn send_markdown(&self, content: &str) -> Result<String> {
        let response = self
            .room
            .send(RoomMessageEventContent::text_markdown(content))
            .await?;
        Ok(response.event_id.to_string())
    }

    async fn send_plain(&self, content: &str) -> Result<String> {
        let response = self
            .room
            .send(RoomMessageEventContent::text_plain(content))
            .await?;
        Ok(response.event_id.to_string())
    }

    async fn edit_markdown(&self, event_id: &str, new_content: &str) -> Result<()> {
        // For now, just send a new message with the edited content
        // The event ID tracking ensures we can reference it later
        self.send_markdown(new_content).await?;
        Ok(())
    }

    async fn edit_last_markdown(&self, content: &str) -> Result<()> {
        // Get the last message event ID from state
        // This will be called by MessageHelper which has access to state
        // For now, this is a placeholder - the actual editing happens in MessageHelper
        Ok(())
    }

    async fn typing(&self, active: bool) -> Result<()> {
        self.room.typing_notice(active).await?;
        Ok(())
    }
}
