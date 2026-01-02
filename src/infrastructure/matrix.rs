//! # Matrix Service Adapter
//!
//! Implements the `ChatProvider` trait for the Matrix protocol using the `matrix_sdk`.
//! This module acts as the bridge between the generic `ChatProvider` interface used by the bot's core logic
//! and the specific implementation details of the Matrix SDK.

use crate::domain::traits::ChatProvider;
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

    /// Helper to send markdown edits
    async fn internal_edit(&self, event_id: &str, new_content: &str) -> Result<()> {
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
impl ChatProvider for MatrixService {
    fn room_id(&self) -> String {
        self.room.room_id().as_str().to_string()
    }

    async fn send_message(&self, content: &str) -> Result<String, String> {
        tracing::info!("Bot sending message to {}: {}", self.room_id(), content);
        self.room
            .send(RoomMessageEventContent::text_markdown(content))
            .await
            .map(|resp| resp.event_id.to_string())
            .map_err(|e| e.to_string())
    }

    async fn edit_message(&self, message_id: &str, content: &str) -> Result<(), String> {
        self.internal_edit(message_id, content)
            .await
            .map_err(|e| e.to_string())
    }

    async fn send_notification(&self, content: &str) -> Result<(), String> {
        // Notifications are also markdown messages for now
        self.send_message(content).await.map(|_| ())
    }

    async fn typing(&self, active: bool) -> Result<(), String> {
        self.room
            .typing_notice(active)
            .await
            .map_err(|e| e.to_string())
    }

    async fn get_latest_event_id(&self) -> Result<Option<String>, String> {
        Ok(self.room.latest_event().map(|e| e.event_id().expect("Event ID missing").to_string()))
    }
}
