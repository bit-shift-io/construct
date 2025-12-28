use super::ChatService;
use anyhow::Result;
use async_trait::async_trait;
use matrix_sdk::room::Room;
use matrix_sdk::ruma::events::relation::Replacement;
use matrix_sdk::ruma::events::room::message::{
    Relation, RoomMessageEventContent, RoomMessageEventContentWithoutRelation,
};
use matrix_sdk::ruma::EventId;
use std::convert::TryFrom;

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
    pub async fn edit_markdown(&self, event_id: &str, new_content: &str) -> Result<()> {
        let event_id = <&EventId>::try_from(event_id)?;
        let mut content = RoomMessageEventContent::text_markdown(new_content);
        let replacement_content = RoomMessageEventContentWithoutRelation::from(content.clone());

        content.relates_to = Some(Relation::Replacement(Replacement::new(
            event_id.to_owned(),
            replacement_content,
        )));

        self.room.send(content).await?;
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
        self.edit_markdown(event_id, new_content).await
    }


    async fn typing(&self, active: bool) -> Result<()> {
        self.room.typing_notice(active).await?;
        Ok(())
    }
}
