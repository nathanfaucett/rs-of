//! Replica modules for `IdP` owner
//!
//! Provides schema migration functions for the IDP and Management owners.

#[cfg(feature = "ofdb")]
use crate::ofdb::MIGRATION;
#[cfg(feature = "ofdb")]
use ofdb_sql::{Client, QueryError, SqlTranslator};

/// Runs the IDP owner migration (identity schema) against the given SQL client.
#[cfg(feature = "ofdb")]
pub async fn up(client: &Client) -> Result<(), QueryError> {
    let mut transaction = client.transaction().await?;
    let result = async {
        for statement in MIGRATION
            .split(';')
            .map(str::trim)
            .filter(|statement| !statement.is_empty())
        {
            transaction
                .translate_and_execute(statement, &SqlTranslator)
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

/// Runs the Management owner migration (policy schema) against the given SQL client.
#[cfg(feature = "ofdb")]
pub async fn up_policy(client: &Client) -> Result<(), QueryError> {
    let mut transaction = client.transaction().await?;
    let result = async {
        for statement in MIGRATION
            .split(';')
            .map(str::trim)
            .filter(|statement| !statement.is_empty())
        {
            transaction
                .translate_and_execute(statement, &SqlTranslator)
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
