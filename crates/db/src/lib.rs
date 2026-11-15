#[cfg(feature = "remote")]
mod remote;
#[cfg(feature = "remote")]
pub use remote::RemoteDatabaseConfig;

#[cfg(feature = "native")]
pub use ofdb_sql::Database;

#[cfg(feature = "ofdb")]
pub use ofdb_sql::{
    Client, FromRow, FromRowError, FromValue, Query, QueryColumn, QueryDelete, QueryError,
    QueryErrorKind, QueryExpr, QueryExprValue, QueryFrom, QueryInsert, QueryParams, QueryResult,
    QuerySelect, QueryUpdate, QueryUpdateAssignment, Row, SqlTranslator, Statement, Transaction,
    Uuid, Value, ValueType, decode, value,
};

#[cfg(feature = "network")]
mod database_transport;
#[cfg(feature = "network")]
pub use database_transport::{DatabaseTransport, SyncTransport};

#[cfg(feature = "ofdb")]
pub mod ofdb;

#[cfg(feature = "native")]
pub mod adapters;

#[cfg(feature = "native")]
pub use adapters::{AdapterError, EmbeddedDatabase, MemoryDatabase};

#[cfg(feature = "postgres")]
pub use adapters::NetworkDatabase;

#[cfg(feature = "remote")]
pub use adapters::RemoteDatabase;

#[cfg(feature = "native")]
pub mod local;

#[cfg(feature = "native")]
pub use local::{local_client, local_database, open_private_storage};
