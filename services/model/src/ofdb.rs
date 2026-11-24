use include_dir::{Dir, include_dir};
#[cfg(feature = "ofdb")]
use ofdb_sql::{Client, QueryError, QueryErrorKind, SqlTranslator};

pub static MIGRATIONS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/migrations");

/// Applies the complete shared schema in filename order.
///
/// # Errors
/// Returns an error if the migration SQL is invalid or cannot be applied.
#[cfg(feature = "ofdb")]
pub async fn up(client: &Client) -> Result<(), QueryError> {
    migrate(client, None).await
}

/// Applies only the Device schema to a standalone owner.
///
/// # Errors
/// Returns an error if the Device schema is missing, invalid, or cannot be applied.
#[cfg(feature = "ofdb")]
pub async fn up_device(client: &Client) -> Result<(), QueryError> {
    migrate(client, Some("device")).await
}

/// Applies only the database catalog schema to a standalone Storage owner.
///
/// # Errors
/// Returns an error if the catalog schema is missing, invalid, or cannot be applied.
#[cfg(feature = "ofdb")]
pub async fn up_storage_catalog(client: &Client) -> Result<(), QueryError> {
    migrate(client, Some("storage_catalog")).await
}

#[cfg(feature = "ofdb")]
#[allow(
    clippy::significant_drop_tightening,
    reason = "commit and rollback consume the transaction"
)]
async fn migrate(client: &Client, schema: Option<&str>) -> Result<(), QueryError> {
    let mut files: Vec<_> = MIGRATIONS
        .files()
        .filter(|file| {
            file.path()
                .to_str()
                .is_some_and(|path| path.ends_with(".up.sql"))
        })
        .collect();
    files.sort_unstable_by_key(|file| file.path());

    let mut statements = Vec::new();
    for file in files {
        let sql = file.contents_utf8().ok_or_else(|| {
            QueryError::new(QueryErrorKind::Rejected, "migration SQL must be UTF-8")
        })?;
        // Named blocks let standalone stores use the same SQL without creating other owners' tables.
        let mut sections = sql.split("-- schema: ");
        if let Some(sql) = sections.next()
            && schema.is_none()
        {
            statements.extend(sql_statements(sql));
        }
        for section in sections {
            if let Some((name, sql)) = section.split_once('\n')
                && schema.is_none_or(|schema| schema == name.trim())
            {
                statements.extend(sql_statements(sql));
            }
        }
    }
    if statements.is_empty() {
        return Err(QueryError::new(
            QueryErrorKind::Rejected,
            "migration schema is empty",
        ));
    }

    let mut transaction = client.transaction().await?;
    for statement in statements {
        if let Err(error) = transaction
            .translate_and_execute(statement, &SqlTranslator)
            .await
        {
            transaction.rollback().await?;
            return Err(error);
        }
    }
    transaction.commit().await
}

#[cfg(feature = "ofdb")]
fn sql_statements(sql: &str) -> impl Iterator<Item = &str> {
    sql.split(';').map(str::trim).filter(|sql| !sql.is_empty())
}
