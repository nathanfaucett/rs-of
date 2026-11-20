use core::fmt;
#[cfg(feature = "redb")]
use std::io;
use std::time::Duration;

#[cfg(feature = "remote")]
use kv_proto::kvdb::{
    DeleteRequest, GetRequest, ScanAllRequest, ScanPrefixRequest, ScanRequest, SetRequest,
    TransactionRequest, TransactionResponse, kv_service_client::KvServiceClient,
    transaction_request::Command, transaction_response::Outcome,
};
#[cfg(any(
    feature = "in-memory",
    feature = "redb",
    all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown")
))]
use kv_store::KvTransaction;
#[cfg(feature = "remote")]
use tokio::sync::mpsc;
#[cfg(feature = "remote")]
use tokio_stream::wrappers::ReceiverStream;
#[cfg(feature = "remote")]
use tonic::{Streaming, transport::Channel};

use value::Value;

#[cfg(feature = "remote")]
use crate::client::map_status;
use crate::{Error, database::Storage};

#[derive(Debug)]
pub struct Transaction {
    backend: Backend,
    request_deadline: Option<Duration>,
}

#[cfg(feature = "in-memory")]
type MemoryTransaction = KvTransaction<
    <btree::InMemoryBTree<Vec<u8>, Vec<u8>> as btree::BTree<Vec<u8>, Vec<u8>>>::Transaction,
>;
#[cfg(feature = "redb")]
type RedbTransaction =
    KvTransaction<<btree_redb::RedbByteBTree as btree::BTree<Vec<u8>, Vec<u8>>>::Transaction>;

#[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
type IndexedDbTransaction = KvTransaction<btree_indexeddb::Transaction>;

enum Backend {
    #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
    IndexedDb(Box<IndexedDbTransaction>),
    #[cfg(feature = "in-memory")]
    Memory(Box<MemoryTransaction>),
    #[cfg(feature = "redb")]
    Redb(
        btree_redb::NativeTransaction<
            kv_store::KvStore<btree_redb::RedbByteBTree>,
            Box<RedbTransaction>,
        >,
    ),
    #[cfg(feature = "remote")]
    Remote(Box<RemoteTransaction>),
}

impl fmt::Debug for Backend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            Self::IndexedDb(_) => f.debug_tuple("IndexedDb").field(&"...").finish(),
            #[cfg(feature = "in-memory")]
            Self::Memory(_) => f.debug_tuple("Memory").field(&"...").finish(),
            #[cfg(feature = "redb")]
            Self::Redb(_) => f.debug_tuple("Redb").field(&"...").finish(),
            #[cfg(feature = "remote")]
            Self::Remote(_) => f.debug_tuple("Remote").field(&"...").finish(),
        }
    }
}

impl Transaction {
    #[cfg(any(
        feature = "in-memory",
        feature = "redb",
        all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown")
    ))]
    pub(crate) async fn embedded(
        database: &crate::Database,
        deadline: Option<Duration>,
    ) -> Result<Self, Error> {
        let backend = match &database.storage {
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            Storage::IndexedDb(store) => Backend::IndexedDb(Box::new(store.transaction().await?)),
            #[cfg(feature = "in-memory")]
            Storage::Memory(store) => Backend::Memory(Box::new(store.transaction().await?)),
            #[cfg(feature = "redb")]
            Storage::Redb(store) => {
                let admitted = store.read().await.map_err(native_error)?;
                Backend::Redb(store.transaction(Box::new(admitted.transaction().await?)))
            }
        };
        Ok(Self {
            backend,
            request_deadline: deadline,
        })
    }

    #[cfg(feature = "remote")]
    pub(crate) async fn remote(
        channel: Channel,
        deadline: Option<Duration>,
    ) -> Result<Self, Error> {
        Ok(Self {
            backend: Backend::Remote(Box::new(RemoteTransaction::connect(channel).await?)),
            request_deadline: deadline,
        })
    }

    /// Gets a value by key.
    ///
    /// # Errors
    ///
    /// Returns an error for storage, transport, or decoding failure.
    pub async fn get(&mut self, key: &str) -> Result<Option<Value>, Error> {
        Self::run(
            self.request_deadline,
            Box::pin(async {
                match &mut self.backend {
                    #[cfg(all(
                        feature = "indexeddb",
                        target_arch = "wasm32",
                        target_os = "unknown"
                    ))]
                    Backend::IndexedDb(tx) => {
                        tx.get(key, current_time_millis()).await.map_err(Into::into)
                    }
                    #[cfg(feature = "in-memory")]
                    Backend::Memory(tx) => Box::pin(tx.get(key, current_time_millis()))
                        .await
                        .map_err(Into::into),
                    #[cfg(feature = "redb")]
                    Backend::Redb(tx) => Box::pin(
                        tx.read()
                            .await
                            .map_err(native_error)?
                            .get(key, current_time_millis()),
                    )
                    .await
                    .map_err(Into::into),
                    #[cfg(feature = "remote")]
                    Backend::Remote(tx) => tx.get(key).await,
                }
            }),
        )
        .await
    }

    /// Sets a value with an optional expiry.
    ///
    /// # Errors
    ///
    /// Returns an error for storage, transport, or encoding failure.
    pub async fn set(
        &mut self,
        key: &str,
        value: impl Into<Value>,
        expires_at: Option<i64>,
    ) -> Result<(), Error> {
        let value = value.into();
        Self::run(
            self.request_deadline,
            Box::pin(async {
                match &mut self.backend {
                    #[cfg(all(
                        feature = "indexeddb",
                        target_arch = "wasm32",
                        target_os = "unknown"
                    ))]
                    Backend::IndexedDb(tx) => {
                        tx.set(key, value, expires_at).await.map_err(Into::into)
                    }
                    #[cfg(feature = "in-memory")]
                    Backend::Memory(tx) => Box::pin(tx.set(key, value, expires_at))
                        .await
                        .map_err(Into::into),
                    #[cfg(feature = "redb")]
                    Backend::Redb(tx) => Box::pin(
                        tx.read()
                            .await
                            .map_err(native_error)?
                            .set(key, value, expires_at),
                    )
                    .await
                    .map_err(Into::into),
                    #[cfg(feature = "remote")]
                    Backend::Remote(tx) => tx.set(key, value, expires_at).await,
                }
            }),
        )
        .await
    }

    /// Deletes a key.
    ///
    /// # Errors
    ///
    /// Returns an error for storage or transport failure.
    pub async fn delete(&mut self, key: &str) -> Result<(), Error> {
        Self::run(
            self.request_deadline,
            Box::pin(async {
                match &mut self.backend {
                    #[cfg(all(
                        feature = "indexeddb",
                        target_arch = "wasm32",
                        target_os = "unknown"
                    ))]
                    Backend::IndexedDb(tx) => tx.delete(key).await.map_err(Into::into),
                    #[cfg(feature = "in-memory")]
                    Backend::Memory(tx) => Box::pin(tx.delete(key)).await.map_err(Into::into),
                    #[cfg(feature = "redb")]
                    Backend::Redb(tx) => {
                        Box::pin(tx.read().await.map_err(native_error)?.delete(key))
                            .await
                            .map_err(Into::into)
                    }
                    #[cfg(feature = "remote")]
                    Backend::Remote(tx) => tx.delete(key).await,
                }
            }),
        )
        .await
    }

    /// Scans a key range.
    ///
    /// # Errors
    ///
    /// Returns an error for storage, transport, or decoding failure.
    pub async fn scan(&mut self, start: &str, end: &str) -> Result<Vec<(String, Value)>, Error> {
        Self::run(
            self.request_deadline,
            Box::pin(async {
                match &mut self.backend {
                    #[cfg(all(
                        feature = "indexeddb",
                        target_arch = "wasm32",
                        target_os = "unknown"
                    ))]
                    Backend::IndexedDb(tx) => tx
                        .scan(start.to_owned()..end.to_owned(), current_time_millis())
                        .await
                        .map_err(Into::into),
                    #[cfg(feature = "in-memory")]
                    Backend::Memory(tx) => {
                        Box::pin(tx.scan(start.to_owned()..end.to_owned(), current_time_millis()))
                            .await
                            .map_err(Into::into)
                    }
                    #[cfg(feature = "redb")]
                    Backend::Redb(tx) => Box::pin(
                        tx.read()
                            .await
                            .map_err(native_error)?
                            .scan(start.to_owned()..end.to_owned(), current_time_millis()),
                    )
                    .await
                    .map_err(Into::into),
                    #[cfg(feature = "remote")]
                    Backend::Remote(tx) => tx.scan(start, end).await,
                }
            }),
        )
        .await
    }

    /// Scans values whose keys start with a prefix.
    ///
    /// # Errors
    ///
    /// Returns an error for storage, transport, or decoding failure.
    pub async fn scan_prefix(&mut self, prefix: &str) -> Result<Vec<(String, Value)>, Error> {
        Self::run(
            self.request_deadline,
            Box::pin(async {
                match &mut self.backend {
                    #[cfg(all(
                        feature = "indexeddb",
                        target_arch = "wasm32",
                        target_os = "unknown"
                    ))]
                    Backend::IndexedDb(tx) => tx
                        .scan_prefix(prefix, current_time_millis())
                        .await
                        .map_err(Into::into),
                    #[cfg(feature = "in-memory")]
                    Backend::Memory(tx) => Box::pin(tx.scan_prefix(prefix, current_time_millis()))
                        .await
                        .map_err(Into::into),
                    #[cfg(feature = "redb")]
                    Backend::Redb(tx) => Box::pin(
                        tx.read()
                            .await
                            .map_err(native_error)?
                            .scan_prefix(prefix, current_time_millis()),
                    )
                    .await
                    .map_err(Into::into),
                    #[cfg(feature = "remote")]
                    Backend::Remote(tx) => tx.scan_prefix(prefix).await,
                }
            }),
        )
        .await
    }

    /// Scans every value.
    ///
    /// # Errors
    ///
    /// Returns an error for storage, transport, or decoding failure.
    pub async fn scan_all(&mut self) -> Result<Vec<(String, Value)>, Error> {
        Self::run(
            self.request_deadline,
            Box::pin(async {
                match &mut self.backend {
                    #[cfg(all(
                        feature = "indexeddb",
                        target_arch = "wasm32",
                        target_os = "unknown"
                    ))]
                    Backend::IndexedDb(tx) => {
                        tx.scan_all(current_time_millis()).await.map_err(Into::into)
                    }
                    #[cfg(feature = "in-memory")]
                    Backend::Memory(tx) => Box::pin(tx.scan_all(current_time_millis()))
                        .await
                        .map_err(Into::into),
                    #[cfg(feature = "redb")]
                    Backend::Redb(tx) => Box::pin(
                        tx.read()
                            .await
                            .map_err(native_error)?
                            .scan_all(current_time_millis()),
                    )
                    .await
                    .map_err(Into::into),
                    #[cfg(feature = "remote")]
                    Backend::Remote(tx) => tx.scan_all().await,
                }
            }),
        )
        .await
    }

    /// Commits the transaction.
    ///
    /// # Errors
    ///
    /// Returns an error for transport or commit failure.
    pub async fn commit(self) -> Result<(), Error> {
        let deadline = self.request_deadline;
        Self::run(
            deadline,
            Box::pin(async move {
                match self.backend {
                    #[cfg(all(
                        feature = "indexeddb",
                        target_arch = "wasm32",
                        target_os = "unknown"
                    ))]
                    Backend::IndexedDb(tx) => tx.commit().await.map_err(Into::into),
                    #[cfg(feature = "in-memory")]
                    Backend::Memory(tx) => tx.commit().await.map_err(Into::into),
                    #[cfg(feature = "redb")]
                    Backend::Redb(tx) => {
                        let (_admitted, tx) = tx.take().await.map_err(native_error)?;
                        tx.commit().await.map_err(Into::into)
                    }
                    #[cfg(feature = "remote")]
                    Backend::Remote(tx) => tx.finish(true).await,
                }
            }),
        )
        .await
    }

    /// Rolls back the transaction.
    ///
    /// # Errors
    ///
    /// Returns an error for transport or rollback failure.
    pub async fn rollback(self) -> Result<(), Error> {
        let deadline = self.request_deadline;
        Self::run(
            deadline,
            Box::pin(async move {
                match self.backend {
                    #[cfg(all(
                        feature = "indexeddb",
                        target_arch = "wasm32",
                        target_os = "unknown"
                    ))]
                    Backend::IndexedDb(tx) => tx.rollback().await.map_err(Into::into),
                    #[cfg(feature = "in-memory")]
                    Backend::Memory(tx) => tx.rollback().await.map_err(Into::into),
                    #[cfg(feature = "redb")]
                    Backend::Redb(tx) => {
                        let (_admitted, tx) = tx.take().await.map_err(native_error)?;
                        tx.rollback().await.map_err(Into::into)
                    }
                    #[cfg(feature = "remote")]
                    Backend::Remote(tx) => tx.finish(false).await,
                }
            }),
        )
        .await
    }

    #[allow(
        clippy::map_err_ignore,
        reason = "an elapsed deadline has one stable timeout error"
    )]
    async fn run<F, T>(deadline: Option<Duration>, operation: F) -> Result<T, Error>
    where
        F: Future<Output = Result<T, Error>>,
    {
        match deadline {
            Some(deadline) => platform::timeout(deadline, operation)
                .await
                .map_err(|_| Error::Timeout)?,
            None => operation.await,
        }
    }
}

#[cfg(feature = "remote")]
struct RemoteTransaction {
    sender: Option<mpsc::Sender<TransactionRequest>>,
    responses: Streaming<TransactionResponse>,
}

#[cfg(feature = "remote")]
impl RemoteTransaction {
    async fn connect(channel: Channel) -> Result<Self, Error> {
        let mut client = KvServiceClient::new(channel);
        let (sender, receiver) = mpsc::channel(1);
        let response = client
            .transaction(ReceiverStream::new(receiver))
            .await
            .map_err(map_status)?;
        let mut transaction = Self {
            sender: Some(sender),
            responses: response.into_inner(),
        };
        if !matches!(transaction.response().await?, Outcome::Ready(())) {
            return Err(protocol_error("expected transaction ready response"));
        }
        Ok(transaction)
    }

    async fn get(&mut self, key: &str) -> Result<Option<Value>, Error> {
        match self
            .exchange(Command::Get(GetRequest {
                key: key.to_owned(),
            }))
            .await?
        {
            Outcome::Got(response) => kv_proto::optional_value(response.value).map_err(map_status),
            _ => Err(protocol_error("expected get response")),
        }
    }

    async fn set(&mut self, key: &str, value: Value, expires_at: Option<i64>) -> Result<(), Error> {
        self.exchange(Command::Set(SetRequest {
            key: key.to_owned(),
            value: Some(kv_proto::value_to_proto(value)),
            expires_at,
        }))
        .await?;
        Ok(())
    }

    async fn delete(&mut self, key: &str) -> Result<(), Error> {
        self.exchange(Command::Delete(DeleteRequest {
            key: key.to_owned(),
        }))
        .await?;
        Ok(())
    }

    async fn scan(&mut self, start: &str, end: &str) -> Result<Vec<(String, Value)>, Error> {
        self.scan_result(Command::Scan(ScanRequest {
            start: start.into(),
            end: end.into(),
        }))
        .await
    }

    async fn scan_prefix(&mut self, prefix: &str) -> Result<Vec<(String, Value)>, Error> {
        self.scan_result(Command::ScanPrefix(ScanPrefixRequest {
            prefix: prefix.into(),
        }))
        .await
    }

    async fn scan_all(&mut self) -> Result<Vec<(String, Value)>, Error> {
        self.scan_result(Command::ScanAll(ScanAllRequest {})).await
    }

    async fn scan_result(&mut self, command: Command) -> Result<Vec<(String, Value)>, Error> {
        match self.exchange(command).await? {
            Outcome::Scanned(response) => {
                kv_proto::scan_entries(response.entries).map_err(map_status)
            }
            _ => Err(protocol_error("expected scan response")),
        }
    }

    async fn finish(mut self, commit: bool) -> Result<(), Error> {
        let command = if commit {
            Command::Commit(())
        } else {
            Command::Rollback(())
        };
        self.exchange(command).await?;
        Ok(())
    }

    #[allow(
        clippy::map_err_ignore,
        reason = "a closed request stream has one stable transport error"
    )]
    async fn exchange(&mut self, command: Command) -> Result<Outcome, Error> {
        let sender = self
            .sender
            .take()
            .ok_or_else(|| protocol_error("transaction is closed"))?;
        sender
            .send(TransactionRequest {
                command: Some(command),
            })
            .await
            .map_err(|_| Error::Transport("transaction stream closed".into()))?;
        let response = self.response().await;
        if response.is_ok() {
            self.sender = Some(sender);
        }
        match response? {
            Outcome::ErrorDetail(detail) => {
                if let Some((kind, message)) = kv_proto::decode_error_details(&detail) {
                    Err(Error::Query {
                        kind: kind.into(),
                        message: message.to_owned(),
                    })
                } else {
                    Err(protocol_error("invalid transaction error detail"))
                }
            }
            outcome => Ok(outcome),
        }
    }

    async fn response(&mut self) -> Result<Outcome, Error> {
        self.responses
            .message()
            .await
            .map_err(map_status)?
            .and_then(|message| message.outcome)
            .ok_or_else(|| Error::Transport("transaction stream closed".into()))
    }
}

#[cfg(feature = "remote")]
fn protocol_error(message: &str) -> Error {
    Error::Query {
        kind: crate::ErrorKind::Internal,
        message: message.into(),
    }
}

#[cfg(any(
    feature = "in-memory",
    feature = "redb",
    all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown")
))]
fn current_time_millis() -> i64 {
    platform::unix_millis()
}

#[cfg(feature = "redb")]
fn native_error(error: io::Error) -> Error {
    Error::from(btree::BTreeError::custom(error))
}
