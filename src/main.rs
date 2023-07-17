mod config;
mod schemas;

use axum::{extract::State, routing::get, Json, Router};
use config::AppConfig;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use schemas::Model;

#[tokio::main]
async fn main() {
    let config = config::load_config().unwrap();

    tracing_subscriber::fmt::init();
    let app = Router::new()
        .route("/", get(root))
        .route("/v1/models", get(get_models))
        .with_state(config);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::info!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn root() -> &'static str {
    "ProxyPrawn - A simple openai reverse proxy server written in Rust."
}

#[derive(Debug, Serialize, Deserialize)]
struct ModelsResponse {
    object: String,
    data: Vec<Model>,
}

async fn get_models(State(config): State<AppConfig>) -> Json<Vec<Model>> {
    let client = reqwest::Client::new();

    println!("API Key: {}", config.openai_api_key);

    let response: ModelsResponse = client
        .get("https://api.openai.com/v1/models")
        .header("Authorization", format!("Bearer {}", config.openai_api_key))
        .header("Content-Type", "application/json")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    Json(response.data)
}
