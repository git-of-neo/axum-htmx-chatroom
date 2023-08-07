use askama::Template;
use axum::routing;

#[derive(Template)]
#[template(path = "index.html")]
#[allow(dead_code)]
struct IndexTemplate<'a> {
    name: &'a str,
}

async fn index() -> IndexTemplate<'static> {
    IndexTemplate { name: "John Doe" }
}

#[tokio::main]
async fn main() {
    let app = axum::Router::new().route("/", routing::get(index));
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
