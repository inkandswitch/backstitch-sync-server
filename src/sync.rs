use std::{net::IpAddr, path::Path, sync::Arc};

use samod::{
    storage::TokioFilesystemStorage, AcceptorHandle, ConcurrencyConfig, ConnFinishedReason,
    NeverAnnounce, Repo, Transport, Url,
};
use tokio::{
    net::{TcpListener, TcpStream},
    select,
    sync::Semaphore,
};
use tokio_util::sync::CancellationToken;

use crate::bans::IpBans;

const BAN_DURATION: std::time::Duration = std::time::Duration::from_secs(600);
const MAX_FAILED_ATTEMPTS: i64 = 50;
const CONNECTION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

pub struct SyncServer {
    inner: Arc<SyncServerInner>,
}

#[derive(Clone)]
struct SyncServerInner {
    repo: Repo,
    bans: IpBans,
    token: CancellationToken,
    semaphore: Arc<Semaphore>,
}

impl Drop for SyncServer {
    fn drop(&mut self) {
        self.inner.token.cancel();
    }
}

impl SyncServer {
    pub async fn new(port: u16, data_dir: &Path) -> Self {
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

        let this = Self {
            inner: Arc::new(SyncServerInner {
                repo,
                bans: IpBans::new(BAN_DURATION, MAX_FAILED_ATTEMPTS),
                token: CancellationToken::new(),
                semaphore: Arc::new(Semaphore::new(500)),
            }),
        };
        let inner = this.inner.clone();
        tokio::spawn(async move { inner.server_loop(port).await });
        this
    }

    pub fn repo(&self) -> Repo {
        self.inner.repo.clone()
    }
}

impl SyncServerInner {
    async fn server_loop(&self, port: u16) {
        // Start the automerge sync server
        let addr = format!("0.0.0.0:{}", port);
        let acceptor = self
            .repo
            .make_acceptor(Url::parse(&format!("tcp://{addr}")).unwrap())
            .unwrap();
        let listener = TcpListener::bind(&addr).await.unwrap();

        tracing::info!("started automerge sync server on {addr}");

        loop {
            select! {
                _ = self.token.cancelled() => break,
                result = listener.accept() => {
                    let _permit = self.semaphore.clone().acquire_owned().await;
                    match result {
                        Ok((socket, addr)) => {
                            let ip = addr.ip();
                            if self.bans.is_banned(&ip).await {
                                continue;
                            }
                            tracing::info!("Client connected. IP: {ip}");
                            let acceptor = acceptor.clone();
                            // Handle as automerge connection
                            let this = self.clone();
                            tokio::spawn(async move {
                                select! {
                                    _ = this.handle_connection(ip, acceptor, socket) => {}
                                    _ = this.token.cancelled() => {}
                                }
                            });
                        }
                        Err(e) => tracing::error!("couldn't get client: {:?}", e),
                    }
                }
            }
        }

        self.repo.stop().await;
    }

    async fn handle_connection(&self, ip: IpAddr, acceptor: AcceptorHandle, socket: TcpStream) {
        let connection = match acceptor.accept(Transport::from_tokio_io(socket)) {
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
