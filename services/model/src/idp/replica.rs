//! Replica modules for `IdP` owner
//!
//! Provides schema migration functions for the IDP and Management owners.

#[cfg(feature = "ofdb")]
use crate::ofdb::MIGRATION;
#[cfg(feature = "ofdb")]
use ofdb_sql::{Client, QueryError, SqlTranslator};

/// Runs the IDP owner migration (identity schema) against the given SQL client.
///
/// # Errors
/// Returns a query error if transaction creation, migration execution, commit, or rollback fails.
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
    if let Err(error) = result {
        transaction.rollback().await?;
        return Err(error);
    }
    transaction.commit().await
}

/// Runs the Management owner migration (policy schema) against the given SQL client.
///
/// # Errors
/// Returns a query error if transaction creation, migration execution, commit, or rollback fails.
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
    if let Err(error) = result {
        transaction.rollback().await?;
        return Err(error);
    }
    transaction.commit().await
}
