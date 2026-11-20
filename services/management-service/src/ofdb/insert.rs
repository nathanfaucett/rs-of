use of_model::id::Id;
use of_model::management_error::{ManagementError, ManagementResult};
use ofdb_sql::{Client, QueryError, QueryParams, QueryResult, Value};

pub async fn insert_policy_record(
    client: &Client,
    table: &str,
    domain_id: Option<Id>,
    application_id: Id,
    name: &str,
    description: Option<&str>,
    timestamp: i64,
) -> ManagementResult<(Id, Id)> {
    let mut tx = client.transaction().await.map_err(db_error)?;
    let row_id = returned_id(
        &tx.execute_sql(
            &format!("INSERT INTO {table} (id, application_id, name, description, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $5) RETURNING row_id"),
            Some(&QueryParams::Positional(vec![
                domain_id.map_or(Value::Null, Value::Uuid),
                Value::Uuid(application_id),
                Value::Text(name.into()),
                description.map_or(Value::Null, |text| Value::Text(text.into())),
                Value::Integer(timestamp),
            ])),
        )
        .await
        .map_err(db_error)?,
    )?;
    // Ordinary creates publish the generated ID. Set it before commit so all
    // public IDs, including retained provisioning IDs, share one unique index.
    if domain_id.is_none() {
        tx.execute_sql(
            &format!("UPDATE {table} SET id = $1 WHERE row_id = $1"),
            Some(&QueryParams::Positional(vec![Value::Uuid(row_id)])),
        )
        .await
        .map_err(db_error)?;
    }
    tx.commit().await.map_err(db_error)?;
    Ok((domain_id.unwrap_or(row_id), row_id))
}

pub fn returned_id(results: &[QueryResult]) -> ManagementResult<Id> {
    match results.first().and_then(|result| result.rows.first()) {
        Some(row) if let [Value::Uuid(id)] = row.values.as_slice() => Ok(*id),
        _ => Err(ManagementError::InvalidInput("missing generated ID".into())),
    }
}

fn db_error(error: QueryError) -> ManagementError {
    ManagementError::InvalidInput(error.to_string())
}
