pub mod login_manager;
pub mod session_manager;

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct User {
    id: i64,
    email: String,
    password: String,
}
