use std::{
    net::{IpAddr, SocketAddr},
    path::Path,
    sync::Arc,
};

use axum::extract::ws::WebSocket;
use samod::{
    storage::TokioFilesystemStorage, AcceptorHandle, ConcurrencyConfig, ConnFinishedReason,
    NeverAnnounce, Repo, Url,
};
use tokio::{
    select,
    sync::{mpsc, OwnedSemaphorePermit, Semaphore},
};
use tokio_util::sync::CancellationToken;

use crate::bans::IpBans;

const BAN_DURATION: std::time::Duration = std::time::Duration::from_secs(600);
const MAX_FAILED_ATTEMPTS: i64 = 50;
const CONNECTION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const MAX_CONNECTIONS: usize = 500;

#[derive(Clone)]
pub struct SyncServer {
    repo: Repo,
    bans: IpBans,
    token: CancellationToken,
    semaphore: Arc<Semaphore>,
    sockets_tx: tokio::sync::mpsc::Sender<SocketInfo>,
}

pub struct SocketInfo(pub SocketAddr, pub WebSocket);

impl SyncServer {
    pub async fn new(data_dir: &Path) -> Self {
        // get home directory
        let storage = TokioFilesystemStorage::new(data_dir);

        let repo = Repo::build_tokio()
            .with_concurrency(ConcurrencyConfig::Threadpool(
                rayon::ThreadPoolBuilder::new().build().unwrap(),
            ))
            .with_storage(storage)
            .with_announce_policy(NeverAnnounce)
            .load()
            .await;

        let (sockets_tx, sockets_rx) = mpsc::channel(MAX_CONNECTIONS * 2);

        let this = Self {
            repo,
            bans: IpBans::new(BAN_DURATION, MAX_FAILED_ATTEMPTS),
            token: CancellationToken::new(),
            semaphore: Arc::new(Semaphore::new(MAX_CONNECTIONS)),
            sockets_tx,
        };

        {
            let this = this.clone();
            tokio::spawn(async move { this.server_loop(sockets_rx).await });
        }
        this
    }

    pub fn repo(&self) -> Repo {
        self.repo.clone()
    }

    pub async fn accept_socket(&self, info: SocketInfo) {
        let _ = self.sockets_tx.send(info).await;
    }

    pub async fn shutdown(&self) {
        self.token.cancel();
        self.semaphore.close();
    }

    async fn server_loop(&self, mut sockets_rx: mpsc::Receiver<SocketInfo>) {
        // Start the automerge sync server
        // This URL does nothing except participate in logs, since we're accepting websockets
        // from the HTTP server in the end.
        let acceptor = self
            .repo
            .make_acceptor(Url::parse("ws://0.0.0.0:8080").unwrap())
            .unwrap();

        tracing::info!("started automerge sync server...");

        loop {
            select! {
                _ = self.token.cancelled() => break,
                info = sockets_rx.recv() => {
                    let Some(SocketInfo(addr, socket)) = info else {
                        break;
                    };

                    let ip = addr.ip();
                    if self.bans.is_banned(&ip).await {
                        continue;
                    }

                    let Ok(permit) = self.semaphore.clone().acquire_owned().await else {
                        break;
                    };
                    tracing::info!("Client connected. IP: {ip}");
                    let acceptor = acceptor.clone();
                    // Handle as automerge connection
                    let this = self.clone();
                    tokio::spawn(async move {
                        select! {
                            _ = this.handle_connection(ip, acceptor, socket, permit) => {}
                            _ = this.token.cancelled() => {}
                        }
                    });
                }
            }
        }

        self.repo.stop().await;
    }

    async fn handle_connection(
        &self,
        ip: IpAddr,
        acceptor: AcceptorHandle,
        socket: WebSocket,
        _permit: OwnedSemaphorePermit,
    ) {
        let connection = match acceptor.accept_axum(socket) {
            Ok(connection) => connection,
            Err(e) => {
                tracing::error!("Error: Acceptor couldn't accept! {e}");
                return;
            }
        };

        // put time-outers in time-out
        match tokio::time::timeout(CONNECTION_TIMEOUT, connection.handshake_complete()).await {
            // If there was a real error, ban 'em
            Ok(Err(ConnFinishedReason::ErrorReceiving(message))) => {
                tracing::error!("Client connection error: {message}. IP: {ip}");
                self.bans.ban(&ip).await;
            }
            // If we're connected successfully, or if there was a graceful error reason, don't ban 'em
            Ok(_) => {
                tracing::info!("Client connection completed successfully. IP: {ip}");
                self.bans.unban(&ip).await;
            }
            // If we timed out, ban 'em
            Err(_) => {
                tracing::warn!("Client connection timed out. IP: {ip}");
                self.bans.ban(&ip).await;
            }
        }
    }
}
