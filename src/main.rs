#![feature(addr_parse_ascii)]

mod config;
mod routes;
mod schemas;

use axum::{routing::get, Router};
use std::net::SocketAddr;

use routes::get_models;

#[tokio::main]
async fn main() {
    let config = config::load_config().unwrap();
    let addr =
        SocketAddr::parse_ascii(format!("{}:{}", config.host, config.port).as_bytes()).unwrap();

    tracing_subscriber::fmt::init();
    let app = Router::new()
        .route("/", get(root))
        .route("/v1/models", get(get_models))
        .with_state(config);

    tracing::info!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

pub async fn root() -> &'static str {
    "ProxyPrawn - A simple openai reverse proxy server written in Rust."
}
