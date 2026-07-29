use std::sync::Arc;

use axum::{routing::get, Router};
use tokio::sync::Semaphore;
use tower_http::cors::CorsLayer;

use crate::web::WebEndpointState;

mod bans;
mod sync;
mod tracing;
mod web;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    tracing::initialize_tracing();

    let web_semaphore = Arc::new(Semaphore::new(100));

    // ends on drop
    let sync_server = sync::SyncServer::new().await;

    let state = WebEndpointState {
        repo: sync_server.repo(),
        semaphore: web_semaphore.clone(),
    };

    // Start the HTTP server
    let app = Router::new()
        .route("/doc/{id}", get(web::doc))
        .route("/last_heads/{id}", get(web::last_heads))
        // route to get the doc at a certain change hash
        .route("/doc_at/{id}/{change_hash}", get(web::doc_at))
        .route("/list_changes/{id}", get(web::list_changes))
        .route("/", get(|| async { "fetch documents with /doc/{id}" }))
        .with_state(state)
        .layer(CorsLayer::permissive());

    let http_port = std::env::var("HTTP_PORT").unwrap_or_else(|_| "80".to_string());
    let http_addr = format!("0.0.0.0:{}", http_port);
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
