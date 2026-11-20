use kv_proto::kvdb::{
    DeleteRequest, GetRequest, ScanAllRequest, ScanPrefixRequest, ScanRequest, SetRequest,
    kv_service_client::KvServiceClient,
};
#[cfg(all(unix, feature = "unix"))]
use std::path::PathBuf;

use tonic::{
    Status,
    transport::{Channel, Endpoint, Error as TransportError},
};
use value::Value;

#[derive(Clone, Debug)]
enum Connector {
    Tcp(Box<Endpoint>),
    #[cfg(all(unix, feature = "unix"))]
    Unix(PathBuf),
}

#[derive(Clone, Debug)]
pub struct Client {
    connector: Connector,
}

impl Client {
    /// Creates a lazy TCP connector.
    ///
    /// # Errors
    ///
    /// Returns an error when the endpoint URI is invalid.
    pub fn lazy_tcp(uri: impl AsRef<str>) -> Result<Self, TransportError> {
        let endpoint = Endpoint::from_shared(uri.as_ref().to_string())?;
        Ok(Self {
            connector: Connector::Tcp(Box::new(endpoint)),
        })
    }

    /// Creates a lazy Unix-socket connector.
    ///
    /// # Errors
    ///
    /// This constructor is infallible; the result matches `lazy_tcp`.
    #[cfg(all(unix, feature = "unix"))]
    #[allow(
        clippy::unnecessary_wraps,
        reason = "keeps Unix and TCP constructor results consistent"
    )]
    pub fn lazy_unix(path: impl Into<PathBuf>) -> Result<Self, TransportError> {
        Ok(Self {
            connector: Connector::Unix(path.into()),
        })
    }

    /// Gets a value by key.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails or the response is malformed.
    pub async fn get(&self, key: String) -> Result<Option<Value>, Status> {
        let mut client = KvServiceClient::new(self.channel());
        let request = GetRequest { key };
        let response = client.get(request).await?;
        let inner = response.into_inner();
        kv_proto::optional_value(inner.value)
    }

    /// Sets a value with an optional expiry.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails.
    pub async fn set(
        &self,
        key: String,
        value: Value,
        expires_at: Option<i64>,
    ) -> Result<(), Status> {
        let mut client = KvServiceClient::new(self.channel());
        let request = SetRequest {
            key,
            value: Some(kv_proto::value_to_proto(value)),
            expires_at,
        };
        client.set(request).await?;
        Ok(())
    }

    /// Deletes a value by key.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails.
    pub async fn delete(&self, key: String) -> Result<(), Status> {
        let mut client = KvServiceClient::new(self.channel());
        let request = DeleteRequest { key };
        client.delete(request).await?;
        Ok(())
    }

    /// Scans a key range.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails or the response is malformed.
    pub async fn scan(&self, start: String, end: String) -> Result<Vec<(String, Value)>, Status> {
        let mut client = KvServiceClient::new(self.channel());
        let request = ScanRequest { start, end };
        let response = client.scan(request).await?;
        let inner = response.into_inner();
        kv_proto::scan_entries(inner.entries)
    }

    /// Scans values whose keys start with a prefix.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails or the response is malformed.
    pub async fn scan_prefix(&self, prefix: String) -> Result<Vec<(String, Value)>, Status> {
        let mut client = KvServiceClient::new(self.channel());
        let request = ScanPrefixRequest { prefix };
        let response = client.scan_prefix(request).await?;
        kv_proto::scan_entries(response.into_inner().entries)
    }

    /// Scans every value.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails or the response is malformed.
    pub async fn scan_all(&self) -> Result<Vec<(String, Value)>, Status> {
        let mut client = KvServiceClient::new(self.channel());
        let request = ScanAllRequest {};
        let response = client.scan_all(request).await?;
        let inner = response.into_inner();
        kv_proto::scan_entries(inner.entries)
    }

    fn channel(&self) -> Channel {
        match &self.connector {
            Connector::Tcp(endpoint) => endpoint.connect_lazy(),
            #[cfg(all(unix, feature = "unix"))]
            Connector::Unix(path) => {
                use hyper_util::rt::TokioIo;
                use tokio::net::UnixStream;
                use tower::service_fn;
                let path = path.clone();
                // Tonic needs a URI, but this connector ignores it and opens the Unix socket below.
                Endpoint::from_static("http://[::]:50051").connect_with_connector_lazy(service_fn(
                    move |_| {
                        let path = path.clone();
                        async move { UnixStream::connect(path).await.map(TokioIo::new) }
                    },
                ))
            }
        }
    }
}
