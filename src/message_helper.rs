/// Message helper for managing bot messages with edit functionality
///
/// This module provides functionality to update the last bot message instead
/// of sending new messages, reducing chat spam and providing a cleaner UX.
use crate::services::ChatService;
use crate::state::BotState;

/// Message helper for managing bot messages
#[derive(Clone)]
pub struct MessageHelper {
    room_id: String,
}

impl MessageHelper {
    /// Create a new message helper for the given room
    pub fn new(room_id: String) -> Self {
        Self { room_id }
    }

    /// Send a markdown message, tracking the event ID for future edits
    ///
    /// This sends a new message and stores its event ID in RoomState.
    /// Subsequent calls to `edit_markdown` will update this message.
    pub async fn send_markdown(
        &self,
        room: &impl ChatService,
        state: &mut BotState,
        content: &str,
    ) -> Result<(), String> {
        let event_id = room
            .send_markdown(content)
            .await
            .map_err(|e| e.to_string())?;

        let room_state = state.get_room_state(&self.room_id);
        room_state.last_message_event_id = Some(event_id);
        state.save();

        Ok(())
    }

    /// Send a plain text message, tracking the event ID for future edits
    pub async fn send_plain(
        &self,
        room: &impl ChatService,
        state: &mut BotState,
        content: &str,
    ) -> Result<(), String> {
        let event_id = room.send_plain(content).await.map_err(|e| e.to_string())?;

        let room_state = state.get_room_state(&self.room_id);
        room_state.last_message_event_id = Some(event_id);
        state.save();

        Ok(())
    }

    /// Edit the last message with new markdown content
    ///
    /// If there's a tracked last message, edits it.
    /// If no last message is tracked or editing fails, sends a new one instead.
    pub async fn edit_markdown(
        &self,
        room: &impl ChatService,
        state: &mut BotState,
        content: &str,
    ) -> Result<(), String> {
        let room_state = state.get_room_state(&self.room_id);

        if let Some(event_id) = &room_state.last_message_event_id {
            // Try to edit the existing message
            if let Err(e) = room.edit_markdown(event_id, content).await {
                // If editing fails, fall back to sending new message
                // (e.g., message too old, doesn't exist, or API error)
                self.send_markdown(room, state, content).await?;
            } else {
                // Edit successful, event ID remains the same
                return Ok(());
            }
        } else {
            // No last message tracked, send new
            self.send_markdown(room, state, content).await?;
        }

        Ok(())
    }

    /// Reset the last message tracking
    ///
    /// Call this when user sends input, so the next bot response
    /// will be a new message instead of continuing to update the old one.
    pub fn reset_last_message(&self, state: &mut BotState) {
        let room_state = state.get_room_state(&self.room_id);
        room_state.last_message_event_id = None;
        state.save();
    }

    /// Send or edit markdown based on context
    ///
    /// - If `force_new` is true, always send a new message
    /// - Otherwise, edit the last message if it exists
    ///
    /// Use this method to automatically decide whether to send a new message
    /// or update the existing one based on the context.
    pub async fn send_or_edit_markdown(
        &self,
        room: &impl ChatService,
        state: &mut BotState,
        content: &str,
        force_new: bool,
    ) -> Result<(), String> {
        if force_new {
            self.send_markdown(room, state, content).await
        } else {
            self.edit_markdown(room, state, content).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_helper_creation() {
        let helper = MessageHelper::new("!test:example.com".to_string());
        assert_eq!(helper.room_id, "!test:example.com");
    }
}
