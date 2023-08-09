use askama::Template;
use axum::routing;
use rand::Rng;

#[tokio::main]
async fn main() {
    let app = axum::Router::new()
        .route("/", routing::get(index))
        .route("/chat", routing::get(chat));

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    name: &'a str,
}

async fn index() -> IndexTemplate<'static> {
    IndexTemplate { name: "John Doe" }
}

#[derive(Template)]
#[template(path = "chat.html")]
struct ChatTemplate<'a> {
    msg: &'a str,
}

async fn chat() -> ChatTemplate<'static> {
    let msgs = [
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
    ];
    ChatTemplate {
        msg: &msgs[rand::thread_rng().gen_range(0..msgs.len())],
    }
}
