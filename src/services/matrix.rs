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
}

#[async_trait]
impl ChatService for MatrixService {
    fn room_id(&self) -> String {
        self.room.room_id().as_str().to_string()
    }

    async fn send_markdown(&self, content: &str) -> Result<()> {
        self.room
            .send(RoomMessageEventContent::text_markdown(content))
            .await?;
        Ok(())
    }

    async fn send_plain(&self, content: &str) -> Result<()> {
        self.room
            .send(RoomMessageEventContent::text_plain(content))
            .await?;
        Ok(())
    }

    async fn typing(&self, active: bool) -> Result<()> {
        self.room.typing_notice(active).await?;
        Ok(())
    }
}
