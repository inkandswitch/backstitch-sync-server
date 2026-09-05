use std::{
    cell::{LazyCell, OnceCell},
    collections::{BTreeSet, HashMap},
    ops::DerefMut,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
    time::Duration,
};

use async_tungstenite::tokio::TokioAdapter;
use automerge::{transaction::CommitOptions, Automerge, AutomergeError, ChangeHash};
use future_form::Sendable;
use futures::Stream;
use hyper_util::rt::TokioIo;
use rand::Rng;
use secrecy::{ExposeSecret, SecretBox};
use sedimentree_core::{
    blob::{Blob, BlobMeta},
    depth::CountLeadingZeroBytes,
    fragment::Fragment,
    id::SedimentreeId,
    loose_commit::{id::CommitId, LooseCommit},
    sedimentree::Sedimentree,
};
use subduction_core::{
    connection::message::SyncMessage,
    handler::sync::SyncHandler,
    policy::open::OpenPolicy,
    remote_heads::RemoteHeadsObserver,
    storage::memory::MemoryStorage,
    subduction::{builder::SubductionBuilder, error::WriteError, Subduction},
    timeout::call::CallTimeout,
    transport::message::MessageTransport,
};
use subduction_crypto::signer::memory::MemorySigner;
use subduction_redb_storage::{RedbStorage, RedbStorageError};
use subduction_websocket::tokio::{client::TokioWebSocketClient, TimeoutTokio, TokioSpawn};
use thiserror::Error;
use tokio::{select, sync::Mutex};
use tokio_util::sync::CancellationToken;

use crate::keys::SigningKey;

type Conn = MessageTransport<
    subduction_websocket::websocket::WebSocket<
        TokioAdapter<TokioIo<hyper::upgrade::Upgraded>>,
        Sendable,
    >,
>;

type Subd = Subduction<
    'static,
    Sendable,
    RedbStorage,
    Conn,
    SyncHandler<
        Sendable,
        RedbStorage,
        Conn,
        OpenPolicy,
        CountLeadingZeroBytes,
        TokioSpawn,
        256,
        HeadsObserver,
    >,
    OpenPolicy,
    MemorySigner,
    TimeoutTokio,
    TokioSpawn,
    CountLeadingZeroBytes,
    256,
>;

#[derive(Debug, Clone)]
pub struct Repo {
    subduction: Arc<Subd>,
    // doc_db: DocumentDb,
    // todo (subd): implement
    token: CancellationToken,
}

pub struct DocumentChanged {
    pub new_heads: Vec<ChangeHash>,
}

#[derive(Error, Debug)]
pub enum RepoError {
    #[error("No such document {0}")]
    NoSuchDocument(SedimentreeId),
    #[error(transparent)]
    Storage(#[from] RedbStorageError),
    #[error(transparent)]
    Automerge(#[from] AutomergeError),
    #[error(transparent)]
    Write(
        #[from] WriteError<Sendable, RedbStorage, TokioWebSocketClient<MemorySigner>, SyncMessage>,
    ),
    #[error("the repo has been stopped")]
    Stopped,
    // TODO (subd): Forward the error
    #[error("there was an IO error")]
    Io,
}

// impl From<DocumentDbError> for RepoError {
//     fn from(value: DocumentDbError) -> Self {
//         match value {
//             DocumentDbError::Automerge(automerge_error) => RepoError::Automerge(automerge_error),
//             DocumentDbError::NoSuchDocument(sedimentree_id) => {
//                 RepoError::NoSuchDocument(sedimentree_id)
//             }
//         }
//     }
// }

pub struct HeadsObserver {
    subduction: Arc<std::sync::Mutex<Option<Arc<Subd>>>>,
    // doc_db: DocumentDb,
}

impl RemoteHeadsObserver for HeadsObserver {
    fn on_remote_heads(
        &self,
        id: SedimentreeId,
        peer: subduction_core::peer::id::PeerId,
        heads: subduction_core::remote_heads::RemoteHeads,
    ) {
        tracing::debug!("NEW HEADS {id}:{heads:?} from {peer}");
    }
}

impl Repo {
    pub fn new(storage_directory: &Path, signing_key: SigningKey) -> Result<Self, RepoError> {
        //let doc_db = DocumentDb::new();
        let sub: Arc<std::sync::Mutex<Option<Arc<Subd>>>> = Default::default();
        let heads_observer = HeadsObserver {
            subduction: sub.clone(),
            //doc_db: doc_db.clone(),
        };
        let storage = RedbStorage::new(storage_directory)?;
        let (subduction, _sync_handler, listener, connection_manager) =
            SubductionBuilder::<_, _, _, _, _, _, 256>::default()
                .storage(storage, Arc::new(OpenPolicy))
                .spawner(TokioSpawn)
                .signer(MemorySigner::from_bytes(signing_key.expose_secret()))
                .timer(TimeoutTokio)
                .heads_observer(heads_observer)
                .build();

        let mut guard = sub.lock().expect("ajajajaja");
        *guard = Some(subduction.clone());
        drop(guard);

        let token = CancellationToken::new();
        let tok = token.clone();
        tokio::task::spawn(async move {
            select! {
                _ = tok.cancelled() => {}
                _ = connection_manager => {}
            }
        });

        let tok = token.clone();
        tokio::task::spawn(async move {
            select! {
                _ = tok.cancelled() => {}
                _ = listener => {}
            }
        });

        let this = Self { subduction, token };

        Ok(this)
    }

    pub fn subduction(&self) -> Arc<Subd> {
        self.subduction.clone()
    }

    pub fn stop(&self) {
        self.token.cancel();
    }

    fn ensure_running(&self) -> Result<(), RepoError> {
        if self.token.is_cancelled() {
            return Err(RepoError::Stopped);
        };
        Ok(())
    }

    pub async fn get_document(&self, id: SedimentreeId) -> Result<Automerge, RepoError> {
        self.ensure_running()?;
        let blobs = self
            .subduction
            .get_blobs(id)
            .await?
            .ok_or(RepoError::NoSuchDocument(id))?;

        let mut doc = Automerge::new();
        let mut blobs = Vec::from(blobs);
        blobs.sort_by(|a, b| b.contents().len().cmp(&a.contents().len()));
        let concat =
            blobs
                .into_iter()
                .map(|b| b.as_slice().to_vec())
                .fold(Vec::new(), |mut acc, el| {
                    acc.extend(el);
                    acc
                });
        doc.load_incremental(&concat)?;

        Ok(doc)
    }
}
