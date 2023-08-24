use axum::{
    http::Request,
    response::{Html, IntoResponse},
    Extension,
};
use futures::{sink::SinkExt, stream::StreamExt};
use sqlx::SqlitePool;
use std::{ops::ControlFlow, sync::Arc};

use askama::Template;
use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    http::{header::SET_COOKIE, HeaderMap},
    middleware, routing, Form,
};
use axum_extra::extract::cookie;
use serde::Deserialize;
use tokio::sync::broadcast;
mod manager;
use manager::{
    chat_manager::ChatManager,
    login_manager::{self, LoginManager},
    session_manager::{SessionId, SessionManager},
    ChatRoom, User,
};

static SESSION_ID_KEY: &'static str = "session_id";

#[derive(Clone)]
struct AppState {
    tx: broadcast::Sender<String>,
    pool: sqlx::SqlitePool,
}

impl AppState {
    fn new(tx: broadcast::Sender<String>, pool: sqlx::SqlitePool) -> Self {
        Self { tx: tx, pool: pool }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    secrets_validator::check_env!();

    let pool = SqlitePool::connect(&dotenvy::var("DATABASE_URL")?).await?;
    sqlx::migrate!().run(&pool).await?;

    let (tx, _rx) = broadcast::channel(100);
    let state = Arc::new(AppState::new(tx, pool));

    match LoginManager::new(&state.pool)
        .new_user("test@example.com", "test123", "test123")
        .await
    {
        Ok(_) | Err(login_manager::Error::EmailTaken) => Ok(()),
        e => e,
    }
    .unwrap();

    let app = axum::Router::new()
        .route("/", routing::get(index))
        .route("/ws", routing::get(ws_handler))
        .route("/login", routing::get(login))
        .route("/login", routing::post(try_login))
        .route("/register", routing::get(register))
        .route("/register", routing::post(try_register))
        // layers (middlewares) are from bottom to top
        .layer(middleware::from_fn_with_state(
            state.clone(),
            authenticate_session_id,
        ))
        .with_state(state);

    axum::Server::bind(&"0.0.0.0:3002".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}

#[derive(Template)]
#[template(path = "redirect.html")]
struct RedirectTemplate {
    url: String,
}

async fn authenticate_session_id<B>(
    State(state): State<Arc<AppState>>,
    jar: cookie::CookieJar,
    mut request: Request<B>,
    next: middleware::Next<B>,
) -> impl IntoResponse {
    let uri = request.uri().path();

    if uri != "/login" && uri != "/register" {
        match jar.get(SESSION_ID_KEY) {
            Some(&ref cookie) => {
                let user = SessionManager::new(&state.pool)
                    .get_user(SessionId(cookie.value().to_string()))
                    .await;
                if user.is_err() {
                    return RedirectTemplate {
                        url: "/login".to_owned(),
                    }
                    .into_response();
                }
                request.extensions_mut().insert(user.unwrap());
            }
            None => {
                return RedirectTemplate {
                    url: "/login".to_owned(),
                }
                .into_response();
            }
        }
    }

    next.run(request).await
}

#[derive(Deserialize, Debug)]
struct WsPayload {
    chat_message: String,
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| websocket(socket, state, user))
}

fn process_message(msg: Result<Message, axum::Error>) -> ControlFlow<(), WsPayload> {
    if let Ok(Message::Text(txt)) = msg {
        return ControlFlow::Continue(serde_json::from_str::<WsPayload>(txt.as_str()).unwrap());
    }
    ControlFlow::Break(())
}

#[derive(Template)]
#[template(path = "new_chat.html")]
struct NewChatTemplate {
    msg: String,
}

async fn websocket<'a>(socket: WebSocket, state: Arc<AppState>, user: User) {
    let (mut sender, mut receiver) = socket.split();

    let mut rx = state.tx.subscribe();
    let sync_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    while let Some(msg) = receiver.next().await {
        let cont = process_message(msg);
        if let ControlFlow::Continue(payload) = cont {
            let msg = payload.chat_message;
            let room = ChatRoom::new();
            ChatManager::new(&state.pool)
                .new_chat(&user, &room, &msg)
                .await
                .unwrap();
            let _ = state
                .tx
                .send(NewChatTemplate { msg: msg }.render().unwrap());
        } else {
            break;
        }
    }

    sync_task.abort()
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    msgs: Vec<String>,
}

async fn index(State(state): State<Arc<AppState>>) -> IndexTemplate {
    let msgs = ChatManager::new(&state.pool)
        .list_chats(&ChatRoom::new())
        .await
        .unwrap()
        .into_iter()
        .map(|msg| msg.message)
        .collect();
    IndexTemplate { msgs }
}

#[derive(Deserialize)]
struct LoginForm {
    email: String,
    password: String,
}

#[derive(Template)]
#[template(path = "login_attempt.html")]
struct LoginAttempt {
    success: bool,
}

async fn try_login(
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
#[template(path = "login.html")]
struct LoginTemplate {}

async fn login() -> LoginTemplate {
    LoginTemplate {}
}

#[derive(Template)]
#[template(path = "widget_register.html")]
struct RegisterWidget {
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

async fn register() -> RegisterWidget {
    RegisterWidget {
        ..Default::default()
    }
}

#[derive(Deserialize)]
struct RegisterUserForm {
    email: String,
    password: String,
    confirm_password: String,
}

async fn try_register(
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
