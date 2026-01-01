//! # Domain Traits
//!
//! Abstract interfaces for core system components (Chat, LLM).
//! Allows for pluggable implementations in the Infrastructure layer.

use async_trait::async_trait;

/// Abstract interface for a Chat Provider (e.g., Matrix, Slack, Console)
#[async_trait]
pub trait ChatProvider: Send + Sync {
    /// Send a message to the room
    async fn send_message(&self, content: &str) -> Result<String, String>;

    /// Edit a message in the room
    async fn edit_message(&self, message_id: &str, content: &str) -> Result<(), String>;

    /// Send a notification (not tracked/editable)
    async fn send_notification(&self, content: &str) -> Result<(), String>;

    /// Send a typing indicator
    async fn typing(&self, active: bool) -> Result<(), String>;

    /// Get the ID of the latest event in the room (for sticky feed logic)
    async fn get_latest_event_id(&self) -> Result<Option<String>, String>;

    /// Get the current room ID
    fn room_id(&self) -> String;
}

/// Abstract interface for an LLM Provider
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Generate a completion
    async fn completion(&self, prompt: &str, model: &str) -> Result<String, String>;
}
