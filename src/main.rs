use futures::{
    sink::SinkExt,
    stream::{SplitSink, SplitStream, StreamExt},
};
use std::{
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
    http::StatusCode,
    routing, Error, Form,
};
use serde::Deserialize;
use tokio::sync::broadcast;

struct AppState {
    msgs: Mutex<Vec<String>>,
    tx: broadcast::Sender<String>,
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
    });

    let app = axum::Router::new()
        .route("/", routing::get(index))
        .route("/chat", routing::get(chat))
        .route("/chat", routing::post(create_chat))
        .route("/ws", routing::get(ws_handler))
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
#[template(path = "placeholder.html")]
struct PlaceHolderTemplate {}

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
            state.msgs.lock().unwrap().push(payload.chat_message);
            let _ = state.tx.send(PlaceHolderTemplate {}.render().unwrap());
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

async fn chat<'a>(State(state): State<Arc<AppState>>) -> ChatTemplate {
    let msgs = state.msgs.lock().unwrap();
    ChatTemplate {
        msgs: msgs.to_vec(),
    }
}

#[derive(Deserialize)]
struct NewChat {
    message: String,
}

async fn create_chat(
    State(state): State<Arc<AppState>>,
    Form(chat): Form<NewChat>,
) -> (StatusCode, ()) {
    let mut msgs = state.msgs.lock().unwrap();
    msgs.push(chat.message.to_owned());
    (StatusCode::CREATED, ())
}
