use std::sync::Arc;

use axum::{extract::State, Form};

use askama::Template;
use serde::Deserialize;

use crate::manager::{user_manager::UserManager, User};
use crate::AppState;

#[derive(Template)]
#[template(path = "user_list.html")]
pub struct UserListTemplate {
    users: Vec<User>,
}

#[derive(Deserialize)]
pub struct SearchFrom {
    search: String,
}

pub async fn list_users(
    State(state): State<Arc<AppState>>,
    Form(data): Form<SearchFrom>,
) -> UserListTemplate {
    let term = data.search;
    UserListTemplate {
        users: UserManager::new(&state.pool)
            .search_user(&term)
            .await
            .unwrap_or(Vec::new()),
    }
}

#[derive(Template)]
#[template(path = "invite_user.html")]
pub struct InviteUserTemplate {}

pub async fn invite_user() -> InviteUserTemplate {
    InviteUserTemplate {}
}
