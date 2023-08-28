use axum::{
    http::Request,
    response::{Html, IntoResponse},
    Extension,
};
use futures::{sink::SinkExt, stream::StreamExt};
use sqlx::SqlitePool;
use tower_http::services::ServeDir;

use askama::Template;
use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, State, WebSocketUpgrade,
    },
    http::HeaderMap,
    middleware, routing, Form,
};
use axum_extra::extract::cookie;
use serde::{Deserialize, Deserializer};
use tokio::sync::broadcast;

mod login_view;
mod manager;

use manager::{
    chat_manager::ChatManager,
    login_manager::{self, LoginManager},
    session_manager::{SessionId, SessionManager},
    User,
};

static SESSION_ID_KEY: &'static str = "session_id";
static IMAGE_DIR: &'static str = "static";

#[derive(Clone)]
pub struct AppState {
    tx: broadcast::Sender<WsPayload>,
    pool: sqlx::SqlitePool,
}

impl AppState {
    fn new(tx: broadcast::Sender<WsPayload>, pool: sqlx::SqlitePool) -> Self {
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
        .route("/chat/:room_id", routing::get(chat))
        .route("/ws/:room_id", routing::get(ws_handler))
        .route("/login", routing::get(login_view::login))
        .route("/login", routing::post(login_view::try_login))
        .route("/register", routing::get(login_view::register))
        .route("/register", routing::post(login_view::try_register))
        .nest_service("/static", ServeDir::new(IMAGE_DIR))
        // layers (middlewares) are from bottom to top
        .layer(middleware::from_fn_with_state(
            state.clone(),
            authenticate_session_id,
        ))
        .with_state(state);

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
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

#[derive(Deserialize, Debug, Clone)]
struct WsPayload {
    #[serde(deserialize_with = "i64_from_string")]
    room_id: i64,
    chat_message: String,
}

fn i64_from_string<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;

    match s.parse::<i64>() {
        Ok(int) => Ok(int),
        Err(e) => Err(serde::de::Error::custom(e.to_string())),
    }
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(room_id): Path<i64>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| websocket(socket, state, user, room_id))
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

async fn websocket<'a>(socket: WebSocket, state: Arc<AppState>, user: User, room_id: i64) {
    let (mut sender, mut receiver) = socket.split();
    let manager = ChatManager::new(&state.pool);
    let room = manager.get_room(room_id).await.unwrap();

    let mut rx = state.tx.subscribe();
    let sync_task = tokio::spawn(async move {
        while let Ok(payload) = rx.recv().await {
            if payload.room_id != room_id {
                continue;
            }
            if sender
                .send(Message::Text(
                    NewChatTemplate {
                        msg: payload.chat_message,
                    }
                    .render()
                    .unwrap(),
                ))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    while let Some(msg) = receiver.next().await {
        let cont = process_message(msg);
        if let ControlFlow::Continue(payload) = cont {
            manager
                .new_chat(&user, &room, &payload.chat_message)
                .await
                .unwrap();
            let _ = state.tx.send(payload);
        } else {
            break;
        }
    }

    sync_task.abort()
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {}

async fn index(State(state): State<Arc<AppState>>) -> IndexTemplate {
    IndexTemplate {}
}

#[derive(Template)]
#[template(path = "chat.html")]
struct ChatTemplate {
    room_id: i64,
    msgs: Vec<String>,
}

async fn chat(State(state): State<Arc<AppState>>, Path(room_id): Path<i64>) -> ChatTemplate {
    let manager = ChatManager::new(&state.pool);
    let msgs = match manager.get_room(room_id).await {
        Ok(room) => manager
            .list_chats(&room)
            .await
            .unwrap()
            .into_iter()
            .map(|msg| msg.message)
            .collect(),
        Err(sqlx::Error::RowNotFound) => Vec::new(),
        Err(e) => panic!("{:?}", e),
    };

    ChatTemplate { msgs, room_id }
}
