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
use history::{
    DEFAULT_MAX_SESSION_BYTES, HistoryRevision, SyncError, SyncKey, SyncStateUnit,
    export_sync_state_bounded_for, history_revision,
};
use platform::MaybeSend;
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
    /// Empty to reject split conflicts. Otherwise the first field selects the unique source
    /// with the canonical value. These fields must be written together in each live source.
    /// All envelope fields, including any companions outside this list, come from that source.
    pub resolve_by: &'a [&'a str],
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
    /// Reads the exact bounded history revision from one transaction.
    /// # Errors
    /// Returns an error if the snapshot fails or exceeds the history budget.
    pub async fn history_revision(&self) -> Result<HistoryRevision, Error> {
        let units = self.staging_snapshot().await.map_err(from_engine)?;
        Ok(history_revision(&units))
    }

    /// Applies the exact private candidate if local history still matches `expected` and no
    /// replica tag exists. Saves `tag` with the history. This does not check the remote receipt;
    /// the caller must confirm that receipt and fence any running synchronization first.
    /// # Errors
    /// Returns an error for a changed history or tag, invalid stage, cancellation or failed commit.
    pub async fn publish_local_history<Q>(
        &self,
        staged: &Self,
        expected: HistoryRevision,
        candidate: HistoryRevision,
        tag: Vec<u8>,
        check: Q,
    ) -> Result<(), SyncError<core::convert::Infallible>>
    where
        Q: FnOnce() -> Result<(), String> + MaybeSend,
    {
        let stage = match &staged.backend {
            DatabaseBackend::InMemory(stage) => stage,
            #[cfg(any(
                feature = "redb",
                all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown")
            ))]
            _ => {
                return Err(EngineError::Unsupported(
                    "local publication requires an in-memory candidate",
                )
                .into());
            }
        };
        match &self.backend {
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            DatabaseBackend::IndexedDb(engine) => {
                conflict_stage(|| {
                    history::publish_local_history_for(
                        engine,
                        stage,
                        DEFAULT_MAX_SESSION_BYTES,
                        expected,
                        candidate,
                        tag,
                        check,
                    )
                })
                .await
            }
            DatabaseBackend::InMemory(engine) => {
                conflict_stage(|| {
                    history::publish_local_history_for(
                        engine,
                        stage,
                        DEFAULT_MAX_SESSION_BYTES,
                        expected,
                        candidate,
                        tag,
                        check,
                    )
                })
                .await
            }
            #[cfg(feature = "redb")]
            DatabaseBackend::File(_) => Err(EngineError::Unsupported(
                "local publication requires an optimistic backend",
            )
            .into()),
        }
    }

    /// Captures each database separately and reconciles the snapshots in private memory.
    /// Neither source is written, including on failure. Both must have exactly the listed active
    /// tables with matching schemas. Each snapshot and the result have the default history session
    /// payload limit. This limit does not bound decoded memory.
    ///
    /// The runner's `_ofdb_migrations` rows must have identical history or be unedited,
    /// independently applied copies of the same versions, descriptions and fingerprints.
    /// The stage marks redundant copies deleted without removing their history. Previously staged
    /// redundant tombstones are accepted only with the same canonical live copy. The caller must
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
    /// Split envelopes with conflicts require an explicit resolution policy and a unique,
    /// complete source for the canonical value. Resolution retains both sources' native changes.
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
    envelope_conflicted: bool,
}

fn record_policy<'a>(records: &[HistoryRecord<'a>]) -> Result<Policy<'a>, EngineError> {
    let mut policy = BTreeMap::new();
    for record in records {
        let columns = record.envelope.iter().copied().collect::<BTreeSet<_>>();
        let stable = record.stable.iter().copied().collect::<BTreeSet<_>>();
        let resolve_by = record.resolve_by.iter().copied().collect::<BTreeSet<_>>();
        if record.table.is_empty()
            || catalog_table_for_storage(record.table).is_some()
            || record.identity.is_empty()
            || columns.is_empty()
            || columns.contains("")
            || columns.contains(record.identity)
            || columns.len() != record.envelope.len()
            || (!resolve_by.is_empty()
                && (resolve_by.len() < 2
                    || resolve_by.len() != record.resolve_by.len()
                    || !resolve_by.is_subset(&columns)
                    || !columns.is_disjoint(&stable)))
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
    verify_source_groups(&left, &left_claims, policy)?;
    verify_source_groups(&right, &right_claims, policy)?;
    let mut redundant = redundant_migrations(&left, &right, &stage, &other).await?;
    drop(other);

    conflict_stage(|| history::apply_sync_state_batch_for(&stage, right.to_vec())).await?;
    let keys = left_keys.union(&right_keys).cloned().collect();
    let (merged_schemas, merged) = conflict_stage(|| observe(&stage, policy, &keys, None)).await?;
    require_same_schema(&schemas, &merged_schemas)?;
    let mut resolutions = Vec::new();
    for (key, claim) in &merged {
        let SyncKey::Row { table, .. } = key;
        let record = &policy[table.as_str()];
        if let Some(values) =
            verify_claim(record, claim, left_claims.get(key), right_claims.get(key))?
        {
            let assignments = record
                .envelope
                .iter()
                .map(|name| (*name).to_owned())
                .zip(values.iter().cloned())
                .collect();
            resolutions.push((key.clone(), assignments));
        }
    }
    resolve_groups(&stage, resolutions).await?;
    redundant.retain(|row| {
        merged
            .get(&SyncKey::Row {
                table: MIGRATIONS.into(),
                row: *row,
            })
            .is_some_and(|claim| claim.envelope.is_some())
    });
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
    for (version, (definition, first, left_rows)) in left {
        let (other_definition, second, right_rows) = &right[&version];
        if &definition != other_definition
            || left_rows.iter().any(|(key, unit)| {
                right_rows
                    .get(key)
                    .is_some_and(|other| unit.state != other.state)
            })
        {
            return Err(INCOMPATIBLE_MIGRATIONS);
        }
        if first.key != second.key {
            let SyncKey::Row { row, .. } = (&first.key).max(&second.key);
            redundant.push(*row);
        }
    }
    Ok(redundant)
}

type MigrationRows<'a> = BTreeMap<SyncKey, &'a SyncStateUnit>;
type MigrationDefinitions<'a> =
    BTreeMap<u32, ((String, Vec<u8>), &'a SyncStateUnit, MigrationRows<'a>)>;

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
        if !row.conflicts.is_empty()
            || (!row.deleted && !unit.metadata.is_empty())
            || !AutomergeRowCodec::is_unedited_insert(&unit.state, &view.columns)?
            || version == 0
            || fingerprint.len() != 32
            || *elapsed < 0
        {
            return Err(INCOMPATIBLE_MIGRATIONS);
        }
        let definition = (description.clone(), fingerprint.clone());
        let (existing, live, rows) = definitions
            .entry(version)
            .or_insert_with(|| (definition.clone(), None, MigrationRows::new()));
        if *existing != definition || (!row.deleted && live.replace(unit).is_some()) {
            return Err(INCOMPATIBLE_MIGRATIONS);
        }
        rows.insert(unit.key.clone(), unit);
    }
    // A previous stage can leave redundant untouched inserts as tombstones. The same
    // canonical minimum must still be the sole live copy on that side.
    definitions
        .into_iter()
        .map(|(version, (definition, live, rows))| {
            let live = live.ok_or(INCOMPATIBLE_MIGRATIONS)?;
            if rows.first_key_value().map(|(key, _)| key) != Some(&live.key) {
                return Err(INCOMPATIBLE_MIGRATIONS);
            }
            Ok((version, (definition, live, rows)))
        })
        .collect()
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
        let envelope_conflicted = !row.deleted
            && envelope.len() > 1
            && row
                .conflicts
                .iter()
                .any(|(column, _)| record.envelope.contains(&column.as_str()));
        let envelope = (!row.deleted).then(|| {
            envelope
                .iter()
                .map(|index| row.row.values[*index].clone())
                .collect()
        });
        claims.insert(
            SyncKey::Row {
                table: view.name.clone(),
                row: row.id,
            },
            Claim {
                identity: id,
                stable,
                envelope,
                envelope_conflicted,
            },
        );
    }
    Ok(())
}

const SPLIT_CONFLICT: EngineError =
    EngineError::InvalidQuery("Incompatible split history envelope: column conflict");

fn verify_source_groups(
    units: &[SyncStateUnit],
    claims: &Claims,
    policy: &Policy<'_>,
) -> Result<(), EngineError> {
    for unit in units {
        let Some(claim) = claims.get(&unit.key) else {
            continue;
        };
        if claim.envelope_conflicted {
            return Err(SPLIT_CONFLICT);
        }
        let SyncKey::Row { table, .. } = &unit.key;
        let record = &policy[table.as_str()];
        if let Some(values) = &claim.envelope
            && !record.resolve_by.is_empty()
        {
            if values.contains(&Value::Null) {
                return Err(EngineError::InvalidQuery(
                    "Incomplete source history envelope",
                ));
            }
            AutomergeRowCodec::verify_history_group(&unit.state, record.resolve_by)?;
        }
    }
    Ok(())
}

async fn resolve_groups(
    stage: &Stage,
    resolutions: Vec<(SyncKey, Vec<(String, Value)>)>,
) -> Result<(), EngineError> {
    if resolutions.is_empty() {
        return Ok(());
    }
    let units = stage
        .read_transaction(|codec, tx| {
            Box::pin(async move {
                let mut units = Vec::with_capacity(resolutions.len());
                for (key, assignments) in resolutions {
                    let SyncKey::Row { table, row } = &key;
                    let snapshot = codec
                        .resolve_history_group(tx, table, row, &assignments)
                        .await?;
                    let metadata = codec.export_metadata(tx, table, *row).await?;
                    units.push(SyncStateUnit::new(key, snapshot, metadata));
                }
                Ok(units)
            })
        })
        .await?;
    conflict_stage(|| history::apply_sync_state_batch_for(stage, units)).await
}

fn verify_claim<'a>(
    record: &HistoryRecord<'_>,
    merged: &Claim,
    left: Option<&'a Claim>,
    right: Option<&'a Claim>,
) -> Result<Option<&'a [Value]>, EngineError> {
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
    if merged.envelope_conflicted {
        let Some(selector) = record.resolve_by.first() else {
            return Err(SPLIT_CONFLICT);
        };
        let index = record
            .envelope
            .iter()
            .position(|column| column == selector)
            .expect("record policy validates the selector column");
        let canonical = &merged
            .envelope
            .as_ref()
            .expect("live conflict has an envelope")[index];
        let mut matches = sources.into_iter().flatten().filter_map(|source| {
            source
                .envelope
                .as_deref()
                .filter(|values| &values[index] == canonical)
        });
        if let (Some(winner), None) = (matches.next(), matches.next()) {
            return Ok(Some(winner));
        }
        return Err(EngineError::InvalidQuery(
            "Ambiguous canonical source history envelope",
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
    Ok(None)
}
