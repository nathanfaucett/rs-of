use of_model::id::Id;
use of_model::management_error::{ManagementError, ManagementResult};
use ofdb_sql::{Client, QueryError, QueryParams, QueryResult, Value};

pub fn policy_insert(
    table: &str,
    application_id: Id,
    name: &str,
    description: Option<&str>,
    timestamp: i64,
) -> (String, QueryParams) {
    (
        format!(
            "INSERT INTO {table} (application_id, name, description, created_at, updated_at) VALUES ($1, $2, $3, $4, $4) RETURNING id"
        ),
        QueryParams::Positional(vec![
            Value::Uuid(application_id),
            Value::Text(name.into()),
            description.map_or(Value::Null, |text| Value::Text(text.into())),
            Value::Integer(timestamp),
        ]),
    )
}

pub async fn insert_policy_record(
    client: &Client,
    table: &str,
    application_id: Id,
    name: &str,
    description: Option<&str>,
    timestamp: i64,
) -> ManagementResult<Id> {
    let (sql, params) = policy_insert(table, application_id, name, description, timestamp);
    returned_id(
        &client
            .execute_sql(&sql, Some(&params))
            .await
            .map_err(db_error)?,
    )
}

pub fn returned_id(results: &[QueryResult]) -> ManagementResult<Id> {
    match results.first().and_then(|result| result.rows.first()) {
        Some(row) if let [Value::Uuid(id)] = row.values.as_slice() => Ok(*id),
        _ => Err(ManagementError::InvalidInput("missing generated ID".into())),
    }
}

pub fn db_error(error: QueryError) -> ManagementError {
    ManagementError::InvalidInput(error.to_string())
}
