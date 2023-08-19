use axum::{
    http::{header, Request, Response},
    response::{Html, IntoResponse},
};
use futures::{sink::SinkExt, stream::StreamExt};
use rand::{distributions::Alphanumeric, Rng};
use std::{
    collections::HashMap,
    fmt::{self, Display},
    hash::{Hash, Hasher},
    ops::ControlFlow,
    sync::Arc,
};

use askama::Template;
use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    http::{header::SET_COOKIE, HeaderMap},
    middleware, routing, Error, Form,
};
use axum_extra::extract::cookie;
use serde::Deserialize;
use tokio::sync::{broadcast, Mutex};

static SESSION_ID_KEY: &'static str = "session_id";

struct AppState {
    msgs: Mutex<Vec<String>>,
    tx: broadcast::Sender<String>,
    login_manager: Mutex<LoginManager>,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let (tx, _rx) = broadcast::channel(100);
    let mut manager = LoginManager {
        store: HashMap::new(),
    };
    let _ = manager
        .new_user("test@example.com", "test123", "test123")
        .await;

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
        login_manager: Mutex::new(manager),
    });

    let app = axum::Router::new()
        .route("/", routing::get(index))
        .route("/chat", routing::get(chat))
        .route("/ws", routing::get(ws_handler))
        .route("/login", routing::get(login))
        .route("/login", routing::post(try_login))
        .route("/register", routing::get(register))
        .route("/register", routing::post(try_register))
        // layers (middlewares) are from bottom to top
        .layer(middleware::from_fn(authenticate_session_id))
        .with_state(state);

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}

#[derive(Template)]
#[template(path = "redirect.html")]
struct Redirect {
    url: String,
}

async fn authenticate_session_id<B>(
    jar: cookie::CookieJar,
    request: Request<B>,
    next: middleware::Next<B>,
) -> impl IntoResponse {
    let url = request.uri().path();

    if url != "/login" {
        let id = jar.get(SESSION_ID_KEY);

        if id.is_none() {
            return Redirect {
                url: "/login".to_owned(),
            }
            .into_response();
        }
    }

    next.run(request).await
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
            state.msgs.lock().await.push(msg.clone());
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
    let msgs = state.msgs.lock().await;
    ChatTemplate {
        msgs: msgs.to_vec(),
    }
}

struct User {
    email: String,
    password: String,
}

impl User {
    fn new(email: &str, password: &str) -> Self {
        Self {
            email: email.to_owned(),
            password: password.to_owned(),
        }
    }
}

impl Hash for User {
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

#[derive(Debug)]
enum ErrorKind {
    EmailTaken,
    PasswordMismatch,
    EmailTakenAndPasswordMismatch,
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Email is already taken by another user")
    }
}

struct LoginManager {
    store: HashMap<String, User>,
}

impl LoginManager {
    async fn get_user(&self, email: &str, password: &str) -> Option<&User> {
        if let Some(user) = self.store.get(email) {
            if compare_password(&user.password, password) {
                return Some(user);
            }
        }
        None
    }

    async fn new_user(
        &mut self,
        email: &str,
        password: &str,
        confirm_password: &str,
    ) -> Result<(), ErrorKind> {
        let r = match (self.store.contains_key(email), password != confirm_password) {
            (true, true) => Err(ErrorKind::EmailTakenAndPasswordMismatch),
            (false, true) => Err(ErrorKind::PasswordMismatch),
            (true, false) => Err(ErrorKind::EmailTaken),
            (false, false) => Ok(()),
        };
        if r.is_ok() {
            self.store
                .insert(email.to_owned(), User::new(email, password));
        }

        r
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
    let manager = state.login_manager.lock().await;
    let user = manager.get_user(email.as_str(), password.as_str()).await;

    let mut headers = HeaderMap::new();
    match user {
        Some(user) => {
            headers.insert("HX-Redirect", "/".parse().unwrap());
            headers.insert(
                SET_COOKIE,
                format!("{}={}", SESSION_ID_KEY, generate_session_id(user))
                    .parse()
                    .unwrap(),
            );
            (headers, Html("").into_response())
        }
        None => (
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
    let user = state
        .login_manager
        .lock()
        .await
        .new_user(&email, &password, &confirm_password)
        .await;

    if user.is_ok() {
        header.insert("HX-Redirect", "/login".parse().unwrap());
    }

    let body = match user {
        Ok(_) => "".to_owned(),

        Err(ErrorKind::EmailTaken) => RegisterWidget {
            email_taken: true,
            email_cache: email,
            ..Default::default()
        }
        .render()
        .unwrap(),

        Err(ErrorKind::PasswordMismatch) => RegisterWidget {
            mismatch_passwords: true,
            email_cache: email,
            ..Default::default()
        }
        .render()
        .unwrap(),

        Err(ErrorKind::EmailTakenAndPasswordMismatch) => RegisterWidget {
            email_taken: true,
            email_cache: email,
            mismatch_passwords: true,
        }
        .render()
        .unwrap(),
    };

    (header, Html(body).into_response())
}
