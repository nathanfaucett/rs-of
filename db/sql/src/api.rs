#[cfg(any(
    feature = "redb",
    feature = "in-memory",
    feature = "indexeddb",
    feature = "sync"
))]
pub use engine::{
    Engine, EngineError, EngineResult, EngineTransaction, Kernel, KernelTransaction, RowCodec,
};
#[cfg(feature = "in-memory")]
pub use engine::{InMemoryKernel, InMemoryKernelTransaction};
#[cfg(any(
    feature = "redb",
    feature = "in-memory",
    feature = "indexeddb",
    feature = "sync"
))]
pub use engine_automerge::AutomergeRowCodec;
#[cfg(feature = "sync")]
pub use engine_automerge::{ChangeWork, SqlInvalidation};
#[cfg(feature = "redb")]
pub use engine_redb::{RedbKernel, RedbKernelTransaction, redb};
#[cfg(feature = "macros")]
pub use macros::FromRow;
pub use query::{
    DataDefinition, Query, QueryColumn, QueryDelete, QueryError, QueryErrorKind, QueryExpr,
    QueryExprValue, QueryFrom, QueryInsert, QueryParams, QueryResult, QuerySelect, QueryUpdate,
    QueryUpdateAssignment, Statement, TranslateError, Translator,
};
#[cfg(feature = "server")]
pub use sql_server::{EngineExecutor, Server};
#[cfg(feature = "sql")]
pub use sql_translator::SqlTranslator;

#[cfg(all(feature = "sync", not(feature = "validated-history")))]
pub use crate::database::{
    SessionConfig, SyncCandidateView, SyncKey, SyncManifest, SyncRole, SyncSelection,
    SyncStageFactory, SyncStateUnit,
};

#[cfg(feature = "validated-history")]
pub use history::{
    SessionConfig, SyncCandidateView, SyncError, SyncKey, SyncManifest, SyncResult, SyncRole,
    SyncRowView, SyncSelection, SyncStageFactory, SyncStateUnit, SyncTableView, SyncTransport,
    SyncValidationDirection,
};

pub use uuid::Uuid;
pub use value::{
    FromRow, FromRowError, FromValue, JsonNumber, JsonValue, Row, Value, ValueType, decode, value,
};
