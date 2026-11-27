use std::collections::BTreeMap;
use std::time::Duration;

use ofdb_kv::{encode_value, parse_value};
use ofdb_sql::{Client, Database, QueryParams, QueryResult, Transaction};
use serde_json::Value as Json;
use wasm_bindgen::JsError;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::prelude::*;

use crate::value::{parse_json, write_json};

#[wasm_bindgen]
pub struct SqlDatabase {
    database: Database,
}

/// Subscription lifetime; freeing this handle stops wakeups.
#[wasm_bindgen]
pub struct SqlCommitWatch(futures_util::future::AbortHandle);

impl Drop for SqlCommitWatch {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[wasm_bindgen]
impl SqlDatabase {
    /// Open a SQL database backed by IndexedDB.
    pub async fn open(name: String) -> Result<SqlDatabase, JsError> {
        let database = Database::open_indexeddb(&name).await?;
        Ok(Self { database })
    }

    /// Create a query handle sharing this database's storage.
    pub fn client(&self) -> SqlClient {
        SqlClient {
            client: self.database.client(),
        }
    }

    /// Observe coalesced physical commits without retaining the SQL database.
    pub fn watch_commits(&self, listener: js_sys::Function) -> Result<SqlCommitWatch, JsError> {
        let mut subscription = self
            .database
            .subscribe_commits()
            .ok_or_else(|| JsError::new("SQL commit notifications require IndexedDB"))?;
        let (handle, registration) = futures_util::future::AbortHandle::new_pair();
        wasm_bindgen_futures::spawn_local(async move {
            let task = async move {
                while subscription.changed().await {
                    let _ = listener.call0(&JsValue::UNDEFINED);
                }
            };
            let _ = futures_util::future::Abortable::new(task, registration).await;
        });
        Ok(SqlCommitWatch(handle))
    }

    /// Get the change subscription for push-based synchronization.
    /// The subscription provides a stream of committed changes via `events()`.
    #[cfg(feature = "sync")]
    pub fn sync_changes(&self) -> Result<crate::sync::SqlChangeSubscription, JsError> {
        let subscription = self.database.sync_changes()?;
        Ok(crate::sync::SqlChangeSubscription::new(subscription))
    }

    /// Push-based synchronization with a remote peer.
    /// Streams local changes to peer and applies remote changes.
    #[cfg(feature = "sync")]
    pub async fn synchronize(
        &self,
        transport: &mut crate::sync::JsPushSyncTransport,
        config: crate::sync::SqlSessionConfig,
    ) -> Result<crate::sync::PushSynchronizeResult, crate::sync::JsPushSyncError> {
        self.database
            .synchronize(transport, config.into())
            .await
            .map_err(crate::sync::JsPushSyncError::from)
            .map(crate::sync::PushSynchronizeResult::from)
    }

    /// Exchange SQL history v7 over caller-supplied byte frames with bounded memory staging.
    #[cfg(feature = "sync")]
    pub async fn synchronize_history(
        &self,
        transport: &mut crate::sync::JsSqlSyncTransport,
        role: crate::sql_history::SqlHistoryRole,
        can_apply: Option<js_sys::Function>,
    ) -> Result<crate::sql_history::SqlHistoryResult, crate::sql_history::JsSqlHistoryError> {
        self.database
            .exchange_resource_history_memory_checked(
                transport,
                &ofdb_sql::history::SessionConfig::default(),
                role.into(),
                || check_history_apply(can_apply.as_ref()),
            )
            .await
            .map_err(crate::sql_history::JsSqlHistoryError::from)
            .map(crate::sql_history::SqlHistoryResult::from)
    }
    /// Reconcile two snapshots into a disposable in-memory database without writing either source.
    #[cfg(feature = "sync")]
    pub async fn stage_history(
        &self,
        other: &SqlDatabase,
        policy: String,
    ) -> Result<SqlDatabase, JsError> {
        type Policy = Vec<(String, String, Vec<String>, Vec<String>)>;
        let policy: Policy = serde_json::from_str(&policy)
            .map_err(|_error| JsError::new("Invalid history record policy"))?;
        let columns = policy
            .iter()
            .map(|(_, _, stable, envelope)| {
                (
                    stable.iter().map(String::as_str).collect::<Vec<_>>(),
                    envelope.iter().map(String::as_str).collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();
        let records = policy
            .iter()
            .zip(&columns)
            .map(
                |((table, identity, _, _), (stable, envelope))| ofdb_sql::HistoryRecord {
                    table,
                    identity,
                    stable,
                    envelope,
                },
            )
            .collect::<Vec<_>>();
        let staged = self
            .database
            .stage_history(&other.database, &records)
            .await?;
        Ok(Self {
            database: staged.into_memory_database().await?,
        })
    }

    /// Opens a disposable database for a read-only Pull or candidate validation.
    #[cfg(feature = "sync")]
    pub fn in_memory() -> Self {
        Self {
            database: Database::in_memory(),
        }
    }

    #[cfg(feature = "sync")]
    pub async fn history_revision(&self) -> Result<Vec<u8>, JsError> {
        Ok(self.database.history_revision().await?.to_vec())
    }

    /// Changes local history and saves the first replica tag together, after remote receipt confirmation.
    #[cfg(feature = "sync")]
    pub async fn publish_local_history(
        &self,
        staged: &Self,
        expected: Vec<u8>,
        candidate: Vec<u8>,
        tag: Vec<u8>,
        can_apply: Option<js_sys::Function>,
    ) -> Result<(), JsError> {
        let invalid_revision = |_| JsError::new("Expected a 32-byte history revision");
        self.database
            .publish_local_history(
                &staged.database,
                expected.try_into().map_err(invalid_revision)?,
                candidate.try_into().map_err(invalid_revision)?,
                tag,
                || check_history_apply(can_apply.as_ref()),
            )
            .await
            .map_err(|error| JsError::new(&error.to_string()))
    }

    /// Preflight only. The restore transaction checks the catalog again.
    #[cfg(feature = "sync")]
    pub async fn initial_restore_is_eligible(&self) -> Result<bool, JsError> {
        Ok(self.database.initial_restore_is_eligible().await?)
    }

    /// Reads the local-only tag saved with this restore.
    #[cfg(feature = "sync")]
    pub async fn initial_restore_tag(&self) -> Result<Option<Vec<u8>>, JsError> {
        Ok(self.database.initial_restore_tag().await?)
    }

    /// Replaces the local-only replica tag if the previous bytes match.
    #[cfg(feature = "sync")]
    pub async fn replace_initial_restore_tag(
        &self,
        expected: Option<Vec<u8>>,
        tag: Vec<u8>,
    ) -> Result<bool, JsError> {
        Ok(self
            .database
            .replace_initial_restore_tag(expected, tag)
            .await?)
    }

    /// Receives SQL history without sending local history, then publishes if still pristine.
    #[cfg(feature = "sync")]
    pub async fn restore_history(
        &self,
        transport: &mut crate::sync::JsSqlSyncTransport,
        signal: js_sys::Object,
        tag: Vec<u8>,
        can_apply: Option<js_sys::Function>,
    ) -> Result<(), crate::sql_history::JsSqlHistoryError> {
        self.database
            .restore_resource_history_memory(
                transport,
                &ofdb_sql::history::SessionConfig::default(),
                tag,
                || match js_sys::Reflect::get(&signal, &JsValue::from_str("aborted")) {
                    Ok(value) if value.as_bool() == Some(false) => {
                        check_history_apply(can_apply.as_ref())
                    }
                    _ => Err("restore cancelled".into()),
                },
            )
            .await
            .map_err(crate::sql_history::JsSqlHistoryError::from)
    }
}

#[cfg(feature = "sync")]
fn check_history_apply(can_apply: Option<&js_sys::Function>) -> Result<(), String> {
    if let Some(check) = can_apply
        && check
            .call0(&JsValue::UNDEFINED)
            .ok()
            .and_then(|value| value.as_bool())
            != Some(true)
    {
        return Err("local history apply cancelled".into());
    }
    Ok(())
}

#[wasm_bindgen]
pub struct SqlClient {
    client: Client,
}

#[wasm_bindgen]
impl SqlClient {
    /// Apply the complete application migration history through the Rust runner.
    pub async fn migrate(&self, migrations: String) -> Result<String, JsValue> {
        crate::sql_migration::migrate(&self.client, &migrations).await
    }

    /// Execute SQL text with optional bound parameters and return results as JSON.
    ///
    /// Parameters are a JSON array for positional placeholders or a JSON object for
    /// named placeholders. Every value uses the owner's lossless tagged codec.
    /// The result is a JSON array of `{ columns, rows }` objects with tagged cells.
    pub async fn execute(&self, sql: String, params: Option<String>) -> Result<String, JsError> {
        let params = match params {
            Some(params) => Some(parse_params(&params)?),
            None => None,
        };
        let results = self.client.execute_sql(&sql, params.as_ref()).await?;
        write_json(&results_to_json(&results))
    }

    /// Start a transaction that holds statements until commit or rollback.
    pub async fn transaction(&self) -> Result<SqlTransaction, JsError> {
        let tx = self.client.transaction().await?;
        Ok(SqlTransaction { tx })
    }

    /// Return a clone of this client with a request deadline in milliseconds.
    pub fn with_deadline(&self, ms: u64) -> SqlClient {
        SqlClient {
            client: self.client.clone().with_deadline(Duration::from_millis(ms)),
        }
    }

    /// Return an independent clone sharing the same storage.
    pub fn clone_client(&self) -> SqlClient {
        SqlClient {
            client: self.client.clone(),
        }
    }
}

#[wasm_bindgen]
pub struct SqlTransaction {
    tx: Transaction,
}

#[wasm_bindgen]
impl SqlTransaction {
    /// Execute SQL text within the transaction.
    pub async fn execute(
        &mut self,
        sql: String,
        params: Option<String>,
    ) -> Result<String, JsError> {
        let params = match params {
            Some(params) => Some(parse_params(&params)?),
            None => None,
        };
        let results = self.tx.execute_sql(&sql, params.as_ref()).await?;
        write_json(&results_to_json(&results))
    }

    pub async fn commit(self) -> Result<(), JsError> {
        self.tx.commit().await?;
        Ok(())
    }

    pub async fn rollback(self) -> Result<(), JsError> {
        self.tx.rollback().await?;
        Ok(())
    }
}

fn results_to_json(results: &[QueryResult]) -> Json {
    Json::Array(
        results
            .iter()
            .map(|result| {
                let columns = result
                    .columns
                    .iter()
                    .map(|column| Json::String(column.name.clone()))
                    .collect::<Vec<_>>();
                let rows = result
                    .rows
                    .iter()
                    .map(|row| Json::Array(row.values.iter().cloned().map(encode_value).collect()))
                    .collect::<Vec<_>>();
                Json::Object(
                    [
                        ("columns".to_owned(), Json::Array(columns)),
                        ("rows".to_owned(), Json::Array(rows)),
                    ]
                    .into_iter()
                    .collect(),
                )
            })
            .collect(),
    )
}

/// Parse bound parameters supplied as JSON by the TypeScript layer.
fn parse_params(text: &str) -> Result<QueryParams, JsError> {
    match parse_json(text)? {
        Json::Array(values) => Ok(QueryParams::Positional(
            values
                .iter()
                .map(parse_value)
                .collect::<Result<_, _>>()
                .map_err(|error| JsError::new(&error))?,
        )),
        Json::Object(values) => Ok(QueryParams::Named(
            values
                .iter()
                .map(|(name, value)| Ok((name.clone(), parse_value(value)?)))
                .collect::<Result<BTreeMap<_, _>, String>>()
                .map_err(|error| JsError::new(&error))?,
        )),
        other => Err(JsError::new(&format!(
            "query parameters must be a JSON array or object, got {other}"
        ))),
    }
}
