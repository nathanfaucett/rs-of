use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use engine::{
    Engine, EngineError, InMemoryKernel, RowReconciler, TableStateView, catalog_table_for_storage,
};
use engine_automerge::AutomergeRowCodec;
use history::{DEFAULT_MAX_SESSION_BYTES, SyncKey, SyncStateUnit, export_sync_state_bounded_for};
use schema::TableSchema;
use value::Value;

use super::{Database, DatabaseBackend, conflict_stage};
use crate::Error;
use crate::error::from_engine;

/// A table whose sole primary key is its application record identity.
///
/// `envelope` lists the non-key columns that must come from one live source row together.
/// Include all fields needed to interpret or authenticate the encrypted value, not only ciphertext.
#[derive(Clone, Copy, Debug)]
pub struct HistoryRecord<'a> {
    pub table: &'a str,
    pub identity: &'a str,
    pub envelope: &'a [&'a str],
}

/// An immutable, private reconciliation result. It is not a publication token.
pub struct StagedHistory(Vec<SyncStateUnit>);

impl fmt::Debug for StagedHistory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StagedHistory")
            .field("unit_count", &self.0.len())
            .finish()
    }
}

impl StagedHistory {
    /// Returns reconciled catalog and row history, including retained tombstones.
    #[must_use]
    pub fn units(&self) -> &[SyncStateUnit] {
        &self.0
    }
}

impl Database {
    /// Captures each database separately and reconciles the snapshots in private memory.
    /// Neither source is written, including on failure. Both must have exactly the listed active
    /// tables with matching schemas. Each snapshot and the result have the default history session
    /// payload limit. This limit does not bound decoded memory.
    ///
    /// The runner's `_ofdb_migrations` rows must have identical history keys, state, and metadata
    /// in both snapshots. Staging does not coalesce independently applied versions, even if their
    /// descriptions and fingerprints match. It does not validate migration plans.
    ///
    /// This checks retained application IDs and the provenance of visible encrypted field tuples,
    /// not ciphertext authenticity, remote authorization, or source freshness.
    /// It cannot approve publication. A publisher must still compare against both sources under
    /// their write fences. Only use this with databases obtained through trusted or authorized paths.
    ///
    /// # Errors
    ///
    /// Rejects invalid policy, incompatible schemas or migration history, unverifiable identities,
    /// lost rows or tombstones, and live protected tuples that were not present in either live source.
    /// Split envelopes with unresolved protected column conflicts are also rejected.
    pub async fn stage_history(
        &self,
        other: &Self,
        records: &[HistoryRecord<'_>],
    ) -> Result<StagedHistory, Error> {
        conflict_stage(|| async {
            let records = record_policy(records)?;
            let left = self.staging_snapshot().await?;
            let right = other.staging_snapshot().await?;
            reconcile(left, right, &records).await
        })
        .await
        .map_err(from_engine)
    }

    async fn staging_snapshot(&self) -> Result<Vec<SyncStateUnit>, EngineError> {
        match &self.backend {
            DatabaseBackend::InMemory(engine) => {
                conflict_stage(|| export_sync_state_bounded_for(engine, DEFAULT_MAX_SESSION_BYTES))
                    .await
            }
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            DatabaseBackend::IndexedDb(engine) => {
                conflict_stage(|| export_sync_state_bounded_for(engine, DEFAULT_MAX_SESSION_BYTES))
                    .await
            }
            #[cfg(feature = "redb")]
            DatabaseBackend::File(resource) => {
                let engine = resource.read().await.map_err(EngineError::custom)?;
                conflict_stage(|| {
                    export_sync_state_bounded_for(&*engine, DEFAULT_MAX_SESSION_BYTES)
                })
                .await
            }
        }
    }
}

type Policy<'a> = BTreeMap<&'a str, HistoryRecord<'a>>;
type Stage = Engine<InMemoryKernel, AutomergeRowCodec>;
type Claims = BTreeMap<SyncKey, Claim>;

struct Claim {
    identity: Value,
    envelope: Option<Vec<Value>>,
}

fn record_policy<'a>(records: &[HistoryRecord<'a>]) -> Result<Policy<'a>, EngineError> {
    let mut policy = BTreeMap::new();
    for record in records {
        let columns = record.envelope.iter().copied().collect::<BTreeSet<_>>();
        if record.table.is_empty()
            || catalog_table_for_storage(record.table).is_some()
            || record.identity.is_empty()
            || columns.is_empty()
            || columns.contains("")
            || columns.contains(record.identity)
            || columns.len() != record.envelope.len()
            || policy.insert(record.table, *record).is_some()
        {
            return Err(EngineError::InvalidQuery("Invalid history record policy"));
        }
    }
    if policy.is_empty() {
        return Err(EngineError::InvalidQuery("History record policy is empty"));
    }
    Ok(policy)
}

async fn reconcile(
    left: Vec<SyncStateUnit>,
    right: Vec<SyncStateUnit>,
    policy: &Policy<'_>,
) -> Result<StagedHistory, EngineError> {
    if !migration_units(&left).eq(migration_units(&right)) {
        return Err(EngineError::InvalidQuery("Incompatible migration history"));
    }
    let left_keys = history_keys(&left);
    let right_keys = history_keys(&right);
    let stage = Engine::new(InMemoryKernel::new(), AutomergeRowCodec::new());
    let other = Engine::new(InMemoryKernel::new(), AutomergeRowCodec::new());
    conflict_stage(|| history::apply_sync_state_batch_for(&stage, left)).await?;
    conflict_stage(|| history::apply_sync_state_batch_for(&other, right.clone())).await?;
    let (schemas, left) = conflict_stage(|| observe(&stage, policy, &left_keys)).await?;
    let (right_schemas, right_claims) =
        conflict_stage(|| observe(&other, policy, &right_keys)).await?;
    require_same_schema(&schemas, &right_schemas)?;
    drop(other);

    conflict_stage(|| history::apply_sync_state_batch_for(&stage, right)).await?;
    let keys = left_keys.union(&right_keys).cloned().collect();
    let (merged_schemas, merged) = conflict_stage(|| observe(&stage, policy, &keys)).await?;
    require_same_schema(&schemas, &merged_schemas)?;
    for (key, claim) in &merged {
        verify_claim(claim, left.get(key), right_claims.get(key))?;
    }
    let units =
        conflict_stage(|| export_sync_state_bounded_for(&stage, DEFAULT_MAX_SESSION_BYTES)).await?;
    if history_keys(&units) != keys {
        return Err(EngineError::InvalidQuery(
            "Stage did not retain all history keys",
        ));
    }
    Ok(StagedHistory(units))
}

fn migration_units(units: &[SyncStateUnit]) -> impl Iterator<Item = &SyncStateUnit> {
    units.iter().filter(|unit| {
        let SyncKey::Row { table, .. } = &unit.key;
        table == "_ofdb_migrations"
    })
}

fn history_keys(units: &[SyncStateUnit]) -> BTreeSet<SyncKey> {
    units.iter().map(|unit| unit.key.clone()).collect()
}

fn require_same_schema(left: &[TableSchema], right: &[TableSchema]) -> Result<(), EngineError> {
    if left != right {
        return Err(EngineError::InvalidQuery(
            "Incompatible history table schemas",
        ));
    }
    Ok(())
}

async fn observe(
    stage: &Stage,
    policy: &Policy<'_>,
    keys: &BTreeSet<SyncKey>,
) -> Result<(Vec<TableSchema>, Claims), EngineError> {
    if !conflict_stage(|| stage.table_names())
        .await?
        .iter()
        .map(String::as_str)
        .eq(policy.keys().copied())
    {
        return Err(EngineError::InvalidQuery(
            "History tables do not match record policy",
        ));
    }
    let mut schemas = Vec::new();
    let mut claims = Claims::new();
    for record in policy.values() {
        let schema = conflict_stage(|| stage.table_schema(record.table)).await?;
        if !schema
            .columns
            .iter()
            .filter(|column| column.primary_key)
            .map(|column| column.name.as_str())
            .eq([record.identity])
        {
            return Err(EngineError::InvalidQuery(
                "History identity is not the sole primary key",
            ));
        }
        let table = schema.name.clone();
        let view = stage
            .read_transaction(|codec, tx| {
                Box::pin(
                    async move { conflict_stage(|| codec.observe_table_state(tx, &table)).await },
                )
            })
            .await?;
        collect_claims(record, view, &mut claims)?;
        schemas.push(schema);
    }
    if !keys
        .iter()
        .filter(|key| {
            let SyncKey::Row { table, .. } = key;
            catalog_table_for_storage(table).is_none()
        })
        .eq(claims.keys())
    {
        return Err(EngineError::InvalidQuery(
            "History row has no verifiable record state",
        ));
    }
    Ok((schemas, claims))
}

fn collect_claims(
    record: &HistoryRecord<'_>,
    view: TableStateView,
    claims: &mut Claims,
) -> Result<(), EngineError> {
    let position = |name| {
        view.columns
            .iter()
            .position(|column| column == name)
            .ok_or(EngineError::InvalidQuery("History record column is absent"))
    };
    let identity = position(record.identity)?;
    let envelope = record
        .envelope
        .iter()
        .map(|name| position(*name))
        .collect::<Result<Vec<_>, _>>()?;
    let mut seen = BTreeSet::new();
    for row in view.rows {
        let id = row.row.values[identity].clone();
        if id == Value::Null
            || row
                .conflicts
                .iter()
                .any(|(column, _)| column == record.identity)
        {
            return Err(EngineError::InvalidQuery(
                "Unverifiable history record identity",
            ));
        }
        if !seen.insert(id.clone()) {
            return Err(EngineError::InvalidQuery(
                "History record identity collision",
            ));
        }
        let envelope = if row.deleted {
            None
        } else {
            if envelope.len() > 1
                && row
                    .conflicts
                    .iter()
                    .any(|(column, _)| record.envelope.contains(&column.as_str()))
            {
                return Err(EngineError::InvalidQuery(
                    "Split history envelope has a column conflict",
                ));
            }
            Some(
                envelope
                    .iter()
                    .map(|index| row.row.values[*index].clone())
                    .collect(),
            )
        };
        claims.insert(
            SyncKey::Row {
                table: view.name.clone(),
                row: row.id,
            },
            Claim {
                identity: id,
                envelope,
            },
        );
    }
    Ok(())
}

fn verify_claim(
    merged: &Claim,
    left: Option<&Claim>,
    right: Option<&Claim>,
) -> Result<(), EngineError> {
    let sources = [left, right];
    if sources
        .into_iter()
        .flatten()
        .any(|source| source.identity != merged.identity)
    {
        return Err(EngineError::InvalidQuery(
            "Logical Row changed history record identity",
        ));
    }
    let deleted = sources
        .into_iter()
        .flatten()
        .any(|source| source.envelope.is_none());
    if deleted != merged.envelope.is_none() {
        return Err(EngineError::InvalidQuery(
            "History record deletion did not match source tombstones",
        ));
    }
    if !sources
        .into_iter()
        .flatten()
        .any(|source| source.envelope == merged.envelope)
    {
        return Err(EngineError::InvalidQuery(
            "History envelope did not come from one source row",
        ));
    }
    Ok(())
}
