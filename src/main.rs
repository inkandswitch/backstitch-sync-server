use std::sync::Arc;

use axum::{routing::get, Router};
use clap::Parser;
use tokio::sync::Semaphore;
use tower_http::cors::CorsLayer;

use crate::{config::CommandConfig, web::WebEndpointState};

mod bans;
mod config;
mod sync;
mod tracing;
mod web;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let config = CommandConfig::parse();
    tracing::initialize_tracing();

    let web_semaphore = Arc::new(Semaphore::new(100));

    // ends on drop
    let sync_server = sync::SyncServer::new(config.sync_port, &config.data_dir).await;

    let state = WebEndpointState {
        repo: sync_server.repo(),
        semaphore: web_semaphore.clone(),
        config: Arc::new(config.clone()),
    };

    // Start the HTTP server
    let app = Router::new()
        .route("/doc/{id}", get(web::doc))
        .route("/last_heads/{id}", get(web::last_heads))
        // route to get the doc at a certain change hash
        .route("/doc_at/{id}/{change_hash}", get(web::doc_at))
        .route("/list_changes/{id}", get(web::list_changes))
        .route("/", get(|| async { "fetch documents with /doc/{id}" }))
        .route("/describe", get(web::describe))
        .with_state(state)
        .layer(CorsLayer::permissive());

    let http_addr = format!("0.0.0.0:{}", config.http_port);
    println!("starting HTTP server on {}", http_addr);

    let listener = tokio::net::TcpListener::bind(http_addr).await.unwrap();
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.unwrap();
        })
        .await
        .unwrap();

    tokio::signal::ctrl_c().await.unwrap();
    drop(sync_server);
}
