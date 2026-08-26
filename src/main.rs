use std::{net::SocketAddr, sync::Arc};

use axum::{
    routing::{any, get},
    Router,
};
use clap::Parser;
use jwt_authorizer::{Authorizer, IntoLayer, JwtAuthorizer, Validation};
use reqwest::Client;
use tokio::sync::Semaphore;
use tower_http::{cors::CorsLayer, services::ServeDir};

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

    if !config.no_webviewer_auth
        && config.webviewer_path.is_some()
        && matches!(config.authentication(), Authentication::Oidc(_))
    {
        ::tracing::error!("Currently the Webviewer does not support OpenID Connect authentication. It will be inaccessible. \
            To disable authentication on the webviewer, set webviewer_endpoint_auth to false.");
    }

    let web_semaphore = Arc::new(Semaphore::new(100));

    let sync_server = sync::SyncServer::new(&config.data_dir).await;

    let state = WebEndpointState {
        server: sync_server.clone(),
        semaphore: web_semaphore.clone(),
        config: Arc::new(config.clone()),
    };

    let mut public_routes = Router::new().route("/describe", get(web::describe));

    if let Some(path) = &config.webviewer_path {
        public_routes = public_routes.fallback_service(ServeDir::new(path));
    } else {
        public_routes = public_routes.route("/", get(|| async { "no webviewer provided" }));
    }

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
        let mut aud = vec![oidc_auth.client_id];
        if let Some(resource) = oidc_auth.resource {
            aud.push(resource);
        }
        let auth: Authorizer = JwtAuthorizer::from_oidc(&oidc_auth.issuer.to_string())
            .http_client(http_client)
            .validation(
                Validation::new()
                    .aud(&aud)
                    .iss(std::slice::from_ref(&oidc_auth.issuer))
                    .exp(true)
                    .nbf(true)
                    .leeway(20),
            )
            .build()
            .await
            .unwrap();
        let layer = auth.into_layer();
        sync_routes = sync_routes.layer(layer.clone());

        if !config.no_webviewer_auth {
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
