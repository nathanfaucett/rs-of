use core::future::ready;

#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeSet, string::String, vec::Vec};
#[cfg(feature = "std")]
use std::collections::BTreeSet;

use platform::{MaybeSend, MaybeSync};

use value::Row;

use crate::{
    Engine, EngineResult, Kernel, KernelTransaction, RowCodec, Uuid,
    executor::execution_stage,
    index::{rebuild_table, update_row},
    schema::{
        active_table_names, active_user_row, catalog_table_for_storage, ensure,
        reconcile_schema_names, validate_row_id,
    },
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MutationSummary {
    changed_tables: BTreeSet<String>,
    catalog_changed: bool,
}

impl MutationSummary {
    pub fn record_table(&mut self, table: &str) {
        if catalog_table_for_storage(table).is_some() {
            self.catalog_changed = true;
        } else {
            self.changed_tables.insert(String::from(table));
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RowMutation {
    pub table: String,
    pub row: Uuid,
    pub old: Option<Row>,
    pub new: Option<Row>,
}

impl<K, R> Engine<K, R>
where
    K: Kernel,
    R: RowCodec<K::Transaction> + MaybeSend + MaybeSync,
{
    /// Runs a read operation in one transaction and rolls it back afterward.
    ///
    /// # Errors
    ///
    /// Returns an error for setup, operation, or rollback failure.
    pub async fn read_transaction<F, O>(&self, operation: F) -> EngineResult<O>
    where
        F: for<'a> FnOnce(&'a R, &'a K::Transaction) -> platform::BoxFuture<'a, EngineResult<O>>,
    {
        let mut transaction = self.kernel.transaction().await?;
        ensure(&mut transaction, self.reconciler.as_ref()).await?;
        let result = operation(self.reconciler.as_ref(), &transaction).await;
        transaction.rollback().await?;
        result
    }

    /// Mutates one row in a transaction.
    ///
    /// # Errors
    ///
    /// Returns an error for validation, operation, index maintenance, or transaction failure.
    pub async fn mutate_transaction<F, O>(
        &self,
        table: &str,
        row: Uuid,
        operation: F,
    ) -> EngineResult<O>
    where
        F: for<'a> FnOnce(
            &'a R,
            &'a mut K::Transaction,
            Option<Row>,
        ) -> platform::BoxFuture<'a, EngineResult<(O, Option<Row>)>>,
    {
        validate_row_id(table, &row)?;
        let mut transaction = self.kernel.transaction().await?;
        let result: EngineResult<O> = async {
            ensure(&mut transaction, self.reconciler.as_ref()).await?;
            self.reconciler
                .ensure_table(&mut transaction, table)
                .await?;
            let old = self.reconciler.get_row(&transaction, table, &row).await?;
            let (output, new) =
                operation(self.reconciler.as_ref(), &mut transaction, old.clone()).await?;
            if catalog_table_for_storage(table).is_none()
                && active_user_row(&transaction, self.reconciler.as_ref(), table, &row).await?
            {
                update_row(
                    &mut transaction,
                    self.reconciler.as_ref(),
                    table,
                    row,
                    old.as_ref(),
                    new.as_ref(),
                    false,
                )
                .await?;
                rebuild_table(&mut transaction, self.reconciler.as_ref(), table, false).await?;
            }
            Ok(output)
        }
        .await;
        match result {
            Ok(output) => {
                transaction.commit().await?;
                Ok(output)
            }
            Err(error) => {
                transaction.rollback().await?;
                Err(error)
            }
        }
    }

    /// Mutates several tables in one transaction.
    ///
    /// # Errors
    ///
    /// Returns an error for setup, operation, schema reconciliation, or commit failure.
    pub async fn mutate_tables<F, O>(&self, tables: &[String], operation: F) -> EngineResult<O>
    where
        F: for<'a> FnOnce(
            &'a R,
            &'a mut K::Transaction,
        ) -> platform::BoxFuture<'a, EngineResult<(O, MutationSummary)>>,
    {
        self.mutate_tables_validated(tables, operation, |_| Ok(()))
            .await
    }

    /// Validate transaction-derived output synchronously before committing mutations.
    /// Rejection rolls back the transaction. Validation must not block or reenter it.
    ///
    /// # Errors
    ///
    /// Returns an error for setup, operation, validation, schema reconciliation, or commit failure.
    pub async fn mutate_tables_validated<F, O, V>(
        &self,
        tables: &[String],
        operation: F,
        validate: V,
    ) -> EngineResult<O>
    where
        F: for<'a> FnOnce(
            &'a R,
            &'a mut K::Transaction,
        ) -> platform::BoxFuture<'a, EngineResult<(O, MutationSummary)>>,
        V: FnOnce(&O) -> EngineResult<()>,
    {
        self.mutate_tables_checked(tables, operation, validate, || ready(Ok(())))
            .await
    }

    /// Runs a final check after mutation, validation, and index maintenance, with the
    /// transaction still open. The check must not depend on this transaction or its writer.
    ///
    /// # Errors
    ///
    /// Rolls back and returns an error if mutation, validation, or the final check fails.
    /// Also returns errors from transaction setup, commit, or rollback.
    pub async fn mutate_tables_checked<F, O, V, Q, U, E>(
        &self,
        tables: &[String],
        operation: F,
        validate: V,
        before_commit: Q,
    ) -> Result<O, E>
    where
        F: for<'a> FnOnce(
            &'a R,
            &'a mut K::Transaction,
        ) -> platform::BoxFuture<'a, EngineResult<(O, MutationSummary)>>,
        V: FnOnce(&O) -> EngineResult<()>,
        Q: FnOnce() -> U,
        U: Future<Output = Result<(), E>>,
        E: From<crate::EngineError>,
    {
        let mut transaction = self.kernel.transaction().await?;
        let result: Result<O, E> = async {
            ensure(&mut transaction, self.reconciler.as_ref()).await?;
            for table in tables {
                self.reconciler
                    .ensure_table(&mut transaction, table)
                    .await?;
            }
            let (output, summary) = operation(self.reconciler.as_ref(), &mut transaction).await?;
            validate(&output)?;
            if summary.catalog_changed || !summary.changed_tables.is_empty() {
                reconcile_schema_names(&mut transaction, self.reconciler.as_ref()).await?;
            }
            let active_tables = active_table_names(&transaction, self.reconciler.as_ref()).await?;
            for table in active_tables {
                if summary.catalog_changed || summary.changed_tables.contains(&table) {
                    self.reconciler
                        .ensure_table(&mut transaction, &table)
                        .await?;
                    execution_stage(|| {
                        rebuild_table(&mut transaction, self.reconciler.as_ref(), &table, false)
                    })
                    .await?;
                }
            }
            before_commit().await?;
            Ok(output)
        }
        .await;
        match result {
            Ok(output) => {
                transaction.commit().await?;
                Ok(output)
            }
            Err(error) => {
                transaction.rollback().await?;
                Err(error)
            }
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "row validation, mutations, and index maintenance share one transaction"
    )]
    /// Mutates several rows in one transaction.
    ///
    /// # Errors
    ///
    /// Returns an error for validation, operation, index maintenance, or transaction failure.
    pub async fn mutate_rows<F, O>(&self, rows: &[(String, Uuid)], operation: F) -> EngineResult<O>
    where
        F: for<'a> FnOnce(
            &'a R,
            &'a mut K::Transaction,
        ) -> platform::BoxFuture<'a, EngineResult<(O, Vec<RowMutation>)>>,
    {
        for (table, row) in rows {
            validate_row_id(table, row)?;
        }
        let mut transaction = self.kernel.transaction().await?;
        let result: EngineResult<O> = async {
            ensure(&mut transaction, self.reconciler.as_ref()).await?;
            for (table, _) in rows {
                self.reconciler
                    .ensure_table(&mut transaction, table)
                    .await?;
            }
            let (output, mutations) = operation(self.reconciler.as_ref(), &mut transaction).await?;
            for mutation in &mutations {
                validate_row_id(&mutation.table, &mutation.row)?;
            }
            let catalog_changed = mutations
                .iter()
                .any(|mutation| catalog_table_for_storage(&mutation.table).is_some());
            if catalog_changed {
                reconcile_schema_names(&mut transaction, self.reconciler.as_ref()).await?;
            }
            for mutation in mutations {
                if catalog_table_for_storage(&mutation.table).is_none()
                    && active_user_row(
                        &transaction,
                        self.reconciler.as_ref(),
                        &mutation.table,
                        &mutation.row,
                    )
                    .await?
                {
                    update_row(
                        &mut transaction,
                        self.reconciler.as_ref(),
                        &mutation.table,
                        mutation.row,
                        mutation.old.as_ref(),
                        mutation.new.as_ref(),
                        false,
                    )
                    .await?;
                }
            }
            let tables = if catalog_changed {
                active_table_names(&transaction, self.reconciler.as_ref()).await?
            } else {
                rows.iter()
                    .map(|(table, _)| table.clone())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect()
            };
            for table in tables {
                if catalog_changed {
                    self.reconciler
                        .ensure_table(&mut transaction, &table)
                        .await?;
                    rebuild_table(&mut transaction, self.reconciler.as_ref(), &table, false)
                        .await?;
                    continue;
                }
                let Some((_, row)) = rows.iter().find(|(row_table, _)| row_table == &table) else {
                    continue;
                };
                if active_user_row(&transaction, self.reconciler.as_ref(), &table, row).await? {
                    self.reconciler
                        .ensure_table(&mut transaction, &table)
                        .await?;
                    rebuild_table(&mut transaction, self.reconciler.as_ref(), &table, false)
                        .await?;
                }
            }
            Ok(output)
        }
        .await;
        match result {
            Ok(output) => {
                transaction.commit().await?;
                Ok(output)
            }
            Err(error) => {
                transaction.rollback().await?;
                Err(error)
            }
        }
    }

    /// Lists active table names.
    ///
    /// # Errors
    ///
    /// Returns an error for setup, catalog reads, or rollback failure.
    pub async fn table_names(&self) -> EngineResult<Vec<String>> {
        let mut transaction = self.kernel.transaction().await?;
        ensure(&mut transaction, self.reconciler.as_ref()).await?;
        let result = active_table_names(&transaction, self.reconciler.as_ref()).await;
        transaction.rollback().await?;
        result
    }
}
