use futures::{sink::SinkExt, stream::StreamExt};
use rand::{distributions::Alphanumeric, Rng};
use std::{
    collections::HashMap,
    fmt::{self, Display},
    hash::{Hash, Hasher},
    ops::ControlFlow,
    sync::{Arc, Mutex},
};

use askama::Template;
use askama_axum::IntoResponse;
use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    http::{header::SET_COOKIE, HeaderMap},
    routing, Error, Form,
};
use serde::Deserialize;
use tokio::sync::broadcast;

struct AppState {
    msgs: Mutex<Vec<String>>,
    tx: broadcast::Sender<String>,
    login_manager: LoginManager<'static>,
}

#[tokio::main]
async fn main() {
    let (tx, _rx) = broadcast::channel(100);
    let state = Arc::new(AppState {
        msgs: Mutex::new(Vec::from(
            [
                "Hey there! How's your day going?",
                "Did you catch that new movie everyone's talking about?",
                "Pizza or tacos? It's the ultimate food debate!",
                "Just wanted to say hi and spread some positivity your way!",
                "I'm counting down the days until the weekend. Anyone else?",
                "Have you ever tried bungee jumping? It's on my bucket list!",
                "Guess what? I adopted a puppy and my heart's melting.",
                "Quick poll: Cats or dogs? Let the battle of cuteness begin!",
                "Netflix recommendations, anyone? I've watched everything on my list.",
                "If you could travel anywhere right now, where would you go?",
            ]
            .map(|s| s.to_owned()),
        )),
        tx: tx,
        login_manager: LoginManager {
            store: HashMap::new(),
        },
    });

    let app = axum::Router::new()
        .route("/", routing::get(index))
        .route("/chat", routing::get(chat))
        .route("/ws", routing::get(ws_handler))
        .route("/login", routing::get(login))
        .route("/login", routing::post(try_login))
        .with_state(state);

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[derive(Deserialize, Debug)]
struct WsPayload {
    chat_message: String,
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket(socket, state))
}

fn process_message(msg: Result<Message, Error>) -> ControlFlow<(), WsPayload> {
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

async fn websocket(socket: WebSocket, state: Arc<AppState>) {
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
            state.msgs.lock().unwrap().push(msg.clone());
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
struct IndexTemplate {}

async fn index() -> IndexTemplate {
    IndexTemplate {}
}

#[derive(Template)]
#[template(path = "chat.html")]
struct ChatTemplate {
    msgs: Vec<String>,
}

async fn chat(State(state): State<Arc<AppState>>) -> ChatTemplate {
    let msgs = state.msgs.lock().unwrap();
    ChatTemplate {
        msgs: msgs.to_vec(),
    }
}

struct User<'a> {
    email: &'a str,
    password: &'a str,
}

impl Hash for User<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.email.hash(state)
    }
}

struct SessionId(String);

impl Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.clone())
    }
}

fn generate_session_id(user: &User) -> SessionId {
    let mut rng = rand::thread_rng();
    SessionId(
        (0..13)
            .map(|_| rng.sample(Alphanumeric))
            .map(char::from)
            .collect::<String>(),
    )
}

fn compare_password<'a>(a: &'a str, b: &'a str) -> bool {
    a == b
}

struct LoginManager<'a> {
    store: HashMap<&'a str, User<'a>>,
}

impl LoginManager<'_> {
    async fn get_user<'a>(&self, email: &str, password: &str) -> Option<&User> {
        if let Some(user) = self.store.get(email) {
            if compare_password(&user.email, password) {
                return Some(user);
            }
        }
        None
    }
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
    let mut headers = HeaderMap::new();
    let session_id = match state
        .login_manager
        .get_user(email.as_str(), password.as_str())
        .await
    {
        Some(user) => Some(generate_session_id(&user)),
        None => None,
    };

    let success = session_id.is_some();
    if let Some(session_id) = session_id {
        headers.insert(
            SET_COOKIE,
            format!("user_id={}", session_id).parse().unwrap(),
        );
    }

    (headers, LoginAttempt { success })
}

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate {}

async fn login() -> LoginTemplate {
    LoginTemplate {}
}
