use crate::db::Database;

pub const MIGRATION: &str = include_str!("../migrations/0001_init.up.sql");

pub async fn up(database: &dyn Database) -> Result<(), ofdb_sql::QueryError> {
    migrate(&database.client(), MIGRATION).await
}

pub async fn up_device(client: &ofdb_sql::Client) -> Result<(), ofdb_sql::QueryError> {
    migrate(client, MIGRATION).await
}

async fn migrate(
    client: &ofdb_sql::Client,
    migration: &str,
) -> Result<(), ofdb_sql::QueryError> {
    let mut transaction = client.transaction().await?;
    let result = async {
        for statement in migration
            .split(';')
            .map(str::trim)
            .filter(|statement| !statement.is_empty())
        {
            transaction
                .translate_and_execute(statement, &ofdb_sql::SqlTranslator)
                .await?;
        }
        Ok(())
    }
    .await;
    match result {
        Ok(()) => transaction.commit().await,
        Err(error) => {
            transaction.rollback().await?;
            Err(error)
        }
    }
}
