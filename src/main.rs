#![feature(addr_parse_ascii)]

mod api_client;
mod config;
mod routes;
mod schemas;

use axum::{routing::get, Router};
use std::net::SocketAddr;

use crate::api_client::ApiClient;
use crate::routes::get_models;

#[tokio::main]
async fn main() {
    let config = config::load_config().unwrap();

    let client = ApiClient::new(
        config.openai_api_base_url.clone(),
        config.openai_api_key.clone(),
    );

    let addr =
        SocketAddr::parse_ascii(format!("{}:{}", config.host, config.port).as_bytes()).unwrap();

    tracing_subscriber::fmt::init();
    let app = Router::new()
        .route("/", get(root))
        .route("/v1/models", get(get_models))
        .with_state(client);

    tracing::info!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

pub async fn root() -> &'static str {
    "ProxyPrawn - A simple openai reverse proxy server written in Rust."
}
