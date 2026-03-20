use std::collections::HashMap;
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;

use axum::extract::Path;
use axum::{routing::get, Router};
use samod::storage::TokioFilesystemStorage;
use samod::{ConcurrencyConfig, ConnFinishedReason, DocHandle, DocumentId, Repo, Transport, Url};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use automerge::{ChangeHash};

const BAN_DURATION: std::time::Duration = std::time::Duration::from_secs(600);
const MAX_FAILED_ATTEMPTS: i64 = 50;
const CONNECTION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

mod tracing;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    tracing::initialize_tracing();

    // get home directory
    let data_dir = std::env::var("DATA_DIR").unwrap();
    let storage = TokioFilesystemStorage::new(data_dir);

    let repo_handle = Repo::build_tokio()
        .with_concurrency(ConcurrencyConfig::Threadpool(
            rayon::ThreadPoolBuilder::new().build().unwrap(),
        ))
        .with_storage(storage)
        // .with_announce_policy(move |_, _| {
        //     false
        // })
        .load()
        .await;

    let ip_bans: Arc<Mutex<HashMap<IpAddr, std::time::Instant>>> = Default::default();
    let ip_failed_attempts: Arc<Mutex<HashMap<IpAddr, i64>>> = Default::default();
    // Start the automerge sync server
    let port: String = std::env::var("PORT").unwrap_or_else(|_| "8085".to_string());
    let addr = format!("0.0.0.0:{}", port);
    let acceptor = repo_handle
        .make_acceptor(Url::parse(format!("tcp://{addr}").as_str()).unwrap())
        .unwrap();

    tokio::spawn(async move {
        let listener = TcpListener::bind(&addr).await.unwrap();

        println!("started automerge sync server on localhost:{}", port);

        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    let ip = addr.ip();
                    {
                        let mut ip_bans = ip_bans.lock().await;
                        let banned_at = ip_bans.get(&ip);
                        if banned_at.is_some() {
                            let time_since = std::time::Instant::now() - *banned_at.unwrap();
                            if time_since < BAN_DURATION {
                                println!("Client connection rejected, banned for {} more minutes. IP: {ip}", (BAN_DURATION - time_since).as_secs() / 60);
                                continue;
                            } else {
                                ip_bans.remove(&ip);
                            }
                        }
                    }
                    println!("Client connected. IP: {ip}");
                    let acceptor = acceptor.clone();
                    let ip_bans = ip_bans.clone();
                    let ip_failed_attempts = ip_failed_attempts.clone();
                    // Handle as automerge connection
                    tokio::spawn(async move {
                        let handle_error = || async {
                            let mut ip_bans = ip_bans.lock().await;
                            let mut ip_failed_attempts = ip_failed_attempts.lock().await;
                            let failed_attempts = ip_failed_attempts.get(&ip).cloned().unwrap_or(0);
                            if failed_attempts >= MAX_FAILED_ATTEMPTS {
                                println!(
                                    "Client has been banned for {} minutes. IP: {ip}",
                                    BAN_DURATION.as_secs() / 60
                                );
                                ip_failed_attempts.insert(ip, 0);
                                ip_bans.insert(ip, std::time::Instant::now());
                            } else {
                                ip_failed_attempts.insert(ip, failed_attempts + 1);
                            }
                        };

                        let Ok(connection) = acceptor.accept(Transport::from_tokio_io(socket))
                        else {
                            println!("Error: Acceptor couldn't accept!");
                            return;
                        };

                        // put time-outers in time-out
                        match tokio::time::timeout(
                            CONNECTION_TIMEOUT,
                            connection.handshake_complete(),
                        )
                        .await
                        {
                            // If there was a real error, ban 'em
                            Ok(Err(ConnFinishedReason::ErrorReceiving(message))) => {
                                println!("Client connection error: {message}. IP: {ip}");
                                handle_error().await;
                            }
                            // If we're connected successfully, or if there was a graceful error reason, don't ban 'em
                            Ok(_) => {
                                println!("Client connection completed successfully. IP: {ip}");
                                let mut ip_bans = ip_bans.lock().await;
                                let mut ip_failed_attempts = ip_failed_attempts.lock().await;
                                // reset failed attempts
                                ip_failed_attempts.insert(ip, 0);
                                // remove from banned list
                                ip_bans.remove(&ip);
                            }
                            // If we timed out, ban 'em
                            Err(_) => {
                                println!("Client connection timed out. IP: {ip}");
                                handle_error().await;
                            }
                        }
                    });
                }
                Err(e) => println!("couldn't get client: {:?}", e),
            }
        }
    });

    let repo_handle_clone = repo_handle.clone();
    let repo_handle_clone2 = repo_handle.clone();

    // Start the HTTP server
    let app = Router::new()
        .route(
            "/doc/{id}",
            get(|Path(id): Path<String>| async move {
                println!("Received request for document ID: {}", id);
                match DocumentId::from_str(&id) {
                    Ok(document_id) => {
                        println!("Successfully parsed document ID");
                        match repo_handle_clone.find(document_id).await {
                            Ok(Some(doc_handle)) => {
                                println!("Successfully retrieved document");
                                doc_to_string_full(&doc_handle)
                            }
                            Ok(None) => {
                                let errstr = "Error retrieving document: Not found!".to_string();
                                println!("{}", errstr);
                                errstr
                            }
                            Err(_) => {
                                let errstr = "Error retrieving document: Repo stopped!".to_string();
                                println!("{}", errstr);
                                errstr
                            }
                        }
                    }
                    Err(e) => {
                        println!("Error parsing document ID: {:?}", e);
                        format!("Error parsing document ID: {:?}", e)
                    }
                }
            }),
        )
        .route(
            "/last_heads/{id}",
            get(|Path(id): Path<String>| async move {
                println!("Received request for last heads of document ID: {}", id);
                match DocumentId::from_str(&id) {
                    Ok(document_id) => {
                        println!("Successfully parsed document ID");
                        match repo_handle_clone2.find(document_id).await {
                            Ok(Some(doc_handle)) => {
                                println!("Successfully retrieved document");
                                let mut heads: Vec<ChangeHash> = Vec::new();
                                doc_handle.with_document(|d| {
                                    heads = d.get_heads();
                                });
                                serde_json::to_string(&heads).unwrap().to_string()
                            }
                            Ok(None) => {
                                let errstr = "Error retrieving document: Not found!".to_string();
                                println!("{}", errstr);
                                format!("<error>{}</error>", errstr).to_string()
                            }
                            Err(_) => {
                                let errstr = "Error retrieving document: Repo stopped!".to_string();
                                println!("{}", errstr);
                                format!("<error>{}</error>", errstr).to_string()
                            }
                        }
                    }
                    Err(e) => {
                        println!("Error parsing document ID: {:?}", e);
                        format!("Error parsing document ID: {:?}", e)
                    }
                }
            }),
        )
        .route("/", get(|| async { "fetch documents with /doc/{id}" }))
        .layer(CorsLayer::permissive());

    let http_port = std::env::var("HTTP_PORT").unwrap_or_else(|_| "80".to_string());
    let http_addr = format!("0.0.0.0:{}", http_port);
    println!("starting HTTP server on {}", http_addr);

    let listener = tokio::net::TcpListener::bind(http_addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    tokio::signal::ctrl_c().await.unwrap();

    repo_handle.stop().await;
}

fn doc_to_string_full(doc_handle: &DocHandle) -> String {
    let checked_out_doc_json = doc_handle
        .with_document(|d| serde_json::to_string(&automerge::AutoSerde::from(&*d)).unwrap());

    checked_out_doc_json.to_string()
}

#[allow(dead_code)]
fn doc_to_string(doc_handle: &DocHandle) -> String {
    let json_value = doc_handle.with_document(|d| {
        let auto_serde = automerge::AutoSerde::from(&*d);
        serde_json::to_value(&auto_serde).unwrap()
    });

    fn truncate_long_strings(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                for (_, v) in map.iter_mut() {
                    truncate_long_strings(v);
                }
            }
            serde_json::Value::Array(arr) => {
                for v in arr.iter_mut() {
                    truncate_long_strings(v);
                }
            }
            serde_json::Value::String(s) => {
                if s.len() > 50 {
                    *s = format!("{}...", &s[..47]);
                }
            }
            _ => {}
        }
    }

    let mut json_value = json_value;
    truncate_long_strings(&mut json_value);
    serde_json::to_string_pretty(&json_value).unwrap()
}
