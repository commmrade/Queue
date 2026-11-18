mod auth;
mod db;
mod error;
mod handlers;
mod models;
mod state;

use state::AppState;
use std::env;

#[tokio::main]
async fn main() {
    let state = AppState::connect().await;
    let port = env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(3000);
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await
        .expect("could not bind API port");

    println!("Queue API listening on 0.0.0.0:{port}");
    axum::serve(listener, handlers::router(state))
        .await
        .expect("API server failed");
}
