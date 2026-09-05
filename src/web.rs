use std::{collections::HashMap, net::SocketAddr, str::FromStr, sync::Arc};

use automerge::{ChangeHash, ReadDoc};
use axum::{
    extract::{ConnectInfo, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{TimeZone, Utc};
use sedimentree_core::{hex, id::SedimentreeId};
use serde::Serialize;
use thiserror::Error;
use tokio::sync::Semaphore;

use crate::{
    config::{Authentication, CommandConfig},
    repo::RepoError,
    sync::{SocketInfo, SyncServer},
    web::tungstenite_ws_extract::{WebSocket, WebSocketUpgrade},
};

pub mod tungstenite_ws_extract;

#[derive(Clone)]
pub struct WebEndpointState {
    pub server: SyncServer,
    pub semaphore: Arc<Semaphore>,
    pub config: Arc<CommandConfig>,
}

#[derive(Debug, Error)]
pub enum WebError {
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("not found: {0}")]
    NotFound(String),

    #[error("the document ID was malformed: {0}")]
    BadDocumentId(String),
    #[error("an error occurred during serialization: {0}")]
    Serde(#[from] serde_json::Error),

    #[error(transparent)]
    Repo(#[from] RepoError),

    // TODO (subd): REMOVE
    #[error("the endpoint is not implemented")]
    NotImplemented,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        // this is really awkward, but push it to the console
        tracing::error!("Error into response: {self}");

        let status = match &self {
            WebError::BadRequest(_) => StatusCode::BAD_REQUEST,
            WebError::NotFound(_) => StatusCode::NOT_FOUND,
            WebError::BadDocumentId(_) => StatusCode::BAD_REQUEST,
            WebError::Serde(_) => StatusCode::INTERNAL_SERVER_ERROR,
            WebError::NotImplemented => StatusCode::NOT_FOUND,
            WebError::Repo(repo_error) => match repo_error {
                RepoError::NoSuchDocument(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            },
        };

        let message = self.to_string();

        (status, Json(ErrorBody { error: message })).into_response()
    }
}

#[derive(Serialize)]
pub struct Change {
    author: String,
    date: String,
    message: serde_json::Value,
}

fn parse_change_hashes(input: &str) -> Result<Vec<ChangeHash>, WebError> {
    let mut hashes = Vec::new();

    for token in input.split(',') {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            continue;
        }
        let hash = ChangeHash::from_str(trimmed)
            .map_err(|e| WebError::BadRequest(format!("invalid hash '{}': {:?}", trimmed, e)))?;
        hashes.push(hash);
    }

    if hashes.is_empty() {
        return Err(WebError::BadRequest(
            "no change hashes provided".to_string(),
        ));
    }

    Ok(hashes)
}

fn hydrate_value_to_json(value: &automerge::hydrate::Value) -> serde_json::Value {
    match value {
        automerge::hydrate::Value::Scalar(scalar) => {
            serde_json::to_value(scalar).unwrap_or(serde_json::Value::Null)
        }
        automerge::hydrate::Value::Map(map) => {
            let mut out = serde_json::Map::new();
            for (key, map_value) in map.iter() {
                out.insert(key.clone(), hydrate_value_to_json(&map_value.value));
            }
            serde_json::Value::Object(out)
        }
        automerge::hydrate::Value::List(list) => serde_json::Value::Array(
            list.iter()
                .map(|list_value| hydrate_value_to_json(&list_value.value))
                .collect(),
        ),
        automerge::hydrate::Value::Text(text) => serde_json::Value::String(text.to_string()),
    }
}

#[derive(Serialize)]
pub struct ServerDescription {
    version: String,
    minimum_backstitch_version: String,
    sync: String,
    webviewer: Option<String>,
    auth: String,
    // This has to be a string, because the Url crate likes to add a bad trailing slash.
    oidc_issuer: Option<String>,
    oidc_client_id: Option<String>,
    oidc_redirect_port: Option<u16>,
    // TODO: Remove this once Endless implements RFC 9728
    oidc_resource: Option<String>,
}

pub async fn describe(
    State(state): State<WebEndpointState>,
) -> Result<Json<ServerDescription>, WebError> {
    let _permit = state.semaphore.acquire().await.unwrap();
    tracing::info!("Received request to describe");
    let auth = state.config.authentication();
    Ok(Json(ServerDescription {
        version: env!("CARGO_PKG_VERSION").to_string(),
        webviewer: state.config.webviewer.clone(),
        minimum_backstitch_version: "3.0.0".to_string(),
        sync: "sync".to_string(),
        auth: match &auth {
            Authentication::None => "none".to_string(),
            Authentication::Oidc(_) => "oidc".to_string(),
        },
        oidc_client_id: match &auth {
            Authentication::None => None,
            Authentication::Oidc(oidc_config) => Some(oidc_config.client_id.clone()),
        },
        oidc_redirect_port: match &auth {
            Authentication::None => None,
            Authentication::Oidc(oidc_config) => Some(oidc_config.redirect_port),
        },
        oidc_issuer: match &auth {
            Authentication::None => None,
            Authentication::Oidc(oidc_config) => Some(oidc_config.issuer.clone()),
        },
        oidc_resource: match &auth {
            Authentication::None => None,
            Authentication::Oidc(oidc_config) => oidc_config.resource.clone(),
        },
    }))
}

pub async fn sync(
    // Use a custom extractor for this, instead of tokio::ws default.
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<WebEndpointState>,
) -> Response {
    tracing::info!("Received request to sync");
    ws.on_failed_upgrade(move |e| tracing::error!("Failed websocket upgrade for {addr}: {e}"))
        .on_upgrade(async move |socket: WebSocket| {
            tracing::info!("Upgrade successful for {}", addr.ip());
            state.server.accept_socket(SocketInfo(addr, socket)).await;
        })
}

pub async fn doc(
    Path(id): Path<String>,
    State(state): State<WebEndpointState>,
) -> Result<Json<serde_json::Value>, WebError> {
    Err(WebError::NotImplemented)
    // let _permit = state.semaphore.acquire().await.unwrap();
    // tracing::info!("Received request for document ID: {}", id);
    // let doc_handle = get_handle(&id, &state).await?;
    // let checked_out_doc_json =
    //     doc_handle.with_document(|d| serde_json::to_value(automerge::AutoSerde::from(&*d)))?;

    // Ok(Json(checked_out_doc_json))
}

pub async fn doc_at(
    Path((id, change_hash)): Path<(String, String)>,
    State(state): State<WebEndpointState>,
) -> Result<Json<serde_json::Value>, WebError> {
    Err(WebError::NotImplemented)
    // let _permit = state.semaphore.acquire().await.unwrap();
    // tracing::info!("Received request for document ID: {id} at change hash: {change_hash}");
    // let id = SedimentreeId::from_str(&id)?;

    // let change_hashes = parse_change_hashes(&change_hash)?;
    // let d = state.server.repo().get_document(id).await?;
    // let doc_at_heads =
    //     ReadDoc::hydrate(&d, automerge::ROOT, Some(&change_hashes)).map_err(|e| {
    //         WebError::BadRequest(format!(
    //             "Error getting document at change hashes {change_hashes:?} ({e})"
    //         ))
    //     })?;

    // Ok(Json(hydrate_value_to_json(&doc_at_heads)))
}

pub async fn last_heads(
    Path(id): Path<String>,
    State(state): State<WebEndpointState>,
) -> Result<Json<Vec<ChangeHash>>, WebError> {
    Err(WebError::NotImplemented)
    // let _permit = state.semaphore.acquire().await.unwrap();
    // tracing::info!("Received request for last heads of document ID: {}", id);
    // let doc_handle = get_handle(&id, &state).await?;

    // Ok(Json(doc_handle.with_document(|d| d.get_heads())))
}

pub async fn list_changes(
    Path(id): Path<String>,
    State(state): State<WebEndpointState>,
) -> Result<Json<HashMap<String, Change>>, WebError> {
    Err(WebError::NotImplemented)
    // let _permit = state.semaphore.acquire().await.unwrap();
    // tracing::info!("Received request for changes list of document ID: {id}");
    // let doc_handle = get_handle(&id, &state).await?;

    // Ok(Json(doc_handle.with_document(|d| {
    //     let changes = d.get_changes(&[]);
    //     let mut out = HashMap::new();

    //     for change in changes {
    //         let hash = change.hash().to_string();
    //         let date = match Utc.timestamp_opt(change.timestamp(), 0).single() {
    //             Some(dt) => dt.to_rfc3339(),
    //             None => change.timestamp().to_string(),
    //         };
    //         let message = match change.message() {
    //             Some(raw_message) => serde_json::from_str::<serde_json::Value>(raw_message)
    //                 .unwrap_or_else(|_| serde_json::Value::String(raw_message.clone())),
    //             None => serde_json::Value::Null,
    //         };

    //         out.insert(
    //             hash,
    //             Change {
    //                 author: change.actor_id().to_string(),
    //                 date,
    //                 message,
    //             },
    //         );
    //     }
    //     out
    // })))
}
