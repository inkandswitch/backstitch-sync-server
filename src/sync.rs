use std::{
    net::{IpAddr, SocketAddr},
    path::Path,
    sync::Arc,
};

use async_tungstenite::{tokio::TokioAdapter, WebSocketStream};
use future_form::{FutureForm, Sendable};
use futures::FutureExt;
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use subduction_core::{
    handshake::{self, audience::DiscoveryId, AuthenticateError},
    subduction::error::AddConnectionError,
    timestamp::TimestampSeconds,
    transport::message::MessageTransport,
};
use subduction_crypto::nonce::Nonce;
use subduction_websocket::{
    handshake::{WebSocketHandshake, WebSocketHandshakeError},
    sleep,
    websocket::{KeepAlive, KeepAliveOutcome, ListenerTask, SenderTask, WebSocket},
};
use thiserror::Error;
use tokio::{
    select,
    sync::{mpsc, OwnedSemaphorePermit, Semaphore},
};
use tokio_util::sync::CancellationToken;

use crate::{
    bans::IpBans,
    keys::SigningKey,
    repo::{Repo, RepoError},
};

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

#[derive(Error, Debug)]
enum ConnectionError {
    #[error("authenticate failed: {0}")]
    Authenticate(#[from] AuthenticateError<WebSocketHandshakeError>),
    #[error("handshake timed out")]
    Timeout(#[from] tokio::time::error::Elapsed),
    #[error("connection disallowed: {0}")]
    AddConnection(#[from] AddConnectionError<!>),
}

type Wss = WebSocketStream<TokioAdapter<TokioIo<Upgraded>>>;

pub struct SocketInfo(pub SocketAddr, pub Wss);

impl SyncServer {
    pub async fn new(data_dir: &Path, signing_key: SigningKey) -> Result<Self, RepoError> {
        let repo = Repo::new(data_dir, signing_key)?;

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
        Ok(this)
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
        // As we receive sockets from accept_socket(), this will process them.
        tracing::info!("started automerge sync server...");

        loop {
            select! {
                _ = self.token.cancelled() => break,
                info = sockets_rx.recv() => {
                    let Some(SocketInfo(addr, socket)) = info else {
                        break;
                    };

                    let ip = addr.ip();
                    // TODO (subd): Reimplement banning?
                    if self.bans.is_banned(&ip).await {
                        tracing::error!("IP {ip} is banned.");
                        continue;
                    }

                    let Ok(permit) = self.semaphore.clone().acquire_owned().await else {
                        break;
                    };
                    tracing::info!("Client connected. IP: {ip}");

                    let this = self.clone();
                    tokio::spawn(async move {
                        select! {
                            _ = this.token.cancelled() => {
                            }
                            res = this.handle_connection(ip, socket, permit) => {
                                match res {
                                    Ok(()) => {
                                        // Graceful disconnect, so we get to unban!
                                        this.bans.unban(&ip).await;
                                    },
                                    Err(e) => {
                                        tracing::error!("Error with {ip}: {e}. Incrementing ban counter...");
                                        this.bans.ban(&ip).await;
                                    },
                                }
                            }
                        }
                    });
                }
            }
        }

        self.repo.stop();
    }

    async fn handle_connection(
        &self,
        ip: IpAddr,
        socket: Wss,
        _permit: OwnedSemaphorePermit,
    ) -> Result<(), ConnectionError> {
        // Do the handshake!
        let now = TimestampSeconds::now();
        let nonce = Nonce::random();
        let subd = self.repo.subduction();
        let handshake_fut = handshake::initiate::<Sendable, _, _, _, _>(
            WebSocketHandshake::new(socket),
            |ws_handshake, peer_id| {
                let (socket, sender_fut, keepalive_task) = WebSocket::new_with_keepalive(
                    ws_handshake.into_inner(),
                    peer_id,
                    KeepAlive::balanced(),
                    sleep::TokioSleeper,
                );
                (
                    MessageTransport::new(socket),
                    (Sendable::from_future(sender_fut), keepalive_task),
                )
            },
            subd.signer(),
            handshake::audience::Audience::Discover(DiscoveryId::new(
                "backstitch_sync_server".as_bytes(),
            )),
            now,
            nonce,
        );

        let (authenticated, (sender_fut, keepalive_task)) =
            tokio::time::timeout(CONNECTION_TIMEOUT, handshake_fut).await??;

        let socket = authenticated.inner().clone();
        let listener_fut = socket.inner().listen();
        tracing::info!("Handshake completed for {ip}!");

        let _fresh = self.repo.subduction().add_connection(authenticated).await?;

        // TODO (subd): Are these cancellation-safe?
        select! {
            _ = self.token.cancelled() => {}
            res = keepalive_task => match res {
                KeepAliveOutcome::ConnectionClosed => {
                    tracing::info!("Connection to {ip} closed.");
                },
                KeepAliveOutcome::Timeout { missed } => {
                    tracing::warn!("Keepalive timed out for {ip}; missed: {missed}");
                },
                KeepAliveOutcome::StaleNoPong { unanswered } => {
                    tracing::warn!("Keepalive stale for {ip}; unanswered: {unanswered}");
                },
            },
            res = listener_fut => match res {
                Ok(()) => tracing::info!("Connection to {ip} finished."),
                Err(e) => {
                    tracing::error!("Error sending: {e}");
                }
            },
            res = sender_fut => match res {
                Ok(()) => tracing::info!("Connection to {ip} finished."),
                Err(e) => {
                    // I don't think we want to increment the ban counter here
                    // Since they successfully handshaked we know they're legit
                    tracing::error!("Error sending: {e}");
                },
            }
        }
        Ok(())
    }
}
