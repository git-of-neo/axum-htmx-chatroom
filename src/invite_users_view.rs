use std::sync::Arc;

use axum::http::{HeaderMap, HeaderName};
use axum::{extract::State, Form};

use askama::Template;
use serde::Deserialize;

use crate::manager::chat_manager::ChatManager;
use crate::manager::{user_manager::UserManager, User};
use crate::utils;
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

#[derive(Deserialize)]
pub struct InviteForm {
    #[serde(deserialize_with = "utils::i64_from_string")]
    user_id: i64,
}

#[derive(Template)]
#[template(path = "invite_user_results.html")]
pub struct InviteUserResultsTemplate {
    success: bool,
}

pub async fn try_invite_user(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Form(data): Form<InviteForm>,
) -> InviteUserResultsTemplate {
    if let Some(refer) = headers.get(HeaderName::from_static("referer")) {
        let room_id = refer
            .to_str()
            .unwrap()
            .split("/")
            .into_iter()
            .last()
            .unwrap()
            .parse::<i64>()
            .unwrap();
        let success = ChatManager::new(&state.pool)
            .invite(data.user_id, room_id)
            .await
            .is_ok();
        return InviteUserResultsTemplate { success };
    }
    InviteUserResultsTemplate { success: false }
}
