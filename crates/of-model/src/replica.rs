use crate::db::Database;

pub const MIGRATION: &str = include_str!("../replica_migrations/0001_init.up.sql");
pub const DEVICE_MIGRATION: &str = include_str!("../device_migrations/0001_init.up.sql");

pub async fn up(database: &dyn Database) -> Result<(), db::QueryError> {
    migrate(&database.client(), MIGRATION).await
}

pub async fn up_device(client: &db::Client) -> Result<(), db::QueryError> {
    migrate(client, DEVICE_MIGRATION).await
}

async fn migrate(client: &db::Client, migration: &str) -> Result<(), db::QueryError> {
    let mut transaction = client.transaction().await?;
    let result = async {
        for statement in migration
            .split(';')
            .map(str::trim)
            .filter(|statement| !statement.is_empty())
        {
            transaction
                .translate_and_execute(statement, &db::SqlTranslator)
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
