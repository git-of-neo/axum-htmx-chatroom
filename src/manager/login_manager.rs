use super::User;

#[derive(Clone)]
pub struct LoginManager<'a> {
    pool: &'a sqlx::SqlitePool,
}

impl<'a> LoginManager<'a> {
    pub fn new(pool: &'a sqlx::SqlitePool) -> Self {
        Self { pool }
    }
}

#[derive(Debug)]
pub enum Error {
    EmailTaken,
    PasswordMismatch,
    WrongPassword,
    EmailTakenAndPasswordMismatch,
    DatabaseError(sqlx::Error),
}

impl From<sqlx::Error> for Error {
    fn from(value: sqlx::Error) -> Self {
        Error::DatabaseError(value)
    }
}

fn compare_password<'a>(a: &'a str, b: &'a str) -> bool {
    a == b
}

impl LoginManager<'_> {
    pub async fn get_user(&self, email: &str, password: &str) -> Result<User, Error> {
        let user = sqlx::query_as!(
            User,
            "SELECT id, email, password FROM User WHERE email=?",
            email
        )
        .fetch_one(self.pool)
        .await?;

        if compare_password(&user.password, password) {
            Ok(user)
        } else {
            Err(Error::WrongPassword)
        }
    }

    pub async fn new_user(
        &self,
        email: &str,
        password: &str,
        confirm_password: &str,
    ) -> Result<(), Error> {
        let exists =
            sqlx::query_scalar!("SELECT EXISTS(SELECT id FROM User WHERE email = ?)", email)
                .fetch_one(self.pool)
                .await?
                .unwrap()
                >= 1;

        match (exists, compare_password(password, confirm_password)) {
            (true, true) => Err(Error::EmailTakenAndPasswordMismatch),
            (false, true) => Err(Error::PasswordMismatch),
            (true, false) => Err(Error::EmailTaken),
            (false, false) => {
                self.persist_new_user(email, password).await?;
                Ok(())
            }
        }
    }

    async fn persist_new_user(&self, email: &str, password: &str) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "INSERT INTO User(email, password) VALUES (?, ?)",
            email,
            password
        )
        .execute(self.pool)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test]
    async fn ok_create_new_user(pool: sqlx::SqlitePool) {
        assert!(LoginManager::new(&pool)
            .new_user("test123@example.com", "test123", "test123")
            .await
            .is_ok())
    }

    #[sqlx::test(fixtures("users"))]
    async fn ok_get_user(pool: sqlx::SqlitePool) {
        assert!(LoginManager::new(&pool)
            .get_user("test123@example.com", "test123")
            .await
            .is_ok())
    }
}
