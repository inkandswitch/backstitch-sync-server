use std::{net::SocketAddr, sync::Arc};

use axum::{
    routing::{any, get},
    Router,
};
use clap::Parser;
use jwt_authorizer::{Authorizer, IntoLayer, JwtAuthorizer, Validation};
use reqwest::Client;
use tokio::sync::Semaphore;
use tower_http::cors::CorsLayer;

use crate::{
    config::{Authentication, CommandConfig},
    web::WebEndpointState,
};

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

    let sync_server = sync::SyncServer::new(&config.data_dir).await;

    let state = WebEndpointState {
        server: sync_server.clone(),
        semaphore: web_semaphore.clone(),
        config: Arc::new(config.clone()),
    };

    let public_routes = Router::new()
        // TODO: make this the webviewer
        .route("/", get(|| async { "fetch documents with /doc/{id}" }))
        .route("/describe", get(web::describe));

    let mut web_routes = Router::new()
        .route("/doc/{id}", get(web::doc))
        .route("/last_heads/{id}", get(web::last_heads))
        .route("/doc_at/{id}/{change_hash}", get(web::doc_at))
        .route("/list_changes/{id}", get(web::list_changes));

    let mut sync_routes = Router::new().route("/sync", any(web::sync));

    let auth = config.authentication();
    if let Authentication::Oidc(oidc_auth) = auth {
        let http_client = Client::builder()
            .tls_danger_accept_invalid_certs(config.accept_invalid_certs)
            .build()
            .unwrap();
        let auth: Authorizer = JwtAuthorizer::from_oidc(&oidc_auth.issuer.to_string())
            .http_client(http_client)
            .validation(
                Validation::new()
                    .aud(&[oidc_auth.client_id])
                    .iss(&[oidc_auth.issuer.to_string()])
                    .exp(true)
                    .nbf(true)
                    .leeway(20),
            )
            .build()
            .await
            .unwrap();
        let layer = auth.into_layer();
        sync_routes = sync_routes.layer(layer.clone());

        if config.webviewer_endpoint_auth {
            web_routes = web_routes.layer(layer);
        }
    }

    // Start the HTTP server
    let app = Router::new()
        .merge(public_routes)
        .merge(web_routes)
        .merge(sync_routes)
        .with_state(state)
        .layer(CorsLayer::permissive());

    let addr = format!("0.0.0.0:{}", config.port);
    ::tracing::info!("starting HTTP server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        tokio::signal::ctrl_c().await.unwrap();
        sync_server.shutdown().await;
    })
    .await
    .unwrap();
}
