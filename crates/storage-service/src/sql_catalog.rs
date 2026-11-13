use std::io;

use of_model::idp::model::Id;
use ofdb_sql::{Client, Transaction, decode};

use super::{Catalog, CatalogRecord, DatabaseResource};

fn worker<R: Send>(work: impl FnOnce() -> io::Result<R> + Send) -> io::Result<R> {
    std::thread::scope(|scope| {
        scope
            .spawn(work)
            .join()
            .map_err(|_| io::Error::other("Storage catalog worker panicked"))?
    })
}

pub fn edit<R: Send>(
    client: Client,
    owner: String,
    application: Id,
    write: bool,
    edit: impl FnOnce(&mut Catalog) -> io::Result<R> + Send,
) -> io::Result<R> {
    worker(move || {
        tokio::runtime::Builder::new_current_thread().enable_all().build()?.block_on(async move {
            let mut namespace = Id::new_v5(&application, owner.as_bytes());
            let mut transaction = client.transaction().await.map_err(|error| io::Error::other(error.to_string()))?;
            let namespaces = transaction.execute_sql(&format!("SELECT id FROM namespaces WHERE owner_subject = '{}' AND application_id = CAST('{application}' AS UUID)", owner.replace('\'', "''")), None).await.map_err(|error| io::Error::other(error.to_string()))?;
            if let Some(row) = namespaces[0].rows.first() { namespace = decode(&row.values[0], "id").map_err(|error| io::Error::other(error.to_string()))?; }
            let rows = transaction.execute_sql(&format!("SELECT id, name, deleted FROM resources WHERE namespace_id = CAST('{namespace}' AS UUID) AND kind = 'database'"), None).await.map_err(|error| io::Error::other(error.to_string()))?;
            let mut catalog = Catalog::default();
            for row in &rows[0].rows {
                let id = decode(&row.values[0], "id").map_err(|error| io::Error::other(error.to_string()))?;
                let databases = transaction.execute_sql(&format!("SELECT initializing FROM databases WHERE resource_id = CAST('{id}' AS UUID)"), None).await.map_err(|error| io::Error::other(error.to_string()))?;
                let metadata = databases[0].rows.first().ok_or_else(|| io::Error::other("database metadata is missing"))?;
                catalog.records.insert(id, CatalogRecord {
                    resource: DatabaseResource { id, name: decode(&row.values[1], "name").map_err(|error| io::Error::other(error.to_string()))? },
                    deleted: decode(&row.values[2], "deleted").map_err(|error| io::Error::other(error.to_string()))?,
                    initializing: decode(&metadata.values[0], "initializing").map_err(|error| io::Error::other(error.to_string()))?,
                });
            }
            let result = edit(&mut catalog)?;
            if write {
                persist(&mut transaction, namespace, &owner, application, &catalog).await?;
                transaction.commit().await.map_err(|error| io::Error::other(error.to_string()))?;
            }
            Ok(result)
        })
    })
}

async fn persist(
    transaction: &mut Transaction,
    namespace: Id,
    owner: &str,
    application: Id,
    catalog: &Catalog,
) -> io::Result<()> {
    let owner = owner.replace('\'', "''");
    let rows = transaction
        .execute_sql(
            &format!("SELECT id FROM namespaces WHERE id = CAST('{namespace}' AS UUID)"),
            None,
        )
        .await
        .map_err(|error| io::Error::other(error.to_string()))?;
    if rows[0].rows.is_empty() {
        transaction.execute_sql(&format!("INSERT INTO namespaces (id, owner_subject, application_id) VALUES (CAST('{namespace}' AS UUID), '{owner}', CAST('{application}' AS UUID))"), None).await.map_err(|error| io::Error::other(error.to_string()))?;
    }
    for record in catalog.records.values() {
        let id = record.resource.id;
        let name = record
            .resource
            .name
            .as_ref()
            .map(|name| format!("'{}'", name.replace('\'', "''")))
            .unwrap_or_else(|| "NULL".into());
        let existing = transaction
            .execute_sql(
                &format!(
                    "SELECT namespace_id, kind FROM resources WHERE id = CAST('{id}' AS UUID)"
                ),
                None,
            )
            .await
            .map_err(|error| io::Error::other(error.to_string()))?;
        if let Some(row) = existing[0].rows.first() {
            let current: Id = decode(&row.values[0], "namespace_id")
                .map_err(|error| io::Error::other(error.to_string()))?;
            let kind: String = decode(&row.values[1], "kind")
                .map_err(|error| io::Error::other(error.to_string()))?;
            if current != namespace || kind != "database" {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "resource belongs to another namespace",
                ));
            }
            transaction.execute_sql(&format!("UPDATE resources SET name = {name}, deleted = {} WHERE id = CAST('{id}' AS UUID)", record.deleted), None).await.map_err(|error| io::Error::other(error.to_string()))?;
            transaction.execute_sql(&format!("UPDATE databases SET initializing = {} WHERE resource_id = CAST('{id}' AS UUID)", record.initializing), None).await.map_err(|error| io::Error::other(error.to_string()))?;
        } else {
            transaction.execute_sql(&format!("INSERT INTO resources (id, namespace_id, kind, name, deleted) VALUES (CAST('{id}' AS UUID), CAST('{namespace}' AS UUID), 'database', {name}, {})", record.deleted), None).await.map_err(|error| io::Error::other(error.to_string()))?;
            transaction.execute_sql(&format!("INSERT INTO databases (id, resource_id, initializing) VALUES (CAST('{id}' AS UUID), CAST('{id}' AS UUID), {})", record.initializing), None).await.map_err(|error| io::Error::other(error.to_string()))?;
        }
    }
    Ok(())
}

pub fn owner_resources(client: Client, owner: String) -> io::Result<Vec<(Id, Id)>> {
    worker(move || {
        tokio::runtime::Builder::new_current_thread().enable_all().build()?.block_on(async move {
            let owner = owner.replace('\'', "''");
            let mut transaction = client.transaction().await.map_err(|error| io::Error::other(error.to_string()))?;
            let namespaces = transaction.execute_sql(&format!("SELECT id, application_id FROM namespaces WHERE owner_subject = '{owner}'"), None).await.map_err(|error| io::Error::other(error.to_string()))?;
            let mut resources = Vec::new();
            for row in &namespaces[0].rows {
                let namespace: Id = decode(&row.values[0], "id").map_err(|error| io::Error::other(error.to_string()))?;
                let application: Id = decode(&row.values[1], "application_id").map_err(|error| io::Error::other(error.to_string()))?;
                let rows = transaction.execute_sql(&format!("SELECT id FROM resources WHERE namespace_id = CAST('{namespace}' AS UUID) AND kind = 'database'"), None).await.map_err(|error| io::Error::other(error.to_string()))?;
                for row in &rows[0].rows { resources.push((application, decode(&row.values[0], "id").map_err(|error| io::Error::other(error.to_string()))?)); }
            }
            Ok(resources)
        })
    })
}
