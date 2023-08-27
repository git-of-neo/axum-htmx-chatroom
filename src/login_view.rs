use axum::response::{Html, IntoResponse};
use std::sync::Arc;

use askama::Template;
use axum::{
    extract::State,
    http::{header::SET_COOKIE, HeaderMap},
    Form,
};
use serde::Deserialize;

use crate::manager;
use crate::AppState;
use crate::SESSION_ID_KEY;

use manager::{
    login_manager::{self, LoginManager},
    session_manager::SessionManager,
};

#[derive(Deserialize)]
pub struct LoginForm {
    email: String,
    password: String,
}

#[derive(Template)]
#[template(path = "login_view/login_attempt.html")]
struct LoginAttempt {
    success: bool,
}

pub async fn try_login(
    State(state): State<Arc<AppState>>,
    Form(credentials): Form<LoginForm>,
) -> impl IntoResponse {
    let LoginForm { email, password } = credentials;
    let user = LoginManager::new(&state.pool)
        .get_user(email.as_str(), password.as_str())
        .await;

    let mut headers = HeaderMap::new();
    match user {
        Ok(user) => {
            headers.insert("HX-Redirect", "/".parse().unwrap());
            headers.insert(
                SET_COOKIE,
                format!(
                    "{}={}",
                    SESSION_ID_KEY,
                    SessionManager::new(&state.pool)
                        .generate_session_id_for(&user)
                        .await
                        .unwrap()
                )
                .parse()
                .unwrap(),
            );
            (headers, Html("").into_response())
        }
        _ => (
            headers,
            Html(LoginAttempt { success: false }.render().unwrap()).into_response(),
        ),
    }
}

#[derive(Template)]
#[template(path = "login_view/login.html")]
pub struct LoginTemplate {}

pub async fn login() -> LoginTemplate {
    LoginTemplate {}
}

#[derive(Template)]
#[template(path = "login_view/widget_register.html")]
pub struct RegisterWidget {
    email_cache: String,
    email_taken: bool,
    mismatch_passwords: bool,
}

impl Default for RegisterWidget {
    fn default() -> Self {
        Self {
            email_cache: String::new(),
            email_taken: false,
            mismatch_passwords: false,
        }
    }
}

pub async fn register() -> RegisterWidget {
    RegisterWidget {
        ..Default::default()
    }
}

#[derive(Deserialize)]
pub struct RegisterUserForm {
    email: String,
    password: String,
    confirm_password: String,
}

pub async fn try_register(
    State(state): State<Arc<AppState>>,
    Form(form): Form<RegisterUserForm>,
) -> impl IntoResponse {
    let RegisterUserForm {
        email,
        password,
        confirm_password,
    } = form;

    let mut header = HeaderMap::new();
    let user = LoginManager::new(&state.pool)
        .new_user(&email, &password, &confirm_password)
        .await;

    if user.is_ok() {
        header.insert("HX-Redirect", "/login".parse().unwrap());
    }

    let body = match user {
        Ok(_) => "".to_owned(),

        Err(login_manager::Error::EmailTaken) => RegisterWidget {
            email_taken: true,
            email_cache: email,
            ..Default::default()
        }
        .render()
        .unwrap(),

        Err(login_manager::Error::PasswordMismatch) => RegisterWidget {
            mismatch_passwords: true,
            email_cache: email,
            ..Default::default()
        }
        .render()
        .unwrap(),

        Err(login_manager::Error::EmailTakenAndPasswordMismatch) => RegisterWidget {
            email_taken: true,
            email_cache: email,
            mismatch_passwords: true,
        }
        .render()
        .unwrap(),

        _ => todo!("unhandled database error"),
    };

    (header, Html(body).into_response())
}
