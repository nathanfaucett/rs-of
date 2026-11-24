use std::{collections::BTreeSet, io};

use of_model::{id::Id, storage::database_resource::DatabaseResource};
use ofdb_sql::{Client, QueryParams, Transaction, Value, decode};

use super::{Catalog, CatalogRecord};
use crate::persistence::{error, run};

#[allow(
    clippy::significant_drop_tightening,
    reason = "Transaction::commit consumes and drops the transaction"
)]
pub fn edit<R: Send>(
    client: &Client,
    owner: &str,
    application: Id,
    write: bool,
    edit: impl FnOnce(&mut Catalog) -> io::Result<R> + Send,
) -> io::Result<R> {
    run(async move {
        let scope = [Value::Text(owner.to_owned()), Value::Uuid(application)];
        let mut transaction = client.transaction().await.map_err(error)?;
        let rows = transaction
            .execute_sql(
                "SELECT resource_id, name, initializing, deleted FROM databases WHERE owner_subject = $1 AND application_id = $2 ORDER BY id DESC",
                Some(&QueryParams::Positional(scope.to_vec())),
            )
            .await
            .map_err(error)?;
        let mut catalog = Catalog::default();
        for row in &rows[0].rows {
            let id = decode(&row.values[0], "resource_id").map_err(error)?;
            let name = decode(&row.values[1], "name").map_err(error)?;
            let initializing: bool = decode(&row.values[2], "initializing").map_err(error)?;
            let deleted: bool = decode(&row.values[3], "deleted").map_err(error)?;
            // Independent registrations have different Logical Row IDs. Keep all of
            // them visible so deletion is absorbing and cannot be shadowed by a new row.
            let record = catalog.records.entry(id).or_insert_with(|| CatalogRecord {
                resource: DatabaseResource { id, name: None },
                deleted: false,
                initializing: true,
            });
            if record.resource.name.is_none() {
                record.resource.name = name;
            }
            record.initializing &= initializing;
            record.deleted |= deleted;
        }
        if !write {
            drop(transaction);
            return edit(&mut catalog);
        }
        let before = catalog.records.clone();
        let result = edit(&mut catalog)?;
        for (id, record) in &catalog.records {
            persist(&mut transaction, &scope, record, before.get(id)).await?;
        }
        transaction.commit().await.map_err(error)?;
        Ok(result)
    })
}

async fn persist(
    transaction: &mut Transaction,
    scope: &[Value; 2],
    record: &CatalogRecord,
    before: Option<&CatalogRecord>,
) -> io::Result<()> {
    let mut params = scope.to_vec();
    params.push(Value::Uuid(record.resource.id));
    let sql = match before {
        None => {
            params.extend([
                record
                    .resource
                    .name
                    .clone()
                    .map_or(Value::Null, Value::Text),
                Value::Bool(record.initializing),
                Value::Bool(record.deleted),
            ]);
            "INSERT INTO databases (owner_subject, application_id, resource_id, name, initializing, deleted) VALUES ($1, $2, $3, $4, $5, $6)"
        }
        Some(before) if !before.deleted && record.deleted => {
            "UPDATE databases SET deleted = TRUE WHERE owner_subject = $1 AND application_id = $2 AND resource_id = $3"
        }
        Some(before) if before.initializing && !record.initializing => {
            "UPDATE databases SET initializing = FALSE WHERE owner_subject = $1 AND application_id = $2 AND resource_id = $3"
        }
        Some(_) => return Ok(()),
    };
    transaction
        .execute_sql(sql, Some(&QueryParams::Positional(params)))
        .await
        .map_err(error)?;
    Ok(())
}

pub fn owner_resources(client: &Client, owner: &str) -> io::Result<Vec<(Id, Id)>> {
    run(async {
        let rows = client
            .execute_sql(
                "SELECT application_id, resource_id FROM databases WHERE owner_subject = $1",
                Some(&QueryParams::Positional(vec![Value::Text(
                    owner.to_owned(),
                )])),
            )
            .await
            .map_err(error)?;
        let mut resources = BTreeSet::new();
        for row in &rows[0].rows {
            resources.insert((
                decode(&row.values[0], "application_id").map_err(error)?,
                decode(&row.values[1], "resource_id").map_err(error)?,
            ));
        }
        Ok(resources.into_iter().collect())
    })
}
