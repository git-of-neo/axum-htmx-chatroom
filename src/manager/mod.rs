use sqlx::types::chrono::NaiveDateTime;

pub mod chat_manager;
pub mod login_manager;
pub mod session_manager;

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct User {
    id: i64,
    email: String,
    password: String,
}

pub struct ChatMessage {
    id: i64,
    user_id: Option<i64>,
    room_id: i64,
    pub message: String,
    time_created: NaiveDateTime,
}

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct ChatRoom {
    id: i64,
    name: String,
}
