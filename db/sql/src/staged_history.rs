use core::fmt;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use engine::{
    Engine, EngineError, InMemoryKernel, MutationSummary, RowCodec, RowReconciler, TableStateView,
    Uuid, catalog_table_for_storage,
};
use engine_automerge::AutomergeRowCodec;
use history::{DEFAULT_MAX_SESSION_BYTES, SyncKey, SyncStateUnit, export_sync_state_bounded_for};
use schema::{IndexSchema, TableSchema};
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
    /// Additional non-null columns whose values must never change (for example, a vault ID).
    pub stable: &'a [&'a str],
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

    /// Reopens the candidate in a new, disposable in-memory database for application checks.
    /// It has no binding and cannot change either captured source.
    ///
    /// # Errors
    ///
    /// Returns an error if the candidate cannot be materialized.
    pub async fn into_memory_database(self) -> Result<Database, Error> {
        let engine = Engine::new(InMemoryKernel::new(), AutomergeRowCodec::new());
        conflict_stage(|| history::apply_sync_state_batch_for(&engine, self.0))
            .await
            .map_err(from_engine)?;
        Ok(Database {
            backend: DatabaseBackend::InMemory(engine),
            changes: None,
        })
    }
}

impl Database {
    /// Captures each database separately and reconciles the snapshots in private memory.
    /// Neither source is written, including on failure. Both must have exactly the listed active
    /// tables with matching schemas. Each snapshot and the result have the default history session
    /// payload limit. This limit does not bound decoded memory.
    ///
    /// The runner's `_ofdb_migrations` rows must have identical history or be unedited,
    /// independently applied copies of the same versions, descriptions and fingerprints.
    /// The stage marks redundant copies deleted without removing their history. The caller must
    /// still verify the ledger against its expected migration plan.
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
    stable: Vec<Value>,
    envelope: Option<Vec<Value>>,
}

fn record_policy<'a>(records: &[HistoryRecord<'a>]) -> Result<Policy<'a>, EngineError> {
    let mut policy = BTreeMap::new();
    for record in records {
        let columns = record.envelope.iter().copied().collect::<BTreeSet<_>>();
        let stable = record.stable.iter().copied().collect::<BTreeSet<_>>();
        if record.table.is_empty()
            || catalog_table_for_storage(record.table).is_some()
            || record.identity.is_empty()
            || columns.is_empty()
            || columns.contains("")
            || columns.contains(record.identity)
            || columns.len() != record.envelope.len()
            || stable.len() != record.stable.len()
            || stable.contains("")
            || stable.contains(record.identity)
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
    let left: Arc<[SyncStateUnit]> = left.into();
    let right: Arc<[SyncStateUnit]> = right.into();
    let left_keys = history_keys(&left);
    let right_keys = history_keys(&right);
    let stage = Engine::new(InMemoryKernel::new(), AutomergeRowCodec::new());
    let other = Engine::new(InMemoryKernel::new(), AutomergeRowCodec::new());
    conflict_stage(|| history::apply_sync_state_batch_for(&stage, left.to_vec())).await?;
    conflict_stage(|| history::apply_sync_state_batch_for(&other, right.to_vec())).await?;
    let (schemas, left_claims) =
        conflict_stage(|| observe(&stage, policy, &left_keys, Some(&left))).await?;
    let (right_schemas, right_claims) =
        conflict_stage(|| observe(&other, policy, &right_keys, Some(&right))).await?;
    require_same_schema(&schemas, &right_schemas)?;
    let redundant = redundant_migrations(&left, &right, &stage, &other).await?;
    drop(other);

    conflict_stage(|| history::apply_sync_state_batch_for(&stage, right.to_vec())).await?;
    let keys = left_keys.union(&right_keys).cloned().collect();
    let (merged_schemas, merged) = conflict_stage(|| observe(&stage, policy, &keys, None)).await?;
    require_same_schema(&schemas, &merged_schemas)?;
    for (key, claim) in &merged {
        verify_claim(claim, left_claims.get(key), right_claims.get(key))?;
    }
    if !redundant.is_empty() {
        suppress_migrations(&stage, redundant).await?;
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

const MIGRATIONS: &str = "_ofdb_migrations";
const INCOMPATIBLE_MIGRATIONS: EngineError =
    EngineError::InvalidQuery("Incompatible migration history");

fn migration_units(units: &[SyncStateUnit]) -> impl Iterator<Item = &SyncStateUnit> + Clone {
    units.iter().filter(|unit| {
        let SyncKey::Row { table, .. } = &unit.key;
        table == MIGRATIONS
    })
}

async fn redundant_migrations(
    left: &[SyncStateUnit],
    right: &[SyncStateUnit],
    stage: &Stage,
    other: &Stage,
) -> Result<Vec<Uuid>, EngineError> {
    let left = migration_units(left);
    let right = migration_units(right);
    if left.clone().eq(right.clone()) {
        return Ok(Vec::new());
    }
    let left = migration_definitions(stage, left).await?;
    let right = migration_definitions(other, right).await?;
    if !left.keys().eq(right.keys()) {
        return Err(INCOMPATIBLE_MIGRATIONS);
    }
    let mut redundant = Vec::new();
    for (version, (definition, first)) in left {
        let (other_definition, second) = &right[&version];
        if &definition != other_definition || (first.key == second.key && first != *second) {
            return Err(INCOMPATIBLE_MIGRATIONS);
        }
        if first.key != second.key {
            let SyncKey::Row { row, .. } = (&first.key).max(&second.key);
            redundant.push(*row);
        }
    }
    Ok(redundant)
}

type MigrationDefinitions<'a> = BTreeMap<u32, ((String, Vec<u8>), &'a SyncStateUnit)>;

async fn migration_definitions<'a>(
    stage: &Stage,
    units: impl Iterator<Item = &'a SyncStateUnit>,
) -> Result<MigrationDefinitions<'a>, EngineError> {
    let units = units
        .map(|unit| {
            let SyncKey::Row { row, .. } = &unit.key;
            (*row, unit)
        })
        .collect::<BTreeMap<_, _>>();
    let view = stage
        .read_transaction(|codec, tx| {
            Box::pin(
                async move { conflict_stage(|| codec.observe_table_state(tx, MIGRATIONS)).await },
            )
        })
        .await?;
    if view.columns
        != [
            "id",
            "version",
            "description",
            "fingerprint",
            "execution_time",
        ]
        || units.len() != view.rows.len()
    {
        return Err(INCOMPATIBLE_MIGRATIONS);
    }
    let mut definitions = BTreeMap::new();
    for row in view.rows {
        let unit = *units.get(&row.id).ok_or(INCOMPATIBLE_MIGRATIONS)?;
        let [
            Value::Uuid(_),
            Value::Integer(version),
            Value::Text(description),
            Value::Blob(fingerprint),
            Value::Integer(elapsed),
        ] = row.row.values.as_slice()
        else {
            return Err(INCOMPATIBLE_MIGRATIONS);
        };
        let version = u32::try_from(*version).map_err(|_error| INCOMPATIBLE_MIGRATIONS)?;
        if row.deleted
            || !row.conflicts.is_empty()
            || !unit.metadata.is_empty()
            || !AutomergeRowCodec::is_unedited_insert(&unit.state, &view.columns)?
            || version == 0
            || fingerprint.len() != 32
            || *elapsed < 0
            || definitions
                .insert(version, ((description.clone(), fingerprint.clone()), unit))
                .is_some()
        {
            return Err(INCOMPATIBLE_MIGRATIONS);
        }
    }
    Ok(definitions)
}

async fn suppress_migrations(stage: &Stage, redundant: Vec<Uuid>) -> Result<(), EngineError> {
    let tables = [MIGRATIONS.to_string()];
    conflict_stage(|| {
        stage.mutate_tables(&tables, |codec, tx| {
            Box::pin(async move {
                for row in redundant {
                    if codec.delete_row(tx, MIGRATIONS, &row).await?.is_none() {
                        return Err(INCOMPATIBLE_MIGRATIONS);
                    }
                }
                let mut mutations = MutationSummary::default();
                mutations.record_table(MIGRATIONS);
                Ok(((), mutations))
            })
        })
    })
    .await
}

fn history_keys(units: &[SyncStateUnit]) -> BTreeSet<SyncKey> {
    units.iter().map(|unit| unit.key.clone()).collect()
}

type Layout = (Vec<TableSchema>, Vec<IndexSchema>);

fn require_same_schema(left: &Layout, right: &Layout) -> Result<(), EngineError> {
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
    source: Option<&Arc<[SyncStateUnit]>>,
) -> Result<(Layout, Claims), EngineError> {
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
    let names = policy.keys().copied().collect::<Vec<_>>();
    let (schemas, indices, _) = conflict_stage(|| stage.observe_schema(&names, None)).await?;
    let mut claims = Claims::new();
    for (record, schema) in policy.values().zip(&schemas) {
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
        let source = source.cloned();
        let columns = schema
            .columns
            .iter()
            .map(|column| {
                (
                    column.name.clone(),
                    column.r#type,
                    column.default.clone(),
                    column.name == record.identity || record.stable.contains(&column.name.as_str()),
                )
            })
            .collect::<Vec<_>>();
        let view = stage
            .read_transaction(|codec, tx| {
                Box::pin(async move {
                    if let Some(units) = source {
                        for unit in units.iter() {
                            let SyncKey::Row { table: name, row } = &unit.key;
                            if name == &table {
                                conflict_stage(|| {
                                    codec.inspect_history(
                                        tx,
                                        &table,
                                        *row,
                                        &unit.state,
                                        &unit.metadata,
                                        &columns,
                                    )
                                })
                                .await
                                .map_err(|_error| {
                                    EngineError::InvalidQuery("Unverifiable stable record history")
                                })?;
                            }
                        }
                    }
                    conflict_stage(|| codec.observe_table_state(tx, &table)).await
                })
            })
            .await?;
        collect_claims(record, view, &mut claims)?;
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
    Ok(((schemas, indices), claims))
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
    let stable = record
        .stable
        .iter()
        .map(|name| position(*name))
        .collect::<Result<Vec<_>, _>>()?;
    let envelope = record
        .envelope
        .iter()
        .map(|name| position(*name))
        .collect::<Result<Vec<_>, _>>()?;
    let mut seen = BTreeSet::new();
    for row in view.rows {
        let id = row.row.values[identity].clone();
        let stable = stable
            .iter()
            .map(|index| row.row.values[*index].clone())
            .collect::<Vec<_>>();
        if id == Value::Null
            || stable.contains(&Value::Null)
            || row.conflicts.iter().any(|(column, _)| {
                column == record.identity || record.stable.contains(&column.as_str())
            })
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
                stable,
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
        .any(|source| source.identity != merged.identity || source.stable != merged.stable)
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
