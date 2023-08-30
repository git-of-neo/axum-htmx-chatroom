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

    pub async fn new_room(
        &self,
        name: &str,
        image_path: &str,
        creator: &User,
    ) -> Result<(), sqlx::Error> {
        let room = sqlx::query_as!(
            ChatRoom,
            "INSERT INTO ChatRoom(name, image_path) VALUES (?, ?) RETURNING *;",
            name,
            image_path
        )
        .fetch_one(self.pool)
        .await?;
        let _ = sqlx::query!(
            "INSERT INTO UserRoom(user_id, room_id) VALUES (?, ?);",
            creator.id,
            room.id
        )
        .execute(self.pool)
        .await?;
        Ok(())
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

    pub async fn list_rooms(&self, user: &User) -> Result<Vec<ChatRoom>, sqlx::Error> {
        Ok(sqlx::query_as!(
            ChatRoom,
            "SELECT * FROM ChatRoom WHERE id IN (SELECT room_id FROM UserRoom WHERE user_id = ?);",
            user.id
        )
        .fetch_all(self.pool)
        .await?)
    }
}
