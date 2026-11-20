use std::collections::BTreeMap;

use of_model::id::Id;
use of_model::management_error::{ManagementError, ManagementResult};
use ofdb_sql::{
    Client, FromRow, QueryError, QueryParams, QueryResult, Row, Transaction, Value, value,
};

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
    let result = async {
    let revocations = revocations(&mut tx, table).await?;
    let incarnation = domain_id.map_or(Ok(0), |id| next_incarnation(&revocations, application_id, id))?;
    let rows = tx.execute_sql(&format!("SELECT id, application_id, incarnation, name FROM {table} WHERE application_id = $1 AND name = $2 AND revoked_at IS NULL"), Some(&QueryParams::Positional(vec![Value::Uuid(application_id), Value::Text(name.into())]))).await.map_err(db_error)?;
    for row in &rows[0].rows {
        if is_current(&revocations, row, &["id", "application_id", "incarnation", "name"])? {
            return Err(ManagementError::InvalidInput("policy name already exists".into()));
        }
    }
    let row_id = returned_id(
        &tx.execute_sql(
            &format!("INSERT INTO {table} (id, application_id, name, description, created_at, updated_at, incarnation) VALUES ($1, $2, $3, $4, $5, $5, $6) RETURNING row_id"),
            Some(&QueryParams::Positional(vec![
                domain_id.map_or(Value::Null, Value::Uuid),
                Value::Uuid(application_id),
                Value::Text(name.into()),
                description.map_or(Value::Null, |text| Value::Text(text.into())),
                Value::Integer(timestamp),
                Value::Integer(incarnation),
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
    Ok((domain_id.unwrap_or(row_id), row_id))
    }.await;
    if result.is_ok() {
        tx.commit().await.map_err(db_error)?;
    } else {
        tx.rollback().await.map_err(db_error)?;
    }
    result
}

// Grants identify the public policy incarnation, never its physical row.
pub fn grant_ref(application: Id, id: Id, incarnation: i64) -> Id {
    let mut scoped = [0; 24];
    scoped[..16].copy_from_slice(application.as_bytes());
    scoped[16..].copy_from_slice(&incarnation.to_be_bytes());
    Id::new_v5(&id, &scoped)
}

type Revocations = BTreeMap<(Id, Id), i64>;

async fn revocations(tx: &mut Transaction, table: &str) -> ManagementResult<Revocations> {
    let rows = tx.execute_sql("SELECT application_id, policy_id, incarnation FROM rbac_revocations WHERE policy_kind = $1", Some(&QueryParams::Positional(vec![Value::Text(table.into())]))).await.map_err(db_error)?;
    let mut revoked = Revocations::new();
    for row in &rows[0].rows {
        let [
            Value::Uuid(application),
            Value::Uuid(id),
            Value::Integer(incarnation),
        ] = row.values.as_slice()
        else {
            return Err(ManagementError::InvalidInput(
                "invalid policy revocation".into(),
            ));
        };
        if *incarnation < 0 {
            return Err(ManagementError::InvalidInput(
                "invalid policy incarnation".into(),
            ));
        }
        revoked
            .entry((*application, *id))
            .and_modify(|current| *current = (*current).max(*incarnation))
            .or_insert(*incarnation);
    }
    Ok(revoked)
}

fn next_incarnation(revoked: &Revocations, application: Id, id: Id) -> ManagementResult<i64> {
    revoked
        .get(&(application, id))
        .copied()
        .unwrap_or(-1)
        .checked_add(1)
        .ok_or_else(|| ManagementError::InvalidInput("policy incarnation exhausted".into()))
}

fn is_current(revoked: &Revocations, row: &Row, columns: &[&str]) -> ManagementResult<bool> {
    let get = |name| {
        value(row, columns, name).map_err(|error| ManagementError::InvalidInput(error.to_string()))
    };
    let (Value::Uuid(app), Value::Uuid(id), Value::Integer(incarnation)) =
        (get("application_id")?, get("id")?, get("incarnation")?)
    else {
        return Err(ManagementError::InvalidInput(
            "invalid policy identity".into(),
        ));
    };
    Ok(*incarnation == next_incarnation(revoked, *app, *id)?)
}

pub async fn active_rows<T: FromRow>(
    client: &Client,
    table: &str,
    columns: &[&str],
) -> ManagementResult<Vec<T>> {
    let mut tx = client.transaction().await.map_err(db_error)?;
    let result = async {
        let rows = tx
            .execute_sql(
                &format!(
                    "SELECT {} FROM {table} WHERE revoked_at IS NULL",
                    columns.join(", ")
                ),
                None,
            )
            .await
            .map_err(db_error)?;
        let revoked = revocations(&mut tx, table).await?;
        let mut active = Vec::new();
        for row in &rows[0].rows {
            if is_current(&revoked, row, columns)? {
                active.push(
                    T::from_row(row, columns)
                        .map_err(|error| ManagementError::InvalidInput(error.to_string()))?,
                );
            }
        }
        Ok(active)
    }
    .await;
    tx.rollback().await.map_err(db_error)?;
    result
}

pub async fn revoke_policy(
    client: &Client,
    table: &str,
    application: Id,
    id: Id,
    incarnation: i64,
    row_id: Id,
) -> ManagementResult<()> {
    client.execute_sql(
        &format!("INSERT INTO rbac_revocations (policy_kind, application_id, policy_id, incarnation) VALUES ($1, $2, $3, $4) ON CONFLICT (policy_kind, application_id, policy_id, incarnation) DO NOTHING; DELETE FROM {table} WHERE row_id = $5"),
        Some(&QueryParams::Positional(vec![Value::Text(table.into()), Value::Uuid(application), Value::Uuid(id), Value::Integer(incarnation), Value::Uuid(row_id)])),
    ).await.map_err(db_error)?;
    Ok(())
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
