use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
};
use ofdb_sql::{Query, QueryParams, SqlTranslator, Statement, Translator, Value};
use serde::{Deserialize, Serialize};
use storage_service::DatabaseId;

use super::{ResourceState, authorization, authorize_response, namespace};

#[derive(Deserialize, Serialize, utoipa::ToSchema)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub(super) enum SqlValue {
    Null,
    Boolean(bool),
    Integer(String),
    Float(f64),
    Text(String),
    Uuid(String),
    Blob(Vec<u8>),
}

impl SqlValue {
    fn into_owner(self) -> Result<Value, SqlError> {
        Ok(match self {
            Self::Null => Value::Null,
            Self::Boolean(value) => Value::Bool(value),
            Self::Integer(value) => Value::Integer(value.parse().map_err(|_| invalid())?),
            Self::Float(value) if value.is_finite() => Value::Float(value),
            Self::Text(value) => Value::Text(value),
            Self::Uuid(value) => Value::Uuid(value.parse().map_err(|_| invalid())?),
            Self::Blob(value) => Value::Blob(value),
            Self::Float(_) => return Err(invalid()),
        })
    }

    fn from_owner(value: Value) -> Result<Self, SqlError> {
        Ok(match value {
            Value::Null => Self::Null,
            Value::Bool(value) => Self::Boolean(value),
            Value::Integer(value) => Self::Integer(value.to_string()),
            Value::Float(value) if value.is_finite() => Self::Float(value),
            Value::Text(value) => Self::Text(value),
            Value::Uuid(value) => Self::Uuid(value.to_string()),
            Value::Blob(value) => Self::Blob(value),
            _ => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(DatabaseFailure {
                        code: "unsupportedValue",
                    }),
                ));
            }
        })
    }
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct SqlStatement {
    /// SQL text, at most 16 KiB UTF-8; positional parameters only.
    sql: String,
    #[serde(default)]
    #[schema(max_items = 128)]
    params: Vec<SqlValue>,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct SqlRequest {
    #[schema(min_items = 1, max_items = 32)]
    statements: Vec<SqlStatement>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub(super) struct SqlResult {
    columns: Vec<String>,
    rows: Vec<Vec<SqlValue>>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub(super) struct SqlResponse {
    results: Vec<SqlResult>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub(in crate::router) struct DatabaseFailure {
    code: &'static str,
}

pub(super) type SqlError = (StatusCode, Json<DatabaseFailure>);

fn invalid() -> SqlError {
    (
        StatusCode::BAD_REQUEST,
        Json(DatabaseFailure {
            code: "invalidRequest",
        }),
    )
}

fn failed() -> SqlError {
    (
        StatusCode::BAD_REQUEST,
        Json(DatabaseFailure {
            code: "executionFailed",
        }),
    )
}

pub(super) fn status(status: StatusCode) -> SqlError {
    let code = match status {
        StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => "invalidRequest",
        StatusCode::GONE => "deleted",
        StatusCode::UNAUTHORIZED => "unauthorized",
        StatusCode::FORBIDDEN => "forbidden",
        StatusCode::NOT_FOUND => "notFound",
        StatusCode::CONFLICT => "localCopyFenced",
        StatusCode::PAYLOAD_TOO_LARGE => "limitExceeded",
        _ => "unavailable",
    };
    (status, Json(DatabaseFailure { code }))
}

fn runtime_error(error: std::io::Error) -> SqlError {
    status(if error.kind() == std::io::ErrorKind::PermissionDenied {
        StatusCode::CONFLICT
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    })
}

#[utoipa::path(post, path = "/storage/databases/{database_id}/sql",
    params(("database_id" = String, Path, description = "Database resource ID")),
    request_body = SqlRequest,
    responses((status = 200, description = "Complete atomic SQL batch", body = SqlResponse),
        (status = 400, description = "Invalid SQL, parameters or execution failure", body = DatabaseFailure),
        (status = 401, description = "Invalid database authorization", body = DatabaseFailure),
        (status = 403, description = "Parsed actions denied", body = DatabaseFailure),
        (status = 404, description = "Resource absent or deleted", body = DatabaseFailure),
        (status = 409, description = "Local copy fenced", body = DatabaseFailure),
        (status = 413, description = "Request or result limit exceeded", body = DatabaseFailure),
        (status = 503, description = "Content or authorization unavailable", body = DatabaseFailure)),
    security(("authorization" = [])))]
pub(super) async fn execute_sql(
    State(state): State<ResourceState>,
    headers: HeaderMap,
    Path(id): Path<DatabaseId>,
    request: Result<Json<SqlRequest>, JsonRejection>,
) -> Result<Json<SqlResponse>, SqlError> {
    let (scope, can_read, can_write) = namespace(
        authorization(&state, &headers).await.map_err(status)?,
        "storage:databases",
    )
    .map_err(status)?;
    let Json(request) = request.map_err(|error| {
        if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
            status(StatusCode::PAYLOAD_TOO_LARGE)
        } else {
            invalid()
        }
    })?;
    if request.statements.is_empty() || request.statements.len() > 32 {
        return Err(invalid());
    }
    let mut statements = Vec::new();
    let mut reads = false;
    let mut writes = false;
    for entry in request.statements {
        if entry.sql.len() > 16 * 1024 || entry.params.len() > 128 {
            return Err(invalid());
        }
        let params = QueryParams::Positional(
            entry
                .params
                .into_iter()
                .map(SqlValue::into_owner)
                .collect::<Result<_, _>>()?,
        );
        let translated = SqlTranslator
            .translate_with_params(&entry.sql, Some(&params))
            .await
            .map_err(|_| invalid())?;
        for statement in translated {
            if matches!(statement, Statement::Query(Query::Select(_))) {
                reads = true;
            } else {
                writes = true;
            }
            statements.push(statement);
        }
        if statements.len() > 32 {
            return Err(invalid());
        }
    }
    if statements.is_empty() {
        return Err(invalid());
    }
    if (reads && !can_read) || (writes && !can_write) {
        return Err(status(StatusCode::FORBIDDEN));
    }
    let runtime = state
        .databases
        .as_ref()
        .ok_or_else(|| status(StatusCode::SERVICE_UNAVAILABLE))?;
    let database = runtime
        .open_existing(&scope, id)
        .map_err(runtime_error)?
        .ok_or_else(|| status(StatusCode::NOT_FOUND))?;
    if reads {
        authorize_response(&state, &headers, &scope, "storage:databases", false)
            .await
            .map_err(status)?;
    }
    if writes {
        authorize_response(&state, &headers, &scope, "storage:databases", true)
            .await
            .map_err(status)?;
    }
    let client = database.client();
    let mut transaction = client.transaction().await.map_err(|_| failed())?;
    runtime
        .require_local_copy(&scope, id)
        .map_err(runtime_error)?;
    if runtime.get(&scope, id).map_err(runtime_error)?.is_none() {
        return Err(status(StatusCode::NOT_FOUND));
    }
    let results = transaction
        .execute(statements)
        .await
        .map_err(|_| failed())?;
    if results
        .iter()
        .map(|result| result.rows.len())
        .sum::<usize>()
        > 1000
    {
        return Err(status(StatusCode::PAYLOAD_TOO_LARGE));
    }
    let response = SqlResponse {
        results: results
            .into_iter()
            .map(|result| {
                Ok(SqlResult {
                    columns: result
                        .columns
                        .into_iter()
                        .map(|column| column.name)
                        .collect(),
                    rows: result
                        .rows
                        .into_iter()
                        .map(|row| row.values.into_iter().map(SqlValue::from_owner).collect())
                        .collect::<Result<_, _>>()?,
                })
            })
            .collect::<Result<_, SqlError>>()?,
    };
    if serde_json::to_vec(&response).map_err(|_| failed())?.len() > 1024 * 1024 {
        return Err(status(StatusCode::PAYLOAD_TOO_LARGE));
    }
    runtime
        .require_local_copy(&scope, id)
        .map_err(runtime_error)?;
    if runtime.get(&scope, id).map_err(runtime_error)?.is_none() {
        return Err(status(StatusCode::NOT_FOUND));
    }
    transaction.commit().await.map_err(|_| failed())?;
    if reads {
        authorize_response(&state, &headers, &scope, "storage:databases", false)
            .await
            .map_err(status)?;
    }
    if writes {
        authorize_response(&state, &headers, &scope, "storage:databases", true)
            .await
            .map_err(status)?;
    }
    runtime
        .require_local_copy(&scope, id)
        .map_err(runtime_error)?;
    if runtime.get(&scope, id).map_err(runtime_error)?.is_none() {
        return Err(status(StatusCode::NOT_FOUND));
    }
    Ok(Json(response))
}
