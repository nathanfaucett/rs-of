use core::slice::from_ref;

#[cfg(all(unix, feature = "unix"))]
use std::path::PathBuf;

use proto::{ExecuteRequest, query_service_client::QueryServiceClient};
use protocol::{
    decode_error_detail, query_result_from_proto, row_to_proto, statement_to_proto,
    value_from_proto, value_to_proto,
};
use query::{QueryError, QueryErrorKind, QueryResult, Statement};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{
    Code, Status,
    transport::{Certificate, Channel, ClientTlsConfig, Endpoint, Identity},
};
use value::{Row, Value};

use crate::Transaction;

#[derive(Clone, Debug)]
pub struct Client {
    channel: Channel,
}

impl Client {
    /// Connects to a SQL service over TCP.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid endpoint or connection failure.
    pub async fn tcp(uri: impl AsRef<str>) -> Result<Self, QueryError> {
        let endpoint = Endpoint::from_shared(uri.as_ref().to_string()).map_err(|error| {
            QueryError::new(
                QueryErrorKind::Validation,
                format!("invalid gRPC endpoint: {error}"),
            )
        })?;
        Self::connect_endpoint(endpoint).await
    }

    /// Connects to a SQL service over TCP with a trusted CA.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid TLS or endpoint configuration, or connection failure.
    pub async fn tcp_with_ca(
        uri: impl AsRef<str>,
        ca_certificate: impl AsRef<[u8]>,
    ) -> Result<Self, QueryError> {
        let endpoint = Endpoint::from_shared(uri.as_ref().to_string()).map_err(|error| {
            QueryError::new(
                QueryErrorKind::Validation,
                format!("invalid gRPC endpoint: {error}"),
            )
        })?;
        let endpoint = endpoint
            .tls_config(
                ClientTlsConfig::new().ca_certificate(Certificate::from_pem(ca_certificate)),
            )
            .map_err(|error| QueryError::new(QueryErrorKind::Validation, error.to_string()))?;
        Self::connect_endpoint(endpoint).await
    }

    /// Connects to a SQL service over TCP with a CA and client identity.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid TLS or endpoint configuration, or connection failure.
    pub async fn tcp_with_identity(
        uri: impl AsRef<str>,
        ca_certificate: impl AsRef<[u8]>,
        certificate: impl AsRef<[u8]>,
        private_key: impl AsRef<[u8]>,
    ) -> Result<Self, QueryError> {
        let endpoint = Endpoint::from_shared(uri.as_ref().to_string()).map_err(|error| {
            QueryError::new(
                QueryErrorKind::Validation,
                format!("invalid gRPC endpoint: {error}"),
            )
        })?;
        let tls = ClientTlsConfig::new()
            .ca_certificate(Certificate::from_pem(ca_certificate))
            .identity(Identity::from_pem(certificate, private_key));
        let endpoint = endpoint
            .tls_config(tls)
            .map_err(|error| QueryError::new(QueryErrorKind::Validation, error.to_string()))?;
        Self::connect_endpoint(endpoint).await
    }

    async fn connect_endpoint(endpoint: Endpoint) -> Result<Self, QueryError> {
        let channel = endpoint
            .connect()
            .await
            .map_err(|error| QueryError::new(QueryErrorKind::Transport, format!("{error:?}")))?;
        Ok(Self { channel })
    }

    /// Connects to a SQL service over a Unix socket.
    ///
    /// # Errors
    ///
    /// Returns an error when the socket connection fails.
    #[cfg(all(unix, feature = "unix"))]
    pub async fn unix(path: impl Into<PathBuf>) -> Result<Self, QueryError> {
        use hyper_util::rt::TokioIo;
        use tokio::net::UnixStream;
        use tower::service_fn;

        let path = path.into();
        let channel = Endpoint::from_static("http://[::]:50051")
            .connect_with_connector(service_fn(move |_| {
                let path = path.clone();
                async move { UnixStream::connect(path).await.map(TokioIo::new) }
            }))
            .await
            .map_err(|error| QueryError::new(QueryErrorKind::Transport, error.to_string()))?;
        Ok(Self { channel })
    }

    /// Returns conflicts for several keys in one request.
    ///
    /// # Errors
    ///
    /// Returns an error for transport failure or an invalid response.
    pub async fn row_conflicts_batch(
        &self,
        table: &str,
        keys: &[Row],
    ) -> Result<Vec<Vec<String>>, QueryError> {
        let response = QueryServiceClient::new(self.channel.clone())
            .row_conflicts(conflict_request(table, keys))
            .await
            .map_err(status_error)?
            .into_inner();
        if response.rows.len() != keys.len() {
            return Err(QueryError::new(
                QueryErrorKind::Protocol,
                "conflict response row count mismatch",
            ));
        }
        Ok(response.rows.into_iter().map(|row| row.columns).collect())
    }

    /// Returns conflicts for one key.
    ///
    /// # Errors
    ///
    /// Returns an error for transport failure or an invalid response.
    pub async fn row_conflicts(&self, table: &str, key: &Row) -> Result<Vec<String>, QueryError> {
        Ok(self
            .row_conflicts_batch(table, from_ref(key))
            .await?
            .remove(0))
    }

    /// Returns conflict values for one key.
    ///
    /// # Errors
    ///
    /// Returns an error for transport failure or an invalid response.
    pub async fn row_conflict_values(
        &self,
        table: &str,
        key: &Row,
    ) -> Result<Vec<(String, Vec<Value>)>, QueryError> {
        let response = QueryServiceClient::new(self.channel.clone())
            .row_conflict_values(conflict_request(table, from_ref(key)))
            .await
            .map_err(status_error)?
            .into_inner();
        decode_conflict_values(response)
    }

    /// Resolves a conflicted row using column values.
    ///
    /// # Errors
    ///
    /// Returns an error for transport failure or a rejected resolution.
    pub async fn resolve_row(
        &self,
        table: &str,
        key: &Row,
        values: Vec<(String, Value)>,
    ) -> Result<(), QueryError> {
        QueryServiceClient::new(self.channel.clone())
            .resolve_row(proto::ResolveRowRequest {
                table: table.into(),
                key: Some(row_to_proto(key.clone())),
                columns: values
                    .into_iter()
                    .map(|(column, value)| proto::ConflictValues {
                        column,
                        values: vec![value_to_proto(value)],
                    })
                    .collect(),
            })
            .await
            .map_err(status_error)?;
        Ok(())
    }

    /// Starts a remote transaction.
    ///
    /// # Errors
    ///
    /// Returns an error for transport failure or an invalid ready response.
    pub async fn transaction(&self) -> Result<Transaction, QueryError> {
        let mut client = QueryServiceClient::new(self.channel.clone());
        let (sender, receiver) = mpsc::channel(1);
        let response = client
            .transaction(ReceiverStream::new(receiver))
            .await
            .map_err(status_error)?;
        Transaction::new(sender, response.into_inner()).await
    }

    /// Executes statements outside an explicit transaction.
    ///
    /// # Errors
    ///
    /// Returns an error for transport failure or a malformed result.
    pub async fn execute(
        &self,
        statements: Vec<Statement>,
    ) -> Result<Vec<QueryResult>, QueryError> {
        let mut client = QueryServiceClient::new(self.channel.clone());
        let request = ExecuteRequest {
            statements: statements.into_iter().map(statement_to_proto).collect(),
        };
        let response = client.execute(request).await.map_err(status_error)?;
        response
            .into_inner()
            .results
            .into_iter()
            .map(|result| {
                query_result_from_proto(result)
                    .map_err(|error| QueryError::new(QueryErrorKind::Protocol, error.to_string()))
            })
            .collect()
    }
}

pub fn conflict_request(table: &str, keys: &[Row]) -> proto::ConflictRequest {
    proto::ConflictRequest {
        table: table.into(),
        keys: keys.iter().cloned().map(row_to_proto).collect(),
    }
}

pub fn decode_conflict_values(
    response: proto::ConflictValuesResponse,
) -> Result<Vec<(String, Vec<Value>)>, QueryError> {
    response
        .columns
        .into_iter()
        .map(|column| {
            let values = column
                .values
                .into_iter()
                .map(|value| {
                    value_from_proto(value).map_err(|error| {
                        QueryError::new(QueryErrorKind::Protocol, error.to_string())
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok((column.column, values))
        })
        .collect()
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "tonic status converter is used directly and through Result::map_err"
)]
pub fn status_error(status: Status) -> QueryError {
    if let Some((kind, message)) = decode_error_detail(status.details()) {
        return QueryError::new(kind, message);
    }

    let kind = match status.code() {
        Code::DeadlineExceeded => QueryErrorKind::Timeout,
        Code::InvalidArgument | Code::OutOfRange => QueryErrorKind::Validation,
        Code::FailedPrecondition | Code::Aborted => QueryErrorKind::Rejected,
        Code::Unimplemented => QueryErrorKind::Unsupported,
        Code::Internal | Code::DataLoss => QueryErrorKind::Internal,
        Code::Unavailable | Code::Cancelled => QueryErrorKind::Transport,
        _ => QueryErrorKind::Protocol,
    };
    QueryError::new(kind, status.message())
}

#[cfg(test)]
mod tests {
    use super::status_error;
    use query::{QueryError, QueryErrorKind};
    use tonic::{Code, Status, codegen::Bytes};

    #[test]
    fn status_error_uses_structured_details() {
        let status = Status::with_details(
            Code::Internal,
            "different human message",
            Bytes::from_static(&[QueryErrorKind::Rejected as u8, b'n', b'o']),
        );
        assert_eq!(
            status_error(status),
            QueryError::new(QueryErrorKind::Rejected, "no")
        );
    }

    #[test]
    fn status_error_falls_back_to_tonic_code() {
        assert_eq!(
            status_error(Status::new(Code::DeadlineExceeded, "deadline")),
            QueryError::new(QueryErrorKind::Timeout, "deadline"),
        );
    }
}
