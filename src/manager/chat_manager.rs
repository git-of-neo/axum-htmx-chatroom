use super::{ChatMessage, ChatRoom, User};

pub struct ChatManager<'a> {
    pool: &'a sqlx::SqlitePool,
}

impl<'a> ChatManager<'a> {
    pub fn new(pool: &'a sqlx::SqlitePool) -> Self {
        Self { pool }
    }
}

impl ChatManager<'_> {
    pub async fn new_chat(
        &self,
        user: &User,
        room: &ChatRoom,
        msg: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "INSERT INTO Chat(user_id, room_id, message) VALUES (?, ?, ?)",
            user.id,
            room.id,
            msg
        )
        .execute(self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_room(&self, room_id: i64) -> Result<ChatRoom, sqlx::Error> {
        sqlx::query_as!(ChatRoom, "SELECT * FROM ChatRoom WHERE id=?;", room_id)
            .fetch_one(self.pool)
            .await
    }

    pub async fn list_chats(&self, room: &ChatRoom) -> Result<Vec<ChatMessage>, sqlx::Error> {
        Ok(sqlx::query_as!(
            ChatMessage,
            "SELECT * FROM Chat WHERE room_id = ? ORDER BY time_created ASC;",
            room.id
        )
        .fetch_all(self.pool)
        .await?)
    }
}
