use std::{collections::HashMap, str::FromStr, sync::Arc};

use automerge::{ChangeHash, ReadDoc};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{TimeZone, Utc};
use samod::{DocHandle, DocumentId, Repo};
use serde::Serialize;
use thiserror::Error;
use tokio::sync::Semaphore;

#[derive(Clone)]
pub struct WebEndpointState {
    pub repo: Repo,
    pub semaphore: Arc<Semaphore>,
}

#[derive(Debug, Error)]
pub enum WebError {
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("not found: {0}")]
    NotFound(String),

    #[error("the document ID was malformed: {0}")]
    BadDocumentId(#[from] samod::BadDocumentId),
    #[error("the repo was stopped")]
    Stopped(#[from] samod::Stopped),
    #[error("an error occurred during serialization: {0}")]
    Serde(#[from] serde_json::Error),
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        // this is really awkward, but push it to the console
        tracing::error!("Error into response: {self}");

        let status = match self {
            WebError::BadRequest(_) => StatusCode::BAD_REQUEST,
            WebError::NotFound(_) => StatusCode::NOT_FOUND,
            WebError::BadDocumentId(_) => StatusCode::BAD_REQUEST,
            WebError::Stopped(_) => StatusCode::INTERNAL_SERVER_ERROR,
            WebError::Serde(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let message = self.to_string();

        (status, Json(ErrorBody { error: message })).into_response()
    }
}

#[derive(Serialize)]
pub struct Change {
    author: String,
    date: String,
    message: Option<String>,
}

async fn get_handle(id: &str, state: &WebEndpointState) -> Result<DocHandle, WebError> {
    state
        .repo
        .find(DocumentId::from_str(&id)?)
        .await?
        .ok_or(WebError::NotFound("document ID doesn't exist".to_string()))
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

pub async fn doc(
    Path(id): Path<String>,
    State(state): State<WebEndpointState>,
) -> Result<Json<serde_json::Value>, WebError> {
    tracing::info!("Received request for document ID: {}", id);
    let _permit = state.semaphore.acquire().await.unwrap();
    let doc_handle = get_handle(&id, &state).await?;
    let checked_out_doc_json =
        doc_handle.with_document(|d| serde_json::to_value(&automerge::AutoSerde::from(&*d)))?;

    Ok(Json(checked_out_doc_json))
}

pub async fn doc_at(
    Path((id, change_hash)): Path<(String, String)>,
    State(state): State<WebEndpointState>,
) -> Result<Json<serde_json::Value>, WebError> {
    tracing::info!("Received request for document ID: {id} at change hash: {change_hash}");
    let _permit = state.semaphore.acquire().await.unwrap();
    let doc_handle = get_handle(&id, &state).await?;

    let change_hashes = parse_change_hashes(&change_hash)?;
    let doc_at_heads = doc_handle.with_document(move |d| {
        ReadDoc::hydrate(&*d, automerge::ROOT, Some(&change_hashes)).map_err(|e| {
            WebError::BadRequest(format!(
                "Error getting document at change hashes {change_hashes:?} ({e})"
            ))
        })
    })?;

    Ok(Json(hydrate_value_to_json(&doc_at_heads)))
}

pub async fn last_heads(
    Path(id): Path<String>,
    State(state): State<WebEndpointState>,
) -> Result<Json<Vec<ChangeHash>>, WebError> {
    println!("Received request for last heads of document ID: {}", id);
    let _permit = state.semaphore.acquire().await.unwrap();
    let doc_handle = get_handle(&id, &state).await?;

    Ok(Json(doc_handle.with_document(|d| d.get_heads())))
}

pub async fn list_changes(
    Path(id): Path<String>,
    State(state): State<WebEndpointState>,
) -> Result<Json<HashMap<String, Change>>, WebError> {
    tracing::info!("Received request for changes list of document ID: {id}");
    let _permit = state.semaphore.acquire().await.unwrap();
    let doc_handle = get_handle(&id, &state).await?;

    Ok(Json(doc_handle.with_document(|d| {
        let changes = d.get_changes(&[]);
        let mut out = HashMap::new();

        for change in changes {
            let hash = change.hash().to_string();
            let date = match Utc.timestamp_opt(change.timestamp(), 0).single() {
                Some(dt) => dt.to_rfc3339(),
                None => change.timestamp().to_string(),
            };

            out.insert(
                hash,
                Change {
                    author: change.actor_id().to_string(),
                    date: date,
                    message: change.message().cloned(),
                },
            );
        }
        out
    })))
}
