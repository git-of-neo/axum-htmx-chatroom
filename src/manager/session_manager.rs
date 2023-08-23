use std::fmt::Display;

use super::User;
use rand::distributions::Alphanumeric;
use rand::Rng;

#[derive(sqlx::Type, Debug)]
#[sqlx(transparent)]
pub struct SessionId(pub String);

impl Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug)]
pub enum Error {
    DoesNotExist,
    DatabaseError(sqlx::Error),
}

impl From<sqlx::Error> for Error {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => Error::DoesNotExist,
            _ => Error::DatabaseError(err),
        }
    }
}

fn random_string_session_id(_user: &User) -> SessionId {
    let mut rng = rand::thread_rng();
    SessionId(
        (0..13)
            .map(|_| rng.sample(Alphanumeric))
            .map(char::from)
            .collect::<String>(),
    )
}

#[derive(Clone)]
pub struct SessionManager<'a> {
    pool: &'a sqlx::SqlitePool,
}

impl<'a> SessionManager<'a> {
    pub fn new(pool: &'a sqlx::SqlitePool) -> Self {
        Self { pool }
    }
}

impl SessionManager<'_> {
    pub async fn get_user(&self, session_id: SessionId) -> Result<User, Error> {
        Ok(sqlx::query_as!(
            User,
            "SELECT * FROM User WHERE id=(SELECT user_id FROM UserSession WHERE session_id = ?)",
            session_id
        )
        .fetch_one(self.pool)
        .await?)
    }

    pub async fn generate_session_id_for(&self, user: &User) -> Result<SessionId, sqlx::Error> {
        let sid = random_string_session_id(user);
        sqlx::query!(
            "INSERT INTO UserSession(session_id, user_id) VAlUES (?, ?)",
            sid,
            user.id
        )
        .execute(self.pool)
        .await?;

        Ok(sid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::login_manager::LoginManager;

    #[sqlx::test(fixtures("users", "sessions"))]
    async fn ok_get_user(pool: sqlx::SqlitePool) {
        assert!(SessionManager::new(&pool)
            .get_user(SessionId("f15wQrWboFNBW".into()))
            .await
            .is_ok())
    }

    #[sqlx::test(fixtures("users"))]
    async fn ok_retrieve_user_from_generated_session_id(pool: sqlx::SqlitePool) {
        let user = LoginManager::new(&pool)
            .get_user("test123@example.com", "test123")
            .await
            .unwrap();
        let session_manager = SessionManager::new(&pool);
        let sid = session_manager
            .generate_session_id_for(&user)
            .await
            .unwrap();
        assert!(session_manager.get_user(sid).await.is_ok())
    }
}
