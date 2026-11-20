use core::fmt;
use std::error::Error;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use btree::{BTree, BTreeError};
use kv::KvStore;
use kv_proto as proto;
use kv_proto::kvdb::{
    DeleteRequest, DeleteResponse, GetRequest, GetResponse, ScanAllRequest, ScanAllResponse,
    ScanEntry, ScanPrefixRequest, ScanRequest, ScanResponse, SetRequest, SetResponse,
    TransactionRequest,
    kv_service_server::{KvService as KvServiceTrait, KvServiceServer},
};
use tokio::net::TcpListener;
#[cfg(unix)]
use tokio::net::UnixListener;
use tokio_stream::wrappers::TcpListenerStream;
#[cfg(unix)]
use tokio_stream::wrappers::UnixListenerStream;
use tonic::{
    Code, Request, Response, Status, Streaming, async_trait,
    codegen::Bytes,
    transport::{Certificate, Identity, Server as TonicServer, ServerTlsConfig},
};

use crate::transaction::{TransactionStream, stream};

#[derive(Debug)]
enum Endpoint {
    Tcp(SocketAddr),
    TcpListener(TcpListener),
    #[cfg(unix)]
    Unix(PathBuf),
}

#[derive(Clone, Debug)]
pub struct KvService<S>
where
    S: BTree<Vec<u8>, Vec<u8>> + fmt::Debug,
{
    store: Arc<KvStore<S>>,
    max_decoding_message_size: Option<usize>,
}

impl<S> KvService<S>
where
    S: BTree<Vec<u8>, Vec<u8>> + fmt::Debug,
{
    pub fn new(store: KvStore<S>) -> Self {
        Self {
            store: Arc::new(store),
            max_decoding_message_size: None,
        }
    }

    pub const fn max_decoding_message_size(mut self, size: usize) -> Self {
        self.max_decoding_message_size = Some(size);
        self
    }

    pub fn into_tonic_service(self) -> KvServiceServer<Self> {
        let max_decoding_message_size = self.max_decoding_message_size;
        let service = KvServiceServer::new(self);
        match max_decoding_message_size {
            Some(size) => service.max_decoding_message_size(size),
            None => service,
        }
    }
}

#[async_trait]
impl<S> KvServiceTrait for KvService<S>
where
    S: BTree<Vec<u8>, Vec<u8>> + Send + Sync + 'static + fmt::Debug,
{
    type TransactionStream = TransactionStream;

    async fn transaction(
        &self,
        request: Request<Streaming<TransactionRequest>>,
    ) -> Result<Response<Self::TransactionStream>, Status> {
        let transaction = self.store.transaction().await.map_err(status_from_error)?;
        Ok(Response::new(stream(transaction, request.into_inner())))
    }

    async fn get(&self, request: Request<GetRequest>) -> Result<Response<GetResponse>, Status> {
        let request = request.into_inner();
        let key = request.key;
        let now = current_time_millis()?;

        let tx = self.store.transaction().await.map_err(status_from_error)?;
        let value = tx.get(&key, now).await.map_err(status_from_error)?;

        Ok(Response::new(GetResponse {
            value: value.map(kv_proto::value_to_proto),
        }))
    }

    async fn set(&self, request: Request<SetRequest>) -> Result<Response<SetResponse>, Status> {
        let request = request.into_inner();
        let key = request.key;
        let value = kv_proto::required_value(request.value)?;
        let expires_at = request.expires_at;

        let mut tx = self.store.transaction().await.map_err(status_from_error)?;
        tx.set(&key, value, expires_at)
            .await
            .map_err(status_from_error)?;
        tx.commit().await.map_err(status_from_error)?;

        Ok(Response::new(SetResponse {}))
    }

    async fn delete(
        &self,
        request: Request<DeleteRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let request = request.into_inner();
        let key = request.key;

        let mut tx = self.store.transaction().await.map_err(status_from_error)?;
        tx.delete(&key).await.map_err(status_from_error)?;
        tx.commit().await.map_err(status_from_error)?;

        Ok(Response::new(DeleteResponse {}))
    }

    async fn scan(&self, request: Request<ScanRequest>) -> Result<Response<ScanResponse>, Status> {
        let request = request.into_inner();
        let start = request.start;
        let _end = request.end;
        let now = current_time_millis()?;

        let tx = self.store.transaction().await.map_err(status_from_error)?;
        // Use scan_prefix with start as prefix since scan now takes a prefix
        let entries = tx
            .scan_prefix(&start, now)
            .await
            .map_err(status_from_error)?;

        let entries = entries
            .into_iter()
            .map(|(key, value)| ScanEntry {
                key,
                value: Some(kv_proto::value_to_proto(value)),
            })
            .collect();

        Ok(Response::new(ScanResponse { entries }))
    }

    async fn scan_prefix(
        &self,
        request: Request<ScanPrefixRequest>,
    ) -> Result<Response<ScanResponse>, Status> {
        let request = request.into_inner();
        let now = current_time_millis()?;
        let entries = self
            .store
            .transaction()
            .await
            .map_err(status_from_error)?
            .scan_prefix(&request.prefix, now)
            .await
            .map_err(status_from_error)?;

        let entries = entries
            .into_iter()
            .map(|(key, value)| ScanEntry {
                key,
                value: Some(kv_proto::value_to_proto(value)),
            })
            .collect();

        Ok(Response::new(ScanResponse { entries }))
    }

    async fn scan_all(
        &self,
        request: Request<ScanAllRequest>,
    ) -> Result<Response<ScanAllResponse>, Status> {
        let _request = request.into_inner();
        let now = current_time_millis()?;

        let tx = self.store.transaction().await.map_err(status_from_error)?;
        let entries = tx.scan_all(now).await.map_err(status_from_error)?;

        let entries = entries
            .into_iter()
            .map(|(key, value)| ScanEntry {
                key,
                value: Some(kv_proto::value_to_proto(value)),
            })
            .collect();

        Ok(Response::new(ScanAllResponse { entries }))
    }
}

#[derive(Debug)]
pub struct Server<S>
where
    S: BTree<Vec<u8>, Vec<u8>> + fmt::Debug,
{
    endpoint: Endpoint,
    service: KvService<S>,
    tls_identity: Option<Identity>,
    tls_client_ca: Option<Certificate>,
}

impl<S> Server<S>
where
    S: BTree<Vec<u8>, Vec<u8>> + fmt::Debug,
{
    pub fn tcp(address: SocketAddr, store: KvStore<S>) -> Self {
        Self {
            endpoint: Endpoint::Tcp(address),
            service: KvService::new(store),
            tls_identity: None,
            tls_client_ca: None,
        }
    }

    pub fn tcp_listener(listener: TcpListener, store: KvStore<S>) -> Self {
        Self {
            endpoint: Endpoint::TcpListener(listener),
            service: KvService::new(store),
            tls_identity: None,
            tls_client_ca: None,
        }
    }

    #[cfg(unix)]
    pub fn unix(path: impl Into<PathBuf>, store: KvStore<S>) -> Self {
        Self {
            endpoint: Endpoint::Unix(path.into()),
            service: KvService::new(store),
            tls_identity: None,
            tls_client_ca: None,
        }
    }

    #[must_use]
    pub fn tls_identity(mut self, certificate: Vec<u8>, private_key: Vec<u8>) -> Self {
        self.tls_identity = Some(Identity::from_pem(certificate, private_key));
        self
    }

    #[must_use]
    pub fn tls_client_ca(mut self, certificate: Vec<u8>) -> Self {
        self.tls_client_ca = Some(Certificate::from_pem(certificate));
        self
    }

    #[must_use]
    pub fn max_decoding_message_size(mut self, size: usize) -> Self {
        self.service = self.service.max_decoding_message_size(size);
        self
    }

    /// Serves KV requests until shutdown.
    ///
    /// # Errors
    ///
    /// Returns an error for TLS configuration, listener setup, or transport failure.
    pub async fn serve(
        self,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> Result<(), Box<dyn Error + Send + Sync>>
    where
        S: BTree<Vec<u8>, Vec<u8>> + Send + Sync + 'static,
    {
        let service = self.service.into_tonic_service();
        let mut builder = TonicServer::builder();
        if let Some(identity) = self.tls_identity {
            let mut tls = ServerTlsConfig::new().identity(identity);
            if let Some(client_ca) = self.tls_client_ca {
                tls = tls.client_ca_root(client_ca);
            }
            builder = builder.tls_config(tls)?;
        }
        match self.endpoint {
            Endpoint::Tcp(address) => builder
                .add_service(service)
                .serve_with_shutdown(address, shutdown)
                .await
                .map_err(Into::into),
            Endpoint::TcpListener(listener) => {
                let incoming = TcpListenerStream::new(listener);
                builder
                    .add_service(service)
                    .serve_with_incoming_shutdown(incoming, shutdown)
                    .await
                    .map_err(Into::into)
            }
            #[cfg(unix)]
            Endpoint::Unix(path) => {
                let listener = UnixListener::bind(path)?;
                let incoming = UnixListenerStream::new(listener);
                builder
                    .add_service(service)
                    .serve_with_incoming_shutdown(incoming, shutdown)
                    .await
                    .map_err(Into::into)
            }
        }
    }
}

pub fn current_time_millis() -> Result<i64, Status> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(status_from_error)?
        .as_millis();
    i64::try_from(millis).map_err(status_from_error)
}

pub fn status_from_error(error: impl Error + 'static) -> Status {
    let kind = (&error as &dyn Error).downcast_ref::<BTreeError>().map_or(
        proto::ErrorKind::Internal,
        |error| match error {
            BTreeError::InvalidDocument => proto::ErrorKind::InvalidDocument,
            BTreeError::TypeMismatch => proto::ErrorKind::TypeMismatch,
            BTreeError::Conflict => proto::ErrorKind::Conflict,
            BTreeError::CommitFailed => proto::ErrorKind::CommitFailed,
            BTreeError::RollbackFailed => proto::ErrorKind::RollbackFailed,
            BTreeError::UnsupportedOperation => proto::ErrorKind::UnsupportedOperation,
            BTreeError::Custom(_) => proto::ErrorKind::Storage,
        },
    );
    let message = error.to_string();
    let details = proto::encode_error_details(kind, &message);
    Status::with_details(Code::Internal, message, Bytes::from(details))
}
