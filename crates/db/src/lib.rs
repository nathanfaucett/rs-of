#[cfg(feature = "remote")]
mod remote;
#[cfg(feature = "remote")]
pub use remote::RemoteDatabaseConfig;

#[cfg(feature = "native")]
pub use ofdb_sql::Database;
#[cfg(feature = "replica")]
pub use ofdb_sql::{
    Client, FromRow, FromRowError, FromValue, Query, QueryColumn, QueryDelete, QueryError,
    QueryErrorKind, QueryExpr, QueryExprValue, QueryFrom, QueryInsert, QueryResult, QuerySelect,
    QueryUpdate, QueryUpdateAssignment, Row, SqlTranslator, Statement, Transaction, Uuid, Value,
    ValueType, decode, value,
};

#[cfg(feature = "network")]
mod database_transport;
#[cfg(feature = "network")]
pub use database_transport::DatabaseTransport;
