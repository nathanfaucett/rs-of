use core::fmt;

use of_model::device::contract::device_state::DeviceState;
use of_model::device::selection_policy::ResourceSelection;
pub use of_model::device::selection_policy::{
    OwnerPolicyObservation, SelectedResource, SelectionPolicy,
};
use ofdb_sql::Client;
use ofdb_sql::Query;
use ofdb_sql::QueryColumn;
use ofdb_sql::QueryExpr;
use ofdb_sql::QueryExprValue;
use ofdb_sql::QueryFrom;
use ofdb_sql::QueryInsert;
use ofdb_sql::QuerySelect;
use ofdb_sql::QueryUpdate;
use ofdb_sql::QueryUpdateAssignment;
use ofdb_sql::Row;
use ofdb_sql::Statement;
use ofdb_sql::Uuid;
use ofdb_sql::Value;
use ofdb_sql::{QueryParams, Transaction};

use of_model::device_error::DeviceError;

use crate::device_repo::DeviceRepo;

use of_model::device_error::DeviceResult;

use crate::ofdb::device_repo::{DbDeviceRepo, device_key};

const TABLE: &str = "device_selection_policies";
const RESOURCE_TABLE: &str = "device_resource_selections";
const COLUMNS: [&str; 6] = [
    "device_id",
    "owner_subject",
    "application_id",
    "selected_kind",
    "selected_id",
    "admin_allowed",
];

pub struct DbSelectionPolicyRepo {
    engine: Client,
}

impl fmt::Debug for DbSelectionPolicyRepo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DbSelectionPolicyRepo")
            .finish_non_exhaustive()
    }
}

impl DbSelectionPolicyRepo {
    #[must_use]
    pub const fn new(engine: Client) -> Self {
        Self { engine }
    }

    async fn clear(&self, device_id: Uuid) -> DeviceResult<()> {
        if self
            .engine
            .row_conflicts(TABLE, &key(device_id))
            .await
            .map_err(db_error)?
            .is_empty()
        {
            Ok(())
        } else {
            Err(invalid("conflicted selection policy"))
        }
    }

    async fn record(&self, device_id: Uuid) -> DeviceResult<Option<SelectionPolicy>> {
        self.resolve_convergent_conflicts(device_id).await?;
        self.clear(device_id).await?;
        let policy = self.raw_record(device_id).await?;
        self.clear(device_id).await?;
        Ok(policy)
    }

    async fn resolve_convergent_conflicts(&self, device_id: Uuid) -> DeviceResult<()> {
        let conflicts = self
            .engine
            .row_conflict_values(TABLE, &key(device_id))
            .await
            .map_err(db_error)?;
        let mut resolutions = Vec::new();
        for (column, values) in conflicts {
            let resolution = match column.as_str() {
                "application_id" | "selected_kind" | "selected_id"
                    if values.contains(&Value::Null) =>
                {
                    Value::Null
                }
                "admin_allowed" if values.contains(&Value::Bool(false)) => Value::Bool(false),
                _ => return Err(invalid("conflicted selection policy")),
            };
            resolutions.push((column, resolution));
        }
        if !resolutions.is_empty() {
            self.engine
                .resolve_row(TABLE, &key(device_id), resolutions)
                .await
                .map_err(db_error)?;
        }
        Ok(())
    }

    #[allow(
        clippy::significant_drop_tightening,
        reason = "rollback consumes the transaction and releases its resources"
    )]
    async fn resource_selected(
        &self,
        device_id: Uuid,
        owner_subject: &str,
        application_id: Uuid,
        kind: &str,
        resource_id: Uuid,
    ) -> DeviceResult<bool> {
        let id = resource_selection_id(device_id, owner_subject, application_id, kind, resource_id);
        let key = resource_key(device_id, owner_subject, application_id, kind, resource_id);
        let conflicts = self
            .engine
            .row_conflict_values(RESOURCE_TABLE, &key)
            .await
            .map_err(db_error)?;
        if !conflicts.is_empty() {
            let mut resolutions = Vec::new();
            for (column, values) in conflicts {
                if column != "selected" || !values.contains(&Value::Bool(false)) {
                    return Err(invalid("conflicted resource selection"));
                }
                resolutions.push((column, Value::Bool(false)));
            }
            self.engine
                .resolve_row(RESOURCE_TABLE, &key, resolutions)
                .await
                .map_err(db_error)?;
        }
        let mut transaction = self.engine.transaction().await.map_err(db_error)?;
        let selected = Self::resource_selected_in(&mut transaction, &key, id).await;
        transaction.rollback().await.map_err(db_error)?;
        selected
    }

    async fn resource_selected_in(
        transaction: &mut Transaction,
        key: &Row,
        id: Uuid,
    ) -> DeviceResult<bool> {
        if Self::has_denial(transaction, id).await?
            || !transaction
                .row_conflicts(RESOURCE_TABLE, key)
                .await
                .map_err(db_error)?
                .is_empty()
        {
            return Ok(false);
        }
        let results = transaction.execute_sql(
            "SELECT device_id, owner_subject, application_id, selected_kind, selected_id, selected FROM device_resource_selections WHERE id = $1",
            Some(&uuid_param(id)),
        ).await.map_err(db_error)?;
        let mut expected = key.clone();
        expected.values.push(Value::Bool(true));
        Ok(results[0].rows.as_slice() == [expected])
    }

    async fn has_denial(transaction: &mut Transaction, id: Uuid) -> DeviceResult<bool> {
        let results = transaction
            .execute_sql(
                "SELECT id FROM device_selection_denials WHERE selection_id = $1 LIMIT 1",
                Some(&uuid_param(id)),
            )
            .await
            .map_err(db_error)?;
        Ok(!results[0].rows.is_empty())
    }

    async fn set_resource_selected(
        transaction: &mut Transaction,
        policy: &SelectionPolicy,
    ) -> DeviceResult<()> {
        let (Some(application), Some(kind), Some(resource)) = (
            policy.application_id,
            policy.selected_kind.as_deref(),
            policy.selected_id,
        ) else {
            return Ok(());
        };
        let id = resource_selection_id(
            policy.device_id,
            &policy.owner_subject,
            application,
            kind,
            resource,
        );
        if !transaction
            .row_conflicts(
                RESOURCE_TABLE,
                &resource_key(
                    policy.device_id,
                    &policy.owner_subject,
                    application,
                    kind,
                    resource,
                ),
            )
            .await
            .map_err(db_error)?
            .is_empty()
        {
            return Err(invalid("conflicted resource selection"));
        }
        let params = QueryParams::Positional(vec![
            Value::Uuid(id),
            Value::Uuid(policy.device_id),
            Value::Text(policy.owner_subject.clone()),
            Value::Uuid(application),
            Value::Text(kind.to_owned()),
            Value::Uuid(resource),
        ]);
        transaction.execute_sql(
            "INSERT INTO device_resource_selections (id, device_id, owner_subject, application_id, selected_kind, selected_id, selected) VALUES ($1, $2, $3, $4, $5, $6, true) ON CONFLICT (device_id, owner_subject, application_id, selected_kind, selected_id) DO UPDATE SET selected = true",
            Some(&params),
        ).await.map_err(db_error)?;
        // Each deletion addresses a row in this transaction's snapshot, not a future deny.
        transaction
            .execute_sql(
                "DELETE FROM device_selection_denials WHERE selection_id = $1",
                Some(&uuid_param(id)),
            )
            .await
            .map_err(db_error)?;
        Ok(())
    }

    async fn deny_resources(
        transaction: &mut Transaction,
        column: &str,
        id: Uuid,
    ) -> DeviceResult<bool> {
        let results = transaction
            .execute_sql(
                &format!("SELECT id FROM device_resource_selections WHERE {column} = $1"),
                Some(&uuid_param(id)),
            )
            .await
            .map_err(db_error)?;
        for row in &results[0].rows {
            transaction
                .execute_sql(
                    "INSERT INTO device_selection_denials (selection_id) VALUES ($1)",
                    Some(&QueryParams::Positional(row.values.clone())),
                )
                .await
                .map_err(db_error)?;
        }
        Ok(!results[0].rows.is_empty())
    }

    async fn policy_in(
        transaction: &mut Transaction,
        device_id: Uuid,
    ) -> DeviceResult<Option<SelectionPolicy>> {
        if !transaction
            .row_conflicts(TABLE, &key(device_id))
            .await
            .map_err(db_error)?
            .is_empty()
        {
            return Err(invalid("conflicted selection policy"));
        }
        let results = transaction
            .execute(vec![Statement::Query(policy_query(device_id))])
            .await
            .map_err(db_error)?;
        Ok(results[0]
            .rows_as::<SelectionPolicy>()
            .map_err(|error| invalid(error.to_string()))?
            .into_iter()
            .next())
    }

    async fn raw_record(&self, device_id: Uuid) -> DeviceResult<Option<SelectionPolicy>> {
        let mut results = self
            .engine
            .execute(vec![Statement::Query(policy_query(device_id))])
            .await
            .map_err(db_error)?;
        let rows = results
            .pop()
            .ok_or_else(|| invalid("missing selection policy query result"))?
            .rows_as::<SelectionPolicy>()
            .map_err(|error| invalid(error.to_string()))?;
        Ok(rows.into_iter().next())
    }

    /// # Errors
    ///
    /// Returns an error when stored Selection state is invalid or a database operation fails.
    pub async fn selected_resources_for_device(
        &self,
        device_id: Uuid,
    ) -> DeviceResult<Vec<SelectedResource>> {
        let mut results = self
            .engine
            .execute(vec![Statement::Query(resource_query(device_id))])
            .await
            .map_err(db_error)?;
        let rows = results
            .pop()
            .ok_or_else(|| invalid("missing resource selection query result"))?
            .rows_as::<ResourceSelection>()
            .map_err(|error| invalid(error.to_string()))?;
        let mut selected = Vec::new();
        for selection in rows {
            if selection.device_id == device_id
                && selection.selected
                && self
                    .resource_selected(
                        device_id,
                        &selection.owner_subject,
                        selection.application_id,
                        &selection.selected_kind,
                        selection.selected_id,
                    )
                    .await?
            {
                selected.push(SelectedResource {
                    device_id,
                    owner_subject: selection.owner_subject,
                    application_id: selection.application_id,
                    kind: selection.selected_kind,
                    resource_id: selection.selected_id,
                });
            }
        }
        Ok(selected)
    }

    /// # Errors
    ///
    /// Returns an error when Device or Selection state cannot be read or is invalid.
    pub async fn peers_selected_for_sync(
        &self,
        devices: &DbDeviceRepo,
        local_public_key: &str,
        remote_public_key: &str,
        owner_subject: &str,
        application_id: Uuid,
        kind: &str,
        resource_id: Uuid,
    ) -> DeviceResult<bool> {
        if local_public_key == remote_public_key
            || owner_subject.trim().is_empty()
            || !matches!(kind, "database" | "filesystem")
        {
            return Ok(false);
        }
        let records = devices.list().await?;
        for public_key in [local_public_key, remote_public_key] {
            let mut matching = records
                .iter()
                .filter(|device| device.public_key == public_key);
            let Some(device) = matching.next() else {
                return Ok(false);
            };
            if matching.next().is_some()
                || device.state != DeviceState::Approved
                || device.owner_subject != owner_subject
            {
                return Ok(false);
            }
            let Some(policy) = self.get(device.id, owner_subject, application_id).await? else {
                return Ok(false);
            };
            if !policy.admin_allowed
                || !self
                    .resource_selected(device.id, owner_subject, application_id, kind, resource_id)
                    .await?
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// The caller supplies current trusted `IdP` ownership, never a peer claim.
    /// An owner change remains gated until the local host records public owner cleanup completion.
    ///
    /// # Errors
    ///
    /// Returns an error when the current owner snapshot cannot be read or is invalid.
    pub async fn reconcile_owner_prevalidated(
        &self,
        device_id: Uuid,
        owner_subject: &str,
    ) -> DeviceResult<bool> {
        Ok(!matches!(
            self.observe_owner_prevalidated(device_id, owner_subject)
                .await?,
            OwnerPolicyObservation::PendingCleanup { .. }
        ))
    }

    /// Observes current policy, selections and the local retirement fence in one snapshot.
    /// The caller supplies trusted current ownership and revalidates it before publication.
    ///
    /// # Errors
    ///
    /// Returns an error when the snapshot is conflicted, invalid, or cannot be read.
    #[allow(
        clippy::significant_drop_tightening,
        reason = "commit consumes the transaction and releases its resources"
    )]
    #[allow(
        clippy::too_many_lines,
        reason = "keep the owner transition and retirement fence in one auditable transaction"
    )]
    pub async fn observe_owner_prevalidated(
        &self,
        device_id: Uuid,
        owner_subject: &str,
    ) -> DeviceResult<OwnerPolicyObservation> {
        if device_id.is_nil() || owner_subject.trim().is_empty() {
            return Err(invalid("invalid Device owner binding"));
        }
        self.resolve_convergent_conflicts(device_id).await?;
        let mut transaction = self.engine.transaction().await.map_err(db_error)?;
        if !transaction
            .row_conflicts(TABLE, &key(device_id))
            .await
            .map_err(db_error)?
            .is_empty()
        {
            return Err(invalid("conflicted selection policy"));
        }
        let results = transaction
            .execute(vec![Statement::Query(policy_query(device_id))])
            .await
            .map_err(db_error)?;
        let policies = results
            .first()
            .ok_or_else(|| invalid("missing policy query result"))?
            .rows_as::<SelectionPolicy>()
            .map_err(|error| invalid(error.to_string()))?;
        if !transaction
            .row_conflicts("local_device_copy_cleanup", &key(device_id))
            .await
            .map_err(db_error)?
            .is_empty()
        {
            return Err(invalid("conflicted owner cleanup"));
        }
        let history = transaction
            .execute(vec![Statement::Query(cleanup_query(device_id, false))])
            .await
            .map_err(db_error)?;
        let retained = history.iter().flat_map(|result| &result.rows).next();
        let pending = retained.is_some_and(|row| row.values.get(2) == Some(&Value::Bool(false)));
        let Some(policy) = policies.into_iter().next() else {
            if pending {
                return Err(invalid("cleanup policy missing"));
            }
            return Ok(OwnerPolicyObservation::MissingPolicy);
        };
        if policy.device_id != device_id
            || policy.owner_subject.trim().is_empty()
            || policy.application_id.is_some() != policy.selected_kind.is_some()
            || policy.selected_kind.is_some() != policy.selected_id.is_some()
        {
            return Err(invalid("invalid selection policy"));
        }
        if policy.owner_subject == owner_subject && !pending {
            let results = transaction
                .execute(vec![Statement::Query(resource_query(device_id))])
                .await
                .map_err(db_error)?;
            let selections = results
                .first()
                .ok_or_else(|| invalid("missing resource selection query result"))?
                .rows_as::<ResourceSelection>()
                .map_err(|error| invalid(error.to_string()))?;
            let mut resources = Vec::new();
            for selection in selections {
                if selection.device_id != device_id
                    || selection.id
                        != resource_selection_id(
                            device_id,
                            &selection.owner_subject,
                            selection.application_id,
                            &selection.selected_kind,
                            selection.selected_id,
                        )
                    || selection.application_id.is_nil()
                    || selection.selected_id.is_nil()
                    || !matches!(selection.selected_kind.as_str(), "database" | "filesystem")
                {
                    return Err(invalid("invalid resource selection binding"));
                }
                let conflicts = transaction
                    .row_conflict_values(
                        RESOURCE_TABLE,
                        &resource_key(
                            device_id,
                            &selection.owner_subject,
                            selection.application_id,
                            &selection.selected_kind,
                            selection.selected_id,
                        ),
                    )
                    .await
                    .map_err(db_error)?;
                for (column, values) in &conflicts {
                    if column != "selected" || !values.contains(&Value::Bool(false)) {
                        return Err(invalid("conflicted resource selection"));
                    }
                }
                // A concurrent deselection is denial evidence, never permission.
                if selection.selected
                    && conflicts.is_empty()
                    && !Self::has_denial(&mut transaction, selection.id).await?
                {
                    if selection.owner_subject != owner_subject {
                        return Err(invalid("selected resource owner mismatch"));
                    }
                    resources.push(SelectedResource {
                        device_id,
                        owner_subject: selection.owner_subject,
                        application_id: selection.application_id,
                        kind: selection.selected_kind,
                        resource_id: selection.selected_id,
                    });
                }
            }
            return Ok(OwnerPolicyObservation::Ready { policy, resources });
        }
        let cleanup_id = if pending {
            let value = retained
                .and_then(|row| row.values.get(1))
                .ok_or_else(|| invalid("missing owner cleanup ID"))?;
            let id: Uuid = ofdb_sql::decode(value, "cleanup_id")
                .map_err(|error| invalid(error.to_string()))?;
            if id.is_nil() {
                return Err(invalid("invalid owner cleanup ID"));
            }
            id
        } else {
            Uuid::now_v7()
        };
        if !pending {
            let cleanup_id = Value::Uuid(cleanup_id);
            let query = if retained.is_some() {
                Query::Update(QueryUpdate {
                    from: QueryFrom {
                        table: "local_device_copy_cleanup".into(),
                        joins: vec![],
                    },
                    assignments: vec![
                        assignment_for("local_device_copy_cleanup", "cleanup_id", cleanup_id),
                        assignment_for(
                            "local_device_copy_cleanup",
                            "completed",
                            Value::Bool(false),
                        ),
                    ],
                    predicate: Some(cleanup_device(device_id)),
                    returning: None,
                })
            } else {
                Query::Insert(QueryInsert {
                    table: "local_device_copy_cleanup".into(),
                    row: Row::new(vec![
                        Value::Text(device_id.to_string()),
                        cleanup_id,
                        Value::Bool(false),
                    ]),
                    returning: None,
                })
            };
            transaction
                .execute(vec![Statement::Query(query)])
                .await
                .map_err(db_error)?;
        }
        transaction
            .execute(vec![Statement::Query(Query::Update(QueryUpdate {
                from: from(),
                assignments: vec![
                    assignment("admin_allowed", Value::Bool(false)),
                    assignment("application_id", Value::Null),
                    assignment("selected_kind", Value::Null),
                    assignment("selected_id", Value::Null),
                ],
                predicate: Some(equals("device_id", Value::Uuid(device_id))),
                returning: None,
            }))])
            .await
            .map_err(db_error)?;
        Self::deny_resources(&mut transaction, "device_id", device_id).await?;
        transaction.commit().await.map_err(db_error)?;
        Ok(OwnerPolicyObservation::PendingCleanup {
            cleanup_id,
            old_owner_subject: policy.owner_subject,
        })
    }

    /// # Errors
    ///
    /// Returns an error when the local cleanup state cannot be read.
    pub async fn local_cleanup_pending(&self, device_id: Uuid) -> DeviceResult<bool> {
        Ok(self
            .engine
            .execute(vec![Statement::Query(cleanup_query(device_id, true))])
            .await
            .map_err(db_error)?
            .iter()
            .any(|result| !result.rows.is_empty()))
    }

    /// # Errors
    ///
    /// Returns an error when the cleanup state is conflicted, invalid, or cannot be read.
    pub async fn pending_cleanup_id(&self, device_id: Uuid) -> DeviceResult<Option<Uuid>> {
        if !self
            .engine
            .row_conflicts("local_device_copy_cleanup", &key(device_id))
            .await
            .map_err(db_error)?
            .is_empty()
        {
            return Err(invalid("conflicted owner cleanup"));
        }
        let results = self
            .engine
            .execute(vec![Statement::Query(cleanup_query(device_id, true))])
            .await
            .map_err(db_error)?;
        results
            .iter()
            .flat_map(|result| &result.rows)
            .next()
            .map(|row| {
                let value = row
                    .values
                    .get(1)
                    .ok_or_else(|| invalid("missing owner cleanup ID"))?;
                let id: Uuid = ofdb_sql::decode(value, "cleanup_id")
                    .map_err(|error| invalid(error.to_string()))?;
                if id.is_nil() {
                    return Err(invalid("invalid owner cleanup ID"));
                }
                Ok(id)
            })
            .transpose()
    }

    /// Trusted local host or authenticated Storage service uses this retirement intent.
    ///
    /// # Errors
    ///
    /// Returns an error when the cleanup state or its Selection policy is invalid or unavailable.
    pub async fn pending_cleanup_owner(&self, device_id: Uuid) -> DeviceResult<Option<String>> {
        if !self.local_cleanup_pending(device_id).await? {
            return Ok(None);
        }
        Ok(Some(
            self.record(device_id)
                .await?
                .ok_or_else(|| invalid("cleanup policy missing"))?
                .owner_subject,
        ))
    }

    /// Caller must finish both public Storage owner cleanups and revalidate current `IdP` ownership.
    ///
    /// # Errors
    ///
    /// Returns an error when owner cleanup cannot be completed or stored state is invalid.
    pub async fn complete_owner_cleanup_prevalidated(
        &self,
        device_id: Uuid,
        old_owner: &str,
        new_owner: &str,
    ) -> DeviceResult<bool> {
        self.complete_owner_cleanup(device_id, old_owner, new_owner, None)
            .await
    }

    /// Storage acknowledges retirement only for this current Management endpoint/owner binding.
    ///
    /// # Errors
    ///
    /// Returns an error when the bound cleanup cannot be completed or stored state is invalid.
    pub async fn complete_owner_cleanup_bound(
        &self,
        device_id: Uuid,
        old_owner: &str,
        new_owner: &str,
        endpoint: &str,
        cleanup_id: Uuid,
    ) -> DeviceResult<bool> {
        if cleanup_id.is_nil() {
            return Ok(false);
        }
        self.complete_owner_cleanup(
            device_id,
            old_owner,
            new_owner,
            Some((endpoint, cleanup_id)),
        )
        .await
    }

    #[allow(
        clippy::significant_drop_tightening,
        reason = "commit consumes the transaction and releases its resources"
    )]
    #[allow(
        clippy::too_many_lines,
        reason = "keep endpoint binding, Selection denial, and cleanup completion in one transaction"
    )]
    async fn complete_owner_cleanup(
        &self,
        device_id: Uuid,
        old_owner: &str,
        new_owner: &str,
        endpoint: Option<(&str, Uuid)>,
    ) -> DeviceResult<bool> {
        if ![old_owner, new_owner]
            .into_iter()
            .all(|owner| owner.parse::<Uuid>().is_ok_and(|owner| !owner.is_nil()))
        {
            return Ok(false);
        }
        let mut transaction = self.engine.transaction().await.map_err(db_error)?;
        if let Some((endpoint, _)) = endpoint {
            for table in ["devices", "local_device_enrollments"] {
                if !transaction
                    .row_conflicts(table, &device_key(table, device_id))
                    .await
                    .map_err(db_error)?
                    .is_empty()
                {
                    return Ok(false);
                }
            }
            let results = transaction
                .execute(vec![Statement::Query(Query::Select(QuerySelect {
                    from: QueryFrom {
                        table: "devices".into(),
                        joins: vec![],
                    },
                    projection: ["owner_subject", "public_key", "state", "revoked_at"]
                        .into_iter()
                        .map(|name| QueryColumn::new("devices".into(), name.into()))
                        .collect(),
                    distinct: false,
                    predicate: Some(QueryExpr::Equals(
                        Box::new(QueryExpr::Value(QueryExprValue::Column(QueryColumn::new(
                            "devices".into(),
                            "id".into(),
                        )))),
                        Box::new(QueryExpr::Value(QueryExprValue::Value(Value::Uuid(
                            device_id,
                        )))),
                    )),
                    aggregates: vec![],
                    group_by: vec![],
                    order_by: vec![],
                    text_concats: vec![],

                    limit: None,
                    offset: None,
                    having: None,
                }))])
                .await
                .map_err(db_error)?;
            if !results.iter().any(|result| {
                result.rows.iter().any(|row| {
                    row.values
                        == vec![
                            Value::Text(new_owner.to_owned()),
                            Value::Text(endpoint.to_owned()),
                            Value::Integer(1),
                            Value::Null,
                        ]
                })
            }) {
                return Ok(false);
            }
        }
        if !transaction
            .row_conflicts(TABLE, &key(device_id))
            .await
            .map_err(db_error)?
            .is_empty()
        {
            return Ok(false);
        }
        let results = transaction
            .execute(vec![Statement::Query(policy_query(device_id))])
            .await
            .map_err(db_error)?;
        let Some(result) = results.first() else {
            return Ok(false);
        };
        let policy = result
            .rows_as::<SelectionPolicy>()
            .map_err(|error| invalid(error.to_string()))?;
        let Some(policy) = policy.first() else {
            return Ok(false);
        };
        let pending = transaction
            .execute(vec![Statement::Query(cleanup_query(device_id, true))])
            .await
            .map_err(db_error)?;
        if pending.iter().all(|result| result.rows.is_empty()) {
            return Ok(policy.owner_subject == new_owner);
        }
        if policy.owner_subject != old_owner {
            return Ok(false);
        }
        if let Some((_, cleanup_id)) = endpoint
            && (!transaction
                .row_conflicts("local_device_copy_cleanup", &key(device_id))
                .await
                .map_err(db_error)?
                .is_empty()
                || !pending.iter().any(|result| {
                    result
                        .rows
                        .iter()
                        .any(|row| row.values.get(1) == Some(&Value::Uuid(cleanup_id)))
                }))
        {
            return Ok(false);
        }
        transaction
            .execute(vec![
                Statement::Query(Query::Update(QueryUpdate {
                    from: from(),
                    assignments: vec![
                        assignment("owner_subject", Value::Text(new_owner.to_owned())),
                        assignment("admin_allowed", Value::Bool(false)),
                        assignment("application_id", Value::Null),
                        assignment("selected_kind", Value::Null),
                        assignment("selected_id", Value::Null),
                    ],
                    predicate: Some(QueryExpr::And(
                        Box::new(equals("device_id", Value::Uuid(device_id))),
                        Box::new(equals("owner_subject", Value::Text(old_owner.to_owned()))),
                    )),
                    returning: None,
                })),
                Statement::Query(Query::Update(QueryUpdate {
                    from: QueryFrom {
                        table: "local_device_copy_cleanup".into(),
                        joins: vec![],
                    },
                    assignments: vec![assignment_for(
                        "local_device_copy_cleanup",
                        "completed",
                        Value::Bool(true),
                    )],
                    predicate: Some(cleanup_device(device_id)),
                    returning: None,
                })),
            ])
            .await
            .map_err(db_error)?;
        transaction.commit().await.map_err(db_error)?;
        Ok(true)
    }

    /// # Errors
    ///
    /// Returns an error when the Selection policy is conflicted, invalid, or cannot be read.
    pub async fn get(
        &self,
        device_id: Uuid,
        owner_subject: &str,
        _application_id: Uuid,
    ) -> DeviceResult<Option<SelectionPolicy>> {
        let policy = self.record(device_id).await?;
        if let Some(policy) = &policy {
            if policy.owner_subject != owner_subject {
                return Err(invalid("selection policy owner mismatch"));
            }
            if policy.application_id.is_some() != policy.selected_kind.is_some()
                || policy.selected_kind.is_some() != policy.selected_id.is_some()
            {
                return Err(invalid("incomplete selection policy"));
            }
        }
        Ok(policy)
    }

    /// Caller must validate Device ownership, authorization, and Resource catalog membership.
    ///
    /// # Errors
    ///
    /// Returns an error when the policy is invalid or the transactional update fails.
    #[allow(
        clippy::too_many_lines,
        reason = "keep policy and Resource Selection updates in one auditable transaction"
    )]
    pub async fn set_prevalidated(&self, policy: SelectionPolicy) -> DeviceResult<()> {
        if policy.owner_subject.trim().is_empty()
            || policy.application_id.is_some() != policy.selected_kind.is_some()
            || policy.selected_kind.is_some() != policy.selected_id.is_some()
            || policy.selected_kind.as_ref().is_some_and(String::is_empty)
        {
            return Err(invalid("invalid selection policy"));
        }
        self.resolve_convergent_conflicts(policy.device_id).await?;
        if let (Some(application), Some(kind), Some(resource)) = (
            policy.application_id,
            policy.selected_kind.as_deref(),
            policy.selected_id,
        ) {
            self.resource_selected(
                policy.device_id,
                &policy.owner_subject,
                application,
                kind,
                resource,
            )
            .await?;
        }
        let mut transaction = self.engine.transaction().await.map_err(db_error)?;
        let update = async {
            let old = Self::policy_in(&mut transaction, policy.device_id).await?;
            if let Some(old) = &old {
                if old.owner_subject != policy.owner_subject {
                    return Err(invalid("selection policy owner mismatch"));
                }
                if old.application_id.is_some() != old.selected_kind.is_some()
                    || old.selected_kind.is_some() != old.selected_id.is_some()
                {
                    return Err(invalid("incomplete selection policy"));
                }
                if policy.selected_id.is_some() && !old.admin_allowed {
                    return Err(invalid("administrator has restricted selection"));
                }
            }
            let selection_unchanged = old.as_ref().is_some_and(|old| {
                old.application_id == policy.application_id
                    && old.selected_kind == policy.selected_kind
                    && old.selected_id == policy.selected_id
            });
            let query = if selection_unchanged {
                None
            } else if old.is_some() {
                Some(Query::Update(QueryUpdate {
                    from: from(),
                    assignments: vec![
                        assignment(
                            "application_id",
                            policy.application_id.map_or(Value::Null, Value::Uuid),
                        ),
                        assignment(
                            "selected_kind",
                            policy
                                .selected_kind
                                .clone()
                                .map_or(Value::Null, Value::Text),
                        ),
                        assignment(
                            "selected_id",
                            policy.selected_id.map_or(Value::Null, Value::Uuid),
                        ),
                    ],
                    predicate: Some(equals("device_id", Value::Uuid(policy.device_id))),
                    returning: None,
                }))
            } else {
                Some(Query::Insert(QueryInsert {
                    table: TABLE.into(),
                    row: Row::new(vec![
                        Value::Uuid(policy.device_id),
                        Value::Text(policy.owner_subject.clone()),
                        policy.application_id.map_or(Value::Null, Value::Uuid),
                        policy
                            .selected_kind
                            .clone()
                            .map_or(Value::Null, Value::Text),
                        policy.selected_id.map_or(Value::Null, Value::Uuid),
                        Value::Bool(policy.admin_allowed),
                        Value::Text(policy.device_id.to_string()),
                    ]),
                    returning: None,
                }))
            };
            if let Some(query) = query {
                transaction
                    .execute(vec![Statement::Query(query)])
                    .await
                    .map_err(db_error)?;
            }
            if policy.selected_id.is_none() {
                Self::deny_resources(&mut transaction, "device_id", policy.device_id).await?;
            } else {
                Self::set_resource_selected(&mut transaction, &policy).await?;
            }
            Ok(())
        }
        .await;
        if update.is_err() {
            transaction.rollback().await.map_err(db_error)?;
            return update;
        }
        transaction.commit().await.map_err(db_error)
    }

    /// Caller must validate permission to change the admin restriction.
    ///
    /// # Errors
    ///
    /// Returns an error when the Selection policy cannot be read or updated.
    pub async fn set_admin_allowed_prevalidated(
        &self,
        device_id: Uuid,
        owner_subject: &str,
        admin_allowed: bool,
    ) -> DeviceResult<bool> {
        if owner_subject.trim().is_empty() {
            return Ok(false);
        }
        let Some(policy) = self.record(device_id).await? else {
            self.set_prevalidated(SelectionPolicy {
                device_id,
                owner_subject: owner_subject.to_owned(),
                application_id: None,
                selected_kind: None,
                selected_id: None,
                admin_allowed,
            })
            .await?;
            return Ok(true);
        };
        if policy.owner_subject != owner_subject {
            return Ok(false);
        }
        if policy.admin_allowed == admin_allowed {
            return Ok(true);
        }
        self.engine
            .execute(vec![Statement::Query(Query::Update(QueryUpdate {
                from: from(),
                assignments: vec![assignment("admin_allowed", Value::Bool(admin_allowed))],
                predicate: Some(equals("device_id", Value::Uuid(device_id))),
                returning: None,
            }))])
            .await
            .map_err(db_error)?;
        self.clear(device_id).await?;
        Ok(true)
    }

    /// # Errors
    ///
    /// Returns an error when the Resource Selection cannot be validated or denied.
    #[allow(
        clippy::significant_drop_tightening,
        reason = "commit consumes the transaction and releases its resources"
    )]
    pub async fn deselect_resource_owned(
        &self,
        device_id: Uuid,
        owner_subject: &str,
        application_id: Uuid,
        kind: &str,
        resource_id: Uuid,
    ) -> DeviceResult<bool> {
        self.resolve_convergent_conflicts(device_id).await?;
        let mut transaction = self.engine.transaction().await.map_err(db_error)?;
        let outcome = async {
            let Some(policy) = Self::policy_in(&mut transaction, device_id).await? else {
                return Ok(false);
            };
            if owner_subject.is_empty() || policy.owner_subject != owner_subject {
                return Ok(false);
            }
            let id =
                resource_selection_id(device_id, owner_subject, application_id, kind, resource_id);
            if !Self::resource_selected_in(
                &mut transaction,
                &resource_key(device_id, owner_subject, application_id, kind, resource_id),
                id,
            )
            .await?
            {
                return Ok(false);
            }
            Self::deny_resources(&mut transaction, "id", id).await
        }
        .await;
        if !matches!(outcome, Ok(true)) {
            transaction.rollback().await.map_err(db_error)?;
            return outcome;
        }
        transaction.commit().await.map_err(db_error)?;
        Ok(true)
    }

    /// # Errors
    ///
    /// Returns an error when the owned Selection state cannot be validated or updated.
    #[allow(
        clippy::significant_drop_tightening,
        reason = "commit consumes the transaction and releases its resources"
    )]
    pub async fn deselect_owned(&self, device_id: Uuid, owner_subject: &str) -> DeviceResult<bool> {
        let conflicts = self
            .engine
            .row_conflict_values(TABLE, &key(device_id))
            .await
            .map_err(db_error)?;
        if conflicts.iter().any(|(column, _)| {
            !matches!(
                column.as_str(),
                "application_id" | "selected_kind" | "selected_id"
            )
        }) {
            return Err(invalid(
                "conflicted selection policy ownership or admin restriction",
            ));
        }
        let Some(policy) = self.raw_record(device_id).await? else {
            return Ok(false);
        };
        if owner_subject.is_empty() || policy.owner_subject != owner_subject {
            return Ok(false);
        }
        if !conflicts.is_empty() {
            self.engine
                .resolve_row(
                    TABLE,
                    &key(device_id),
                    conflicts
                        .into_iter()
                        .map(|(column, _)| (column, Value::Null))
                        .collect(),
                )
                .await
                .map_err(db_error)?;
            self.clear(device_id).await?;
        }
        let mut transaction = self.engine.transaction().await.map_err(db_error)?;
        let update = async {
            if Self::policy_in(&mut transaction, device_id)
                .await?
                .is_none_or(|policy| policy.owner_subject != owner_subject)
            {
                return Ok(false);
            }
            Self::deny_resources(&mut transaction, "device_id", device_id).await?;
            transaction
                .execute(vec![Statement::Query(Query::Update(QueryUpdate {
                    from: from(),
                    assignments: ["application_id", "selected_kind", "selected_id"]
                        .into_iter()
                        .map(|column| assignment(column, Value::Null))
                        .collect(),
                    predicate: Some(equals("device_id", Value::Uuid(device_id))),
                    returning: None,
                }))])
                .await
                .map_err(db_error)?;
            if !transaction
                .row_conflicts(TABLE, &key(device_id))
                .await
                .map_err(db_error)?
                .is_empty()
            {
                return Err(invalid("conflicted selection policy"));
            }
            Ok(true)
        }
        .await;
        if !matches!(update, Ok(true)) {
            transaction.rollback().await.map_err(db_error)?;
            return update;
        }
        transaction.commit().await.map_err(db_error)?;
        Ok(true)
    }
}

fn invalid(message: impl Into<String>) -> DeviceError {
    DeviceError::InvalidInput(message.into())
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "map_err supplies an owned QueryError to this adapter"
)]
fn db_error(error: ofdb_sql::QueryError) -> DeviceError {
    invalid(error.to_string())
}

fn key(id: Uuid) -> Row {
    Row::new(vec![Value::Text(id.to_string())])
}

fn resource_key(
    device_id: Uuid,
    owner_subject: &str,
    application_id: Uuid,
    kind: &str,
    resource_id: Uuid,
) -> Row {
    Row::new(vec![
        Value::Uuid(device_id),
        Value::Text(owner_subject.to_owned()),
        Value::Uuid(application_id),
        Value::Text(kind.to_owned()),
        Value::Uuid(resource_id),
    ])
}

fn uuid_param(id: Uuid) -> QueryParams {
    QueryParams::Positional(vec![Value::Uuid(id)])
}

fn resource_selection_id(
    device_id: Uuid,
    owner_subject: &str,
    application_id: Uuid,
    kind: &str,
    resource_id: Uuid,
) -> Uuid {
    let mut identity = Vec::with_capacity(16 * 3 + owner_subject.len() + kind.len() + 16);
    identity.extend_from_slice(device_id.as_bytes());
    identity.extend_from_slice(&(owner_subject.len() as u64).to_be_bytes());
    identity.extend_from_slice(owner_subject.as_bytes());
    identity.extend_from_slice(application_id.as_bytes());
    identity.extend_from_slice(&(kind.len() as u64).to_be_bytes());
    identity.extend_from_slice(kind.as_bytes());
    identity.extend_from_slice(resource_id.as_bytes());
    Uuid::new_v5(&Uuid::NAMESPACE_URL, &identity)
}

fn resource_from() -> QueryFrom {
    QueryFrom {
        table: RESOURCE_TABLE.into(),
        joins: vec![],
    }
}

fn resource_column(name: &str) -> QueryColumn {
    QueryColumn::new(RESOURCE_TABLE.into(), name.into())
}

fn resource_equals(name: &str, value: Value) -> QueryExpr {
    QueryExpr::Equals(
        Box::new(QueryExpr::Value(QueryExprValue::Column(resource_column(
            name,
        )))),
        Box::new(QueryExpr::Value(QueryExprValue::Value(value))),
    )
}

fn assignment_for(table: &str, name: &str, value: Value) -> QueryUpdateAssignment {
    QueryUpdateAssignment {
        column: QueryColumn::new(table.into(), name.into()),
        value: QueryExprValue::Value(value),
    }
}

fn from() -> QueryFrom {
    QueryFrom {
        table: TABLE.into(),
        joins: vec![],
    }
}

fn column(name: &str) -> QueryColumn {
    QueryColumn::new(TABLE.into(), name.into())
}

fn equals(name: &str, value: Value) -> QueryExpr {
    QueryExpr::Equals(
        Box::new(QueryExpr::Value(QueryExprValue::Column(column(name)))),
        Box::new(QueryExpr::Value(QueryExprValue::Value(value))),
    )
}

fn assignment(name: &str, value: Value) -> QueryUpdateAssignment {
    QueryUpdateAssignment {
        column: column(name),
        value: QueryExprValue::Value(value),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    };

    use of_model::device::contract::device_state::DeviceState;
    use ofdb_sql::Database;
    use ofdb_sql::Uuid;
    use ofdb_sql::Value;

    use crate::{device_repo::DeviceRepo, replica::device_repo::DbDeviceRepo};

    use crate::ofdb::selection_policy::DbSelectionPolicyRepo;

    use of_model::device::selection_policy::OwnerPolicyObservation;

    use of_model::device::selection_policy::SelectedResource;

    use of_model::device::selection_policy::SelectionPolicy;

    use super::TABLE;

    use super::assignment;

    use super::from;

    use super::key;

    #[tokio::test]
    async fn owner_observation_keeps_exact_selections_and_cleanup_history_after_reopen() {
        let path = std::env::temp_dir().join(format!("owner-observation-{}.redb", Uuid::now_v7()));
        let device = Uuid::now_v7();
        let owner = Uuid::now_v7().to_string();
        let next = Uuid::now_v7().to_string();
        let application = Uuid::now_v7();
        let database = Uuid::now_v7();
        let filesystem = Uuid::now_v7();
        let cleanup_id;
        {
            let engine = Arc::new(Database::open(&path).expect("open Management owner"));
            crate::ofdb::schema::up(&engine)
                .await
                .expect("Management schema");
            let repo = DbSelectionPolicyRepo::new(engine.client());
            for (kind, resource) in [("database", database), ("filesystem", filesystem)] {
                repo.set_prevalidated(SelectionPolicy {
                    device_id: device,
                    owner_subject: owner.clone(),
                    application_id: Some(application),
                    selected_kind: Some(kind.into()),
                    selected_id: Some(resource),
                    admin_allowed: true,
                })
                .await
                .expect("select exact owner resource");
            }
            repo.set_admin_allowed_prevalidated(device, &owner, false)
                .await
                .expect("restrict without erasing selections");
            let of_model::device::selection_policy::OwnerPolicyObservation::Ready {
                policy,
                mut resources,
            } = repo
                .observe_owner_prevalidated(device, &owner)
                .await
                .expect("ready observation")
            else {
                panic!("current owner must have a ready observation");
            };
            assert_eq!(policy.device_id, device);
            assert_eq!(policy.owner_subject, owner);
            assert!(!policy.admin_allowed);
            resources.sort_by(|a, b| a.kind.cmp(&b.kind));
            assert_eq!(
                resources,
                vec![
                    SelectedResource {
                        device_id: device,
                        owner_subject: owner.clone(),
                        application_id: application,
                        kind: "database".into(),
                        resource_id: database
                    },
                    SelectedResource {
                        device_id: device,
                        owner_subject: owner.clone(),
                        application_id: application,
                        kind: "filesystem".into(),
                        resource_id: filesystem
                    },
                ]
            );
            let of_model::device::selection_policy::OwnerPolicyObservation::PendingCleanup {
                cleanup_id: id,
                old_owner_subject,
            } = repo
                .observe_owner_prevalidated(device, &next)
                .await
                .expect("learn owner transfer")
            else {
                panic!("transfer must expose only a pending cleanup binding");
            };
            cleanup_id = id;
            assert!(!id.is_nil());
            assert_eq!(old_owner_subject, owner);
            assert_eq!(
                repo.pending_cleanup_id(device)
                    .await
                    .expect("public cleanup ID"),
                Some(id)
            );
        }
        {
            let engine = Arc::new(Database::open(&path).expect("offline owner reopen"));
            let repo = DbSelectionPolicyRepo::new(engine.client());
            assert_eq!(
                repo.observe_owner_prevalidated(device, &owner)
                    .await
                    .expect("A to B to A remains pending"),
                of_model::device::selection_policy::OwnerPolicyObservation::PendingCleanup {
                    cleanup_id,
                    old_owner_subject: owner.clone()
                }
            );
            assert!(
                repo.complete_owner_cleanup_prevalidated(device, &owner, &owner)
                    .await
                    .expect("retire original owner copies")
            );
            assert!(
                matches!(repo.observe_owner_prevalidated(device, &owner).await.expect("completed history is not pending"), of_model::device::selection_policy::OwnerPolicyObservation::Ready { policy, resources } if !policy.admin_allowed && resources.is_empty())
            );
            let of_model::device::selection_policy::OwnerPolicyObservation::PendingCleanup {
                cleanup_id: fresh,
                old_owner_subject,
            } = repo
                .observe_owner_prevalidated(device, &next)
                .await
                .expect("later transfer reopens completed history")
            else {
                panic!("later transfer must create a fresh obligation");
            };
            assert_ne!(fresh, cleanup_id);
            assert_eq!(old_owner_subject, owner);
        }
        std::fs::remove_file(path).expect("remove Management fixture");
    }

    #[tokio::test]
    async fn owner_observation_denies_concurrent_deselection_and_ambiguous_resource_bindings() {
        let first = Arc::new(Database::in_memory());
        let second = Database::in_memory();
        crate::ofdb::schema::up(&first)
            .await
            .expect("Management schema");
        let repo = DbSelectionPolicyRepo::new(first.clone().client());
        let device = Uuid::now_v7();
        let policy = SelectionPolicy {
            device_id: device,
            owner_subject: "owner".into(),
            application_id: Some(Uuid::now_v7()),
            selected_kind: Some("database".into()),
            selected_id: Some(Uuid::now_v7()),
            admin_allowed: true,
        };
        for _ in 0..2 {
            repo.set_prevalidated(policy.clone())
                .await
                .expect("select resource");
        }
        let selections = first
            .export_history()
            .await
            .expect("export Device history")
            .into_iter()
            .filter_map(|unit| match unit.key {
                ofdb_sql_history::SyncKey::Row { table, row }
                    if table == "device_resource_selections" =>
                {
                    Some(row)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(selections.len(), 1, "reselection retains one Logical Row");
        assert_eq!(selections[0].get_version_num(), 7);
        sync_replica(&first, &second).await;
        first
            .client()
            .translate_and_execute(
                "UPDATE device_resource_selections SET selected = false",
                &ofdb_sql::SqlTranslator,
            )
            .await
            .expect("offline deselection");
        for value in ["false", "true"] {
            second
                .client()
                .translate_and_execute(
                    &format!("UPDATE device_resource_selections SET selected = {value}"),
                    &ofdb_sql::SqlTranslator,
                )
                .await
                .expect("concurrent reselection");
        }
        sync_replica(&first, &second).await;
        assert!(
            matches!(repo.observe_owner_prevalidated(device, "owner").await.expect("conservative resource observation"), of_model::device::selection_policy::OwnerPolicyObservation::Ready { policy, resources } if policy.admin_allowed && resources.is_empty())
        );
        first
            .client()
            .translate_and_execute(
                "UPDATE device_resource_selections SET id = CAST('01890f4e-7c00-7000-8000-000000000001' AS UUID)",
                &ofdb_sql::SqlTranslator,
            )
            .await
            .expect("diverge first resource binding");
        second
            .client()
            .translate_and_execute(
                "UPDATE device_resource_selections SET id = CAST('01890f4e-7c00-7000-8000-000000000002' AS UUID)",
                &ofdb_sql::SqlTranslator,
            )
            .await
            .expect("diverge second resource binding");
        sync_replica(&first, &second).await;
        assert!(
            repo.observe_owner_prevalidated(device, "owner")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn independent_selection_cannot_erase_deselection_and_can_be_reselected() {
        let (left, right) = (Database::in_memory(), Database::in_memory());
        let policy = SelectionPolicy {
            device_id: Uuid::now_v7(),
            owner_subject: "owner".into(),
            application_id: Some(Uuid::now_v7()),
            selected_kind: Some("database".into()),
            selected_id: Some(Uuid::now_v7()),
            admin_allowed: true,
        };
        let a = DbSelectionPolicyRepo::new(left.client());
        let b = DbSelectionPolicyRepo::new(right.client());
        crate::ofdb::schema::up(&left).await.expect("left schema");
        a.set_prevalidated(policy.clone())
            .await
            .expect("left selection");
        assert!(
            a.deselect_resource_owned(
                policy.device_id,
                "owner",
                policy.application_id.expect("application"),
                "database",
                policy.selected_id.expect("resource")
            )
            .await
            .expect("deselect on left")
        );
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        crate::ofdb::schema::up(&right).await.expect("right schema");
        b.set_prevalidated(policy.clone())
            .await
            .expect("independent later selection");
        exchange_device_history(&left, &right).await;
        for repo in [&a, &b] {
            assert!(
                repo.selected_resources_for_device(policy.device_id)
                    .await
                    .expect("list selections")
                    .is_empty()
            );
            assert!(
                matches!(repo.observe_owner_prevalidated(policy.device_id, "owner").await.expect("owner observation"), OwnerPolicyObservation::Ready { resources, .. } if resources.is_empty())
            );
        }
        b.set_prevalidated(policy.clone())
            .await
            .expect("reselect after observing deny");
        exchange_device_history(&right, &left).await;
        for repo in [&a, &b] {
            assert_eq!(
                repo.selected_resources_for_device(policy.device_id)
                    .await
                    .expect("list after reselect")
                    .len(),
                1
            );
        }
        // Both peers now observe the Selection. A removes only its own next denial.
        for repo in [&a, &b] {
            assert!(
                repo.deselect_resource_owned(
                    policy.device_id,
                    "owner",
                    policy.application_id.expect("application"),
                    "database",
                    policy.selected_id.expect("resource")
                )
                .await
                .expect("concurrent deselection")
            );
        }
        a.set_prevalidated(policy.clone())
            .await
            .expect("reselect without observing the other deny");
        exchange_device_history(&left, &right).await;
        for repo in [&a, &b] {
            assert!(
                repo.selected_resources_for_device(policy.device_id)
                    .await
                    .expect("unobserved deny remains live")
                    .is_empty()
            );
        }
        a.set_prevalidated(policy.clone())
            .await
            .expect("reselect after observing both denials");
        exchange_device_history(&left, &right).await;
        for repo in [&a, &b] {
            assert_eq!(
                repo.selected_resources_for_device(policy.device_id)
                    .await
                    .expect("converged reselect")
                    .len(),
                1
            );
        }
    }

    async fn exchange_device_history(left: &Database, right: &Database) {
        use crate::security_policy::SecurityHistory;
        use ofdb_sql::history::{SessionConfig, SyncRole, SyncTransport};
        use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

        struct Channel(UnboundedSender<Vec<u8>>, UnboundedReceiver<Vec<u8>>);
        impl SyncTransport for Channel {
            type Error = &'static str;
            async fn send(&mut self, frame: Vec<u8>) -> Result<(), Self::Error> {
                self.0.send(frame).map_err(|_| "closed")
            }
            async fn receive(&mut self) -> Result<Vec<u8>, Self::Error> {
                self.1.recv().await.ok_or("closed")
            }
        }
        let (a, rx_a) = unbounded_channel();
        let (b, rx_b) = unbounded_channel();
        let config = SessionConfig::default();
        let owner = SecurityHistory::Devices;
        let tables = owner.tables();
        let exchange = |database: &Database, role, mut channel: Channel| {
            let database = database.clone();
            let config = &config;
            let tables = &tables;
            async move {
                database
                    .exchange_history(
                        &mut channel,
                        config,
                        role,
                        std::env::temp_dir(),
                        tables,
                        |table, column, value| {
                            owner.validate_value(table, column, value, "fixture")
                        },
                        |view| {
                            owner
                                .validate_candidate(view, &[])
                                .map_err(|error| error.to_string())
                        },
                        || async { Ok(()) },
                    )
                    .await
            }
        };
        let (a_result, b_result) =
            tokio::time::timeout(std::time::Duration::from_secs(15), async {
                tokio::join!(
                    exchange(left, SyncRole::Initiator, Channel(a, rx_b)),
                    exchange(right, SyncRole::Responder, Channel(b, rx_a))
                )
            })
            .await
            .expect("Device exchange completes");
        assert!(
            a_result.is_ok() && b_result.is_ok(),
            "{a_result:?}; {b_result:?}"
        );
    }

    async fn sync_replica(source: &ofdb_sql::Database, destination: &ofdb_sql::Database) {
        destination
            .import_history(
                source
                    .export_history()
                    .await
                    .expect("source native history"),
            )
            .await
            .expect("atomic native history import");
        source
            .import_history(
                destination
                    .export_history()
                    .await
                    .expect("merged native history"),
            )
            .await
            .expect("atomic reverse native history import");
    }

    #[tokio::test]
    async fn public_facade_preserves_owner_denial_and_restrictive_resolution_after_reopen() {
        let path =
            std::env::temp_dir().join(format!("management-conflicts-{}.redb", Uuid::now_v7()));
        let restricted = Uuid::now_v7();
        let ambiguous = Uuid::now_v7();
        let namespace = Uuid::now_v7();
        {
            let first = Arc::new(Database::open(&path).expect("open local policy"));
            let second = ofdb_sql::Database::in_memory();
            crate::ofdb::schema::up(&first)
                .await
                .expect("initialize management only");
            let repo = DbSelectionPolicyRepo::new(Arc::clone(&first).client());
            for device_id in [restricted, ambiguous] {
                repo.set_prevalidated(SelectionPolicy {
                    device_id,
                    owner_subject: "owner".into(),
                    application_id: None,
                    selected_kind: None,
                    selected_id: None,
                    admin_allowed: true,
                })
                .await
                .expect("persist unselected policy");
            }
            sync_replica(&first, &second).await;
            for (database, updates) in [
                (
                    &*first,
                    vec![
                        (restricted, "admin_allowed", "false"),
                        (ambiguous, "owner_subject", "'alice'"),
                    ],
                ),
                (
                    &second,
                    vec![
                        (restricted, "admin_allowed", "false"),
                        (restricted, "admin_allowed", "true"),
                        (ambiguous, "owner_subject", "'bob'"),
                    ],
                ),
            ] {
                for (device, column, value) in updates {
                    database.client().translate_and_execute(
                        &format!("UPDATE device_selection_policies SET {column} = {value} WHERE device_id = CAST('{device}' AS UUID)"),
                        &ofdb_sql::SqlTranslator,
                    ).await.expect("diverge trusted policy fixture");
                }
            }
            sync_replica(&first, &second).await;
            assert!(
                repo.observe_owner_prevalidated(ambiguous, "alice")
                    .await
                    .is_err()
            );
            assert!(
                matches!(repo.observe_owner_prevalidated(restricted, "owner").await.expect("restrictive coherent observation"), of_model::device::selection_policy::OwnerPolicyObservation::Ready { policy, resources } if !policy.admin_allowed && resources.is_empty())
            );
            assert!(repo.get(ambiguous, "alice", namespace).await.is_err());
            let policy = repo
                .get(restricted, "owner", namespace)
                .await
                .expect("resolve only restrictive administrator conflict")
                .expect("policy remains present");
            assert!(!policy.admin_allowed);
            assert_eq!(policy.selected_id, None);
        }
        {
            let database = Arc::new(Database::open(&path).expect("reopen offline management"));
            let repo = DbSelectionPolicyRepo::new(database.client());
            assert!(repo.get(ambiguous, "alice", namespace).await.is_err());
            assert!(
                !repo
                    .get(restricted, "owner", namespace)
                    .await
                    .expect("persisted restriction")
                    .expect("policy remains present")
                    .admin_allowed
            );
        }
        std::fs::remove_file(path).expect("remove management conflict fixture");
    }

    #[tokio::test]
    async fn sync_requires_two_approved_matching_selected_devices() {
        let engine = Arc::new(ofdb_sql::Database::in_memory());
        crate::ofdb::schema::up(&engine)
            .await
            .expect("initialize replica");
        let devices = DbDeviceRepo::new(Arc::clone(&engine).client());
        let policies = DbSelectionPolicyRepo::new(engine.client());
        let local = devices
            .create(
                "owner".into(),
                "local".into(),
                "local-key".into(),
                "addr".into(),
                vec![],
                0,
            )
            .await
            .expect("create local device");
        let remote = devices
            .create(
                "owner".into(),
                "remote".into(),
                "remote-key".into(),
                "addr".into(),
                vec![1],
                i64::MAX,
            )
            .await
            .expect("create remote device");
        let application_id = ofdb_sql::Uuid::now_v7();
        let resource_id = ofdb_sql::Uuid::now_v7();
        let selected = |device_id| SelectionPolicy {
            device_id,
            owner_subject: "owner".into(),
            application_id: Some(application_id),
            selected_kind: Some("database".into()),
            selected_id: Some(resource_id),
            admin_allowed: true,
        };
        for device in [&local, &remote] {
            policies
                .set_prevalidated(selected(device.id))
                .await
                .expect("select resource");
        }
        let admitted = || {
            policies.peers_selected_for_sync(
                &devices,
                "local-key",
                "remote-key",
                "owner",
                application_id,
                "database",
                resource_id,
            )
        };
        assert_eq!(remote.state, DeviceState::Pending);
        assert!(!admitted().await.expect("pending peer denied"));
        devices
            .approve(remote.id, &[1])
            .await
            .expect("approve peer")
            .expect("pending peer exists");
        assert!(admitted().await.expect("matching peers admitted"));
        assert!(
            policies
                .selected_resources_for_device(remote.id)
                .await
                .expect("list selected resources")
                .iter()
                .any(|resource| {
                    resource.kind == "database" && resource.resource_id == resource_id
                })
        );
        let filesystem_id = ofdb_sql::Uuid::now_v7();
        for device in [&local, &remote] {
            policies
                .set_prevalidated(SelectionPolicy {
                    device_id: device.id,
                    owner_subject: "owner".into(),
                    application_id: Some(application_id),
                    selected_kind: Some("filesystem".into()),
                    selected_id: Some(filesystem_id),
                    admin_allowed: true,
                })
                .await
                .expect("select filesystem without replacing database");
        }
        assert!(admitted().await.expect("database remains selected"));
        assert!(
            policies
                .peers_selected_for_sync(
                    &devices,
                    "local-key",
                    "remote-key",
                    "owner",
                    application_id,
                    "filesystem",
                    filesystem_id,
                )
                .await
                .expect("filesystem selected on both peers")
        );
        for (owner, application, kind, id) in [
            ("other", application_id, "database", resource_id),
            ("owner", ofdb_sql::Uuid::now_v7(), "database", resource_id),
            ("owner", application_id, "filesystem", resource_id),
            (
                "owner",
                application_id,
                "database",
                ofdb_sql::Uuid::now_v7(),
            ),
        ] {
            assert!(!matches!(
                policies
                    .peers_selected_for_sync(
                        &devices,
                        "local-key",
                        "remote-key",
                        owner,
                        application,
                        kind,
                        id,
                    )
                    .await,
                Ok(true)
            ));
        }
        assert!(
            !policies
                .peers_selected_for_sync(
                    &devices,
                    "local-key",
                    "unknown",
                    "owner",
                    application_id,
                    "database",
                    resource_id,
                )
                .await
                .expect("unknown peer denied")
        );
        policies
            .deselect_owned(remote.id, "owner")
            .await
            .expect("deselect peer");
        assert!(
            policies
                .selected_resources_for_device(remote.id)
                .await
                .expect("list after deselection")
                .is_empty()
        );
        assert!(!admitted().await.expect("deselected peer denied"));
        policies
            .set_prevalidated(selected(remote.id))
            .await
            .expect("reselect peer");
        policies
            .set_admin_allowed_prevalidated(remote.id, "owner", false)
            .await
            .expect("restrict peer");
        assert!(
            policies
                .selected_resources_for_device(remote.id)
                .await
                .expect("list restricted device selections")
                .iter()
                .any(|resource| {
                    resource.kind == "database" && resource.resource_id == resource_id
                })
        );
        assert!(!admitted().await.expect("restricted peer denied"));
        policies
            .set_admin_allowed_prevalidated(remote.id, "owner", true)
            .await
            .expect("restore peer");
        devices
            .revoke("owner", remote.id, "local-key")
            .await
            .expect("revoke remote peer");
        assert!(!admitted().await.expect("revoked peer denied"));
    }

    #[tokio::test]
    async fn replicated_revocation_blocks_previously_selected_peer() {
        let source = Arc::new(ofdb_sql::Database::in_memory());
        let destination = Arc::new(ofdb_sql::Database::in_memory());
        crate::ofdb::schema::up(&source)
            .await
            .expect("initialize source replica");

        let devices = DbDeviceRepo::new(Arc::clone(&source).client());
        let policies = DbSelectionPolicyRepo::new(Arc::clone(&source).client());
        let local = devices
            .create(
                "owner".into(),
                "local".into(),
                "local-key".into(),
                "addr".into(),
                vec![],
                0,
            )
            .await
            .expect("create local device");
        let remote = devices
            .create(
                "owner".into(),
                "remote".into(),
                "remote-key".into(),
                "addr".into(),
                vec![1],
                i64::MAX,
            )
            .await
            .expect("create remote device");
        devices
            .approve(remote.id, &[1])
            .await
            .expect("approve remote device")
            .expect("remote device exists");

        let application_id = ofdb_sql::Uuid::now_v7();
        let resource_id = ofdb_sql::Uuid::now_v7();
        for device in [&local, &remote] {
            policies
                .set_prevalidated(SelectionPolicy {
                    device_id: device.id,
                    owner_subject: "owner".into(),
                    application_id: Some(application_id),
                    selected_kind: Some("database".into()),
                    selected_id: Some(resource_id),
                    admin_allowed: true,
                })
                .await
                .expect("select resource");
        }
        sync_replica(&source, &destination).await;

        let destination_devices = DbDeviceRepo::new(Arc::clone(&destination).client());
        let destination_policies = DbSelectionPolicyRepo::new(Arc::clone(&destination).client());
        assert!(
            destination_policies
                .peers_selected_for_sync(
                    &destination_devices,
                    "local-key",
                    "remote-key",
                    "owner",
                    application_id,
                    "database",
                    resource_id,
                )
                .await
                .expect("initial selection admits peer")
        );

        devices
            .revoke("owner", remote.id, "local-key")
            .await
            .expect("revoke remote device");
        assert!(
            destination_policies
                .peers_selected_for_sync(
                    &destination_devices,
                    "local-key",
                    "remote-key",
                    "owner",
                    application_id,
                    "database",
                    resource_id,
                )
                .await
                .expect("stale replica remains temporarily permissive"),
            "a peer may sync only while its local policy has not converged"
        );
        sync_replica(&source, &destination).await;
        assert!(
            !destination_policies
                .peers_selected_for_sync(
                    &destination_devices,
                    "local-key",
                    "remote-key",
                    "owner",
                    application_id,
                    "database",
                    resource_id,
                )
                .await
                .expect("replicated revocation denies peer")
        );
    }

    #[tokio::test]
    async fn selection_cannot_lift_local_or_replicated_admin_restriction() {
        let first = Arc::new(ofdb_sql::Database::in_memory());
        let second = Arc::new(ofdb_sql::Database::in_memory());
        crate::ofdb::schema::up(&first)
            .await
            .expect("initialize replica");
        let first_repo = DbSelectionPolicyRepo::new(Arc::clone(&first).client());
        let second_repo = DbSelectionPolicyRepo::new(Arc::clone(&second).client());
        let device_id = ofdb_sql::Uuid::now_v7();
        let application_id = ofdb_sql::Uuid::now_v7();
        let mut stale = SelectionPolicy {
            device_id,
            owner_subject: "owner".into(),
            application_id: Some(application_id),
            selected_kind: Some("database".into()),
            selected_id: Some(ofdb_sql::Uuid::now_v7()),
            admin_allowed: true,
        };
        first_repo
            .set_prevalidated(stale.clone())
            .await
            .expect("create policy");
        sync_replica(&first, &second).await;
        assert!(
            !second_repo
                .set_admin_allowed_prevalidated(ofdb_sql::Uuid::now_v7(), " ", false)
                .await
                .expect("missing owner")
        );
        assert!(
            !second_repo
                .set_admin_allowed_prevalidated(device_id, "other", false)
                .await
                .expect("wrong owner")
        );
        assert!(
            second_repo
                .set_admin_allowed_prevalidated(device_id, "owner", false)
                .await
                .expect("restrict admin")
        );
        assert!(
            second_repo
                .set_admin_allowed_prevalidated(device_id, "owner", false)
                .await
                .expect("restriction already set")
        );

        stale.selected_id = Some(ofdb_sql::Uuid::now_v7());
        first_repo
            .set_prevalidated(stale.clone())
            .await
            .expect("offline selection");
        sync_replica(&first, &second).await;
        sync_replica(&second, &first).await;
        for (engine, repo) in [(&first, &first_repo), (&second, &second_repo)] {
            assert!(
                engine
                    .client()
                    .row_conflicts(TABLE, &key(device_id))
                    .await
                    .expect("read conflicts")
                    .is_empty()
            );
            let mut expected = stale.clone();
            expected.admin_allowed = false;
            assert_eq!(
                repo.get(device_id, "owner", application_id)
                    .await
                    .expect("read restricted selection"),
                Some(expected)
            );
        }

        stale.admin_allowed = false;
        let expected = stale.clone();
        stale.selected_id = Some(ofdb_sql::Uuid::now_v7());
        assert!(first_repo.set_prevalidated(stale).await.is_err());
        assert_eq!(
            first_repo
                .get(device_id, "owner", application_id)
                .await
                .expect("read local restriction"),
            Some(expected)
        );
    }

    #[tokio::test]
    async fn unselected_admin_restriction_survives_restart_and_blocks_selection() {
        let path = std::env::temp_dir().join(format!(
            "selection-policy-{}.redb",
            ofdb_sql::Uuid::now_v7()
        ));
        let device_id = ofdb_sql::Uuid::now_v7();
        let application_id = ofdb_sql::Uuid::now_v7();
        {
            let engine = Arc::new(Database::open(&path).expect("open database"));
            crate::ofdb::schema::up(&engine)
                .await
                .expect("create schema");
            let repo = DbSelectionPolicyRepo::new(engine.client());
            assert!(
                repo.set_admin_allowed_prevalidated(device_id, "owner", false)
                    .await
                    .expect("restrict unselected device")
            );
            assert_eq!(
                repo.get(device_id, "owner", application_id)
                    .await
                    .expect("read policy")
                    .expect("policy exists")
                    .selected_id,
                None
            );
        }
        {
            let engine = Arc::new(Database::open(&path).expect("reopen database"));
            let repo = DbSelectionPolicyRepo::new(engine.client());
            assert!(
                !repo
                    .get(device_id, "owner", application_id)
                    .await
                    .expect("read restriction")
                    .expect("policy exists")
                    .admin_allowed
            );
            assert!(
                repo.set_prevalidated(SelectionPolicy {
                    device_id,
                    owner_subject: "owner".into(),
                    application_id: Some(application_id),
                    selected_kind: Some("database".into()),
                    selected_id: Some(ofdb_sql::Uuid::now_v7()),
                    admin_allowed: true,
                })
                .await
                .is_err()
            );
            assert!(
                repo.deselect_owned(device_id, "owner")
                    .await
                    .expect("owner can deselect")
            );
            assert!(
                !repo
                    .get(device_id, "owner", application_id)
                    .await
                    .expect("read restriction")
                    .expect("policy exists")
                    .admin_allowed
            );
        }
        std::fs::remove_file(path).expect("remove test database");
    }

    #[tokio::test]
    async fn learned_owner_change_clears_selection_and_keeps_new_owner_gated_after_reopen() {
        let root =
            std::env::temp_dir().join(format!("learned-owner-{}.redb", ofdb_sql::Uuid::now_v7()));
        let device_id = ofdb_sql::Uuid::now_v7();
        let app = ofdb_sql::Uuid::now_v7();
        let old = ofdb_sql::Uuid::now_v7().to_string();
        let new = ofdb_sql::Uuid::now_v7().to_string();
        {
            let engine = std::sync::Arc::new(ofdb_sql::Database::open(&root).expect("open policy"));
            super::super::up(&engine).await.expect("initialize policy");
            let repo = crate::ofdb::selection_policy::DbSelectionPolicyRepo::new(engine.client());
            repo.set_prevalidated(of_model::device::selection_policy::SelectionPolicy {
                device_id,
                owner_subject: old.clone(),
                application_id: Some(app),
                selected_kind: Some("database".into()),
                selected_id: Some(ofdb_sql::Uuid::now_v7()),
                admin_allowed: true,
            })
            .await
            .expect("select under old local view");
            assert_eq!(
                repo.selected_resources_for_device(device_id)
                    .await
                    .expect("unaware selection")
                    .len(),
                1
            );
            assert!(
                !repo
                    .reconcile_owner_prevalidated(device_id, &new)
                    .await
                    .expect("learn owner change")
            );
            assert!(
                repo.selected_resources_for_device(device_id)
                    .await
                    .expect("old selection cleared")
                    .is_empty()
            );
            assert!(
                !repo
                    .reconcile_owner_prevalidated(device_id, &new)
                    .await
                    .expect("retry stays gated")
            );
        }
        {
            let engine =
                std::sync::Arc::new(ofdb_sql::Database::open(&root).expect("reopen policy"));
            let repo = crate::ofdb::selection_policy::DbSelectionPolicyRepo::new(engine.client());
            assert!(repo.get(device_id, &new, app).await.is_err());
            assert!(
                !repo
                    .reconcile_owner_prevalidated(device_id, &old)
                    .await
                    .expect("owner rollback cannot bypass learned cleanup")
            );
            assert!(
                repo.selected_resources_for_device(device_id)
                    .await
                    .expect("cleared durably")
                    .is_empty()
            );
            assert!(
                !repo
                    .reconcile_owner_prevalidated(device_id, &new)
                    .await
                    .expect("restart stays gated")
            );
        }
        std::fs::remove_file(root).expect("remove policy fixture");
    }

    #[tokio::test]
    async fn completed_local_transfer_rebinds_empty_policy_without_lifting_restriction() {
        let engine = Arc::new(ofdb_sql::Database::in_memory());
        super::super::up(&engine).await.expect("schema");
        let repo = crate::ofdb::selection_policy::DbSelectionPolicyRepo::new(engine.client());
        let device = ofdb_sql::Uuid::now_v7();
        let old = ofdb_sql::Uuid::now_v7().to_string();
        let new = ofdb_sql::Uuid::now_v7().to_string();
        repo.set_prevalidated(of_model::device::selection_policy::SelectionPolicy {
            device_id: device,
            owner_subject: old.clone(),
            application_id: Some(ofdb_sql::Uuid::now_v7()),
            selected_kind: Some("database".into()),
            selected_id: Some(ofdb_sql::Uuid::now_v7()),
            admin_allowed: true,
        })
        .await
        .expect("old selection");
        assert!(
            !repo
                .reconcile_owner_prevalidated(device, &new)
                .await
                .expect("learn transfer")
        );
        assert_eq!(
            repo.pending_cleanup_owner(device).await.expect("intent"),
            Some(old.clone())
        );
        assert!(
            !repo
                .complete_owner_cleanup_prevalidated(device, &new, &old)
                .await
                .expect("wrong intent denies")
        );
        assert!(
            repo.local_cleanup_pending(device)
                .await
                .expect("still pending")
        );
        assert!(
            repo.complete_owner_cleanup_prevalidated(device, &old, &new)
                .await
                .expect("trusted host completed both owners")
        );
        assert!(
            !repo
                .local_cleanup_pending(device)
                .await
                .expect("completed atomically")
        );
        assert!(
            repo.reconcile_owner_prevalidated(device, &new)
                .await
                .expect("new binding ready")
        );
        let policy = repo
            .get(device, &new, ofdb_sql::Uuid::nil())
            .await
            .expect("rebound")
            .expect("policy");
        assert!(!policy.admin_allowed);
        assert!(policy.selected_id.is_none());
        assert!(
            repo.selected_resources_for_device(device)
                .await
                .expect("old selections gone")
                .is_empty()
        );
        assert!(
            repo.complete_owner_cleanup_prevalidated(device, &old, &new)
                .await
                .expect("idempotent completion")
        );
    }

    #[tokio::test]
    async fn concurrent_reselection_and_deselection_resolves_to_deselected() {
        let first = Arc::new(ofdb_sql::Database::in_memory());
        let second = Arc::new(ofdb_sql::Database::in_memory());
        crate::ofdb::schema::up(&first)
            .await
            .expect("initialize first replica");
        let first_devices = DbDeviceRepo::new(Arc::clone(&first).client());
        let second_devices = DbDeviceRepo::new(Arc::clone(&second).client());
        let local = first_devices
            .create(
                "owner".into(),
                "local".into(),
                "local-key".into(),
                "addr".into(),
                vec![],
                0,
            )
            .await
            .expect("create local device");
        let remote = first_devices
            .create_pairing(
                "remote".into(),
                "remote-key".into(),
                "addr".into(),
                "local-key".into(),
            )
            .await
            .expect("pair remote device");
        first_devices
            .approve_pairing(remote.id)
            .await
            .expect("approve peer")
            .expect("paired peer exists");
        let first_repo = DbSelectionPolicyRepo::new(Arc::clone(&first).client());
        let second_repo = DbSelectionPolicyRepo::new(Arc::clone(&second).client());
        let device_id = remote.id;
        let application_id = ofdb_sql::Uuid::now_v7();
        let mut selected = SelectionPolicy {
            device_id,
            owner_subject: "owner".into(),
            application_id: Some(application_id),
            selected_kind: Some("database".into()),
            selected_id: Some(ofdb_sql::Uuid::now_v7()),
            admin_allowed: true,
        };
        first_repo
            .set_prevalidated(SelectionPolicy {
                device_id: local.id,
                ..selected.clone()
            })
            .await
            .expect("select local peer");

        first_repo
            .set_prevalidated(selected.clone())
            .await
            .expect("create shared selection");
        sync_replica(&first, &second).await;
        assert_eq!(
            second_repo
                .get(device_id, "owner", application_id)
                .await
                .expect("shared selection"),
            Some(selected.clone())
        );
        for (repo, devices) in [
            (&first_repo, &first_devices),
            (&second_repo, &second_devices),
        ] {
            assert!(
                repo.peers_selected_for_sync(
                    devices,
                    "local-key",
                    "remote-key",
                    "owner",
                    application_id,
                    "database",
                    selected.selected_id.expect("selected resource"),
                )
                .await
                .expect("replicated peers admitted")
            );
        }

        selected.selected_id = Some(ofdb_sql::Uuid::now_v7());
        first_repo
            .set_prevalidated(selected.clone())
            .await
            .expect("reselect offline");
        let mut deselected = selected.clone();
        deselected.application_id = None;
        deselected.selected_kind = None;
        deselected.selected_id = None;
        second_repo
            .set_prevalidated(deselected.clone())
            .await
            .expect("deselect offline");
        sync_replica(&first, &second).await;
        sync_replica(&second, &first).await;

        for (engine, repo, devices) in [
            (&first, &first_repo, &first_devices),
            (&second, &second_repo, &second_devices),
        ] {
            assert_eq!(
                repo.get(device_id, "owner", application_id)
                    .await
                    .expect("deselection wins concurrent selection"),
                Some(deselected.clone())
            );
            assert!(
                engine
                    .client()
                    .row_conflicts(TABLE, &key(device_id))
                    .await
                    .expect("read conflicts after deterministic resolution")
                    .is_empty()
            );
            assert!(
                !repo
                    .peers_selected_for_sync(
                        devices,
                        "local-key",
                        "remote-key",
                        "owner",
                        application_id,
                        "database",
                        selected.selected_id.expect("selected resource"),
                    )
                    .await
                    .expect("deselected peers are denied")
            );
        }
        sync_replica(&first, &second).await;
        sync_replica(&second, &first).await;
        for repo in [&first_repo, &second_repo] {
            assert_eq!(
                repo.get(device_id, "owner", application_id)
                    .await
                    .expect("read replicated deselection"),
                Some(deselected.clone())
            );
        }

        first_repo
            .set_prevalidated(selected.clone())
            .await
            .expect("deliberately reselect");
        sync_replica(&first, &second).await;
        sync_replica(&second, &first).await;
        for repo in [&first_repo, &second_repo] {
            assert_eq!(
                repo.get(device_id, "owner", application_id)
                    .await
                    .expect("read reselect"),
                Some(selected.clone())
            );
        }
    }

    #[tokio::test]
    async fn owner_and_admin_conflicts_cannot_be_deselected() {
        let first = Arc::new(ofdb_sql::Database::in_memory());
        let second = Arc::new(ofdb_sql::Database::in_memory());
        crate::ofdb::schema::up(&first)
            .await
            .expect("initialize replica");
        let first_repo = DbSelectionPolicyRepo::new(Arc::clone(&first).client());
        let second_repo = DbSelectionPolicyRepo::new(Arc::clone(&second).client());
        let device_id = ofdb_sql::Uuid::now_v7();
        let application_id = ofdb_sql::Uuid::now_v7();
        let selected = SelectionPolicy {
            device_id,
            owner_subject: "owner".into(),
            application_id: Some(application_id),
            selected_kind: Some("database".into()),
            selected_id: Some(ofdb_sql::Uuid::now_v7()),
            admin_allowed: true,
        };
        first_repo
            .set_prevalidated(selected.clone())
            .await
            .expect("initial selection");
        sync_replica(&first, &second).await;

        for (engine, owner) in [(&first, "other"), (&second, "third")] {
            engine
                .client()
                .execute(vec![ofdb_sql::Statement::Query(ofdb_sql::Query::Update(
                    ofdb_sql::QueryUpdate {
                        from: from(),
                        assignments: vec![assignment("owner_subject", Value::Text(owner.into()))],
                        predicate: Some(super::equals("device_id", Value::Uuid(device_id))),
                        returning: None,
                    },
                ))])
                .await
                .expect("concurrent owner change");
        }
        sync_replica(&first, &second).await;
        sync_replica(&second, &first).await;
        assert!(
            first
                .client()
                .row_conflicts(TABLE, &key(device_id))
                .await
                .expect("owner conflicts")
                .contains(&"owner_subject".into())
        );
        for repo in [&first_repo, &second_repo] {
            for owner in ["owner", "other", "third"] {
                assert!(repo.deselect_owned(device_id, owner).await.is_err());
            }
        }

        let other_device = ofdb_sql::Uuid::now_v7();
        let mut other_policy = selected.clone();
        other_policy.device_id = other_device;
        first_repo
            .set_prevalidated(other_policy.clone())
            .await
            .expect("second selection");
        sync_replica(&first, &second).await;
        for (engine, allowed) in [(&first, false), (&second, false), (&second, true)] {
            engine
                .client()
                .execute(vec![ofdb_sql::Statement::Query(ofdb_sql::Query::Update(
                    ofdb_sql::QueryUpdate {
                        from: from(),
                        assignments: vec![assignment("admin_allowed", Value::Bool(allowed))],
                        predicate: Some(super::equals("device_id", Value::Uuid(other_device))),
                        returning: None,
                    },
                ))])
                .await
                .expect("concurrent admin change");
        }
        sync_replica(&first, &second).await;
        sync_replica(&second, &first).await;
        assert!(
            first
                .client()
                .row_conflicts(TABLE, &key(other_device))
                .await
                .expect("admin conflicts")
                .contains(&"admin_allowed".into())
        );
        for repo in [&first_repo, &second_repo] {
            assert!(repo.deselect_owned(other_device, "owner").await.is_err());
            assert!(
                !repo
                    .get(other_device, "owner", application_id)
                    .await
                    .expect("admin restriction conflict resolves fail-closed")
                    .expect("selection remains stored")
                    .admin_allowed
            );
        }
    }

    #[tokio::test]
    async fn selection_survives_reopen_and_rejects_wrong_namespace() {
        let path = std::env::temp_dir().join(format!(
            "management-selection-{}-{}.redb",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        let device_id = ofdb_sql::Uuid::now_v7();
        let application_id = ofdb_sql::Uuid::now_v7();
        let selected_id = ofdb_sql::Uuid::now_v7();
        let policy = SelectionPolicy {
            device_id,
            owner_subject: "owner".into(),
            application_id: Some(application_id),
            selected_kind: Some("database".into()),
            selected_id: Some(selected_id),
            admin_allowed: false,
        };
        {
            let engine: Arc<Database> = Arc::new(Database::open(&path).expect("open database"));
            crate::ofdb::schema::up(&engine)
                .await
                .expect("migrate database");
            DbSelectionPolicyRepo::new(engine.client())
                .set_prevalidated(policy.clone())
                .await
                .expect("write validated selection");
        }
        {
            let engine: Arc<Database> = Arc::new(Database::open(&path).expect("reopen database"));
            let repo = DbSelectionPolicyRepo::new(engine.client());
            assert_eq!(
                repo.get(device_id, "owner", application_id)
                    .await
                    .expect("read selection"),
                Some(policy)
            );
            assert!(repo.get(device_id, "other", application_id).await.is_err());
        }
        std::fs::remove_file(path).expect("remove database");
    }

    #[tokio::test]
    async fn offline_deselection_keeps_admin_restriction_and_checks_owner() {
        let engine = Arc::new(ofdb_sql::Database::in_memory());
        crate::ofdb::schema::up(&engine)
            .await
            .expect("initialize replica schema");
        let repo = DbSelectionPolicyRepo::new(engine.client());
        let device_id = ofdb_sql::Uuid::now_v7();
        let application_id = ofdb_sql::Uuid::now_v7();
        let mut policy = SelectionPolicy {
            device_id,
            owner_subject: "owner".into(),
            application_id: Some(application_id),
            selected_kind: Some("database".into()),
            selected_id: Some(ofdb_sql::Uuid::now_v7()),
            admin_allowed: false,
        };
        repo.set_prevalidated(policy.clone())
            .await
            .expect("set selection");
        assert!(
            !repo
                .deselect_owned(ofdb_sql::Uuid::now_v7(), "owner")
                .await
                .expect("missing policy")
        );
        assert!(
            !repo
                .deselect_owned(device_id, "other")
                .await
                .expect("wrong owner")
        );
        assert_eq!(
            repo.get(device_id, "owner", application_id)
                .await
                .expect("unchanged selection"),
            Some(policy.clone())
        );
        assert!(
            repo.deselect_owned(device_id, "owner")
                .await
                .expect("offline deselection")
        );
        assert!(
            repo.deselect_owned(device_id, "owner")
                .await
                .expect("already deselected")
        );
        policy.application_id = None;
        policy.selected_kind = None;
        policy.selected_id = None;
        assert_eq!(
            repo.get(device_id, "owner", application_id)
                .await
                .expect("read policy"),
            Some(policy)
        );
    }

    #[tokio::test]
    async fn admin_restriction_without_selection_and_switch_after_deselect() {
        let engine = Arc::new(ofdb_sql::Database::in_memory());
        crate::ofdb::schema::up(&engine)
            .await
            .expect("initialize replica schema");
        let repo = DbSelectionPolicyRepo::new(engine.client());
        let device_id = ofdb_sql::Uuid::now_v7();
        let app_a = ofdb_sql::Uuid::now_v7();
        let app_b = ofdb_sql::Uuid::now_v7();
        let mut policy = SelectionPolicy {
            device_id,
            owner_subject: "owner".into(),
            application_id: None,
            selected_kind: None,
            selected_id: None,
            admin_allowed: false,
        };
        repo.set_prevalidated(policy.clone())
            .await
            .expect("restrict admin with no selection");
        assert_eq!(
            repo.get(device_id, "owner", app_b)
                .await
                .expect("read unselected policy"),
            Some(policy.clone())
        );
        assert!(repo.get(device_id, "other", app_b).await.is_err());

        assert!(
            repo.set_prevalidated(SelectionPolicy {
                application_id: Some(app_a),
                selected_kind: Some("database".into()),
                selected_id: Some(ofdb_sql::Uuid::now_v7()),
                ..policy.clone()
            })
            .await
            .is_err()
        );
        repo.set_admin_allowed_prevalidated(device_id, "owner", true)
            .await
            .expect("lift restriction");
        policy.admin_allowed = true;
        policy.application_id = Some(app_a);
        policy.selected_kind = Some("database".into());
        policy.selected_id = Some(ofdb_sql::Uuid::now_v7());
        repo.set_prevalidated(policy.clone())
            .await
            .expect("select app A");
        let mut app_a_filesystem = policy.clone();
        app_a_filesystem.selected_kind = Some("filesystem".into());
        app_a_filesystem.selected_id = Some(ofdb_sql::Uuid::now_v7());
        repo.set_prevalidated(app_a_filesystem.clone())
            .await
            .expect("select filesystem alongside database");
        let mut app_b_policy = policy.clone();
        app_b_policy.application_id = Some(app_b);
        app_b_policy.selected_id = Some(ofdb_sql::Uuid::now_v7());
        repo.set_prevalidated(app_b_policy.clone())
            .await
            .expect("select app B without replacing app A");
        assert!(
            repo.resource_selected(
                device_id,
                "owner",
                app_a,
                "database",
                policy.selected_id.expect("app A resource")
            )
            .await
            .expect("check app A database")
        );
        assert!(
            repo.resource_selected(
                device_id,
                "owner",
                app_a,
                "filesystem",
                app_a_filesystem.selected_id.expect("app A filesystem")
            )
            .await
            .expect("check app A filesystem")
        );
        let app_a_resource = policy.selected_id.expect("app A resource");
        let app_a_filesystem_id = app_a_filesystem.selected_id.expect("app A filesystem");
        let selected = repo
            .selected_resources_for_device(device_id)
            .await
            .expect("list selected resources");
        assert_eq!(selected.len(), 3);
        assert!(selected.iter().any(|resource| {
            resource.application_id == app_a
                && resource.kind == "database"
                && resource.resource_id == app_a_resource
        }));
        assert!(selected.iter().any(|resource| {
            resource.application_id == app_a
                && resource.kind == "filesystem"
                && resource.resource_id == app_a_filesystem_id
        }));
        assert!(
            repo.deselect_resource_owned(
                device_id,
                "owner",
                app_a,
                "filesystem",
                app_a_filesystem_id,
            )
            .await
            .expect("deselect only app A filesystem")
        );
        assert!(
            repo.resource_selected(device_id, "owner", app_a, "database", app_a_resource)
                .await
                .expect("database remains selected")
        );
        let selected = repo
            .selected_resources_for_device(device_id)
            .await
            .expect("list resources after deselection");
        assert_eq!(selected.len(), 2);
        assert!(!selected.iter().any(|resource| {
            resource.application_id == app_a
                && resource.kind == "filesystem"
                && resource.resource_id == app_a_filesystem_id
        }));
        repo.set_prevalidated(app_a_filesystem.clone())
            .await
            .expect("reselect app A filesystem");
        let app_b_resource = app_b_policy.selected_id.expect("app B resource");

        policy.application_id = None;
        policy.selected_kind = None;
        policy.selected_id = None;
        repo.set_prevalidated(policy.clone())
            .await
            .expect("deselect app A");
        assert_eq!(
            repo.get(device_id, "owner", app_b)
                .await
                .expect("read after deselect"),
            Some(policy.clone())
        );
        repo.set_prevalidated(app_b_policy.clone())
            .await
            .expect("select app B");
        assert_eq!(
            repo.get(device_id, "owner", app_b)
                .await
                .expect("read app B"),
            Some(app_b_policy.clone())
        );
        assert!(
            !repo
                .resource_selected(device_id, "owner", app_a, "database", app_a_resource)
                .await
                .expect("app A was deselected")
        );
        assert!(
            !repo
                .resource_selected(device_id, "owner", app_a, "filesystem", app_a_filesystem_id,)
                .await
                .expect("app A filesystem was deselected")
        );
        assert!(
            repo.resource_selected(device_id, "owner", app_b, "database", app_b_resource)
                .await
                .expect("app B remains selected")
        );
    }
}

fn policy_query(device_id: Uuid) -> Query {
    Query::Select(QuerySelect {
        from: from(),
        projection: COLUMNS.into_iter().map(column).collect(),
        distinct: false,
        predicate: Some(QueryExpr::And(
            Box::new(equals("device_id", Value::Uuid(device_id))),
            Box::new(equals("device_key", Value::Text(device_id.to_string()))),
        )),
        aggregates: vec![],
        group_by: vec![],
        order_by: vec![],
        text_concats: vec![],
        limit: None,
        offset: None,
        having: None,
    })
}

fn cleanup_device(device_id: Uuid) -> QueryExpr {
    QueryExpr::Equals(
        Box::new(QueryExpr::Value(QueryExprValue::Column(QueryColumn::new(
            "local_device_copy_cleanup".into(),
            "device_id".into(),
        )))),
        Box::new(QueryExpr::Value(QueryExprValue::Value(Value::Text(
            device_id.to_string(),
        )))),
    )
}

fn cleanup_query(device_id: Uuid, pending_only: bool) -> Query {
    let table = "local_device_copy_cleanup";
    let column = QueryColumn::new(table.into(), "device_id".into());
    Query::Select(QuerySelect {
        from: QueryFrom {
            table: table.into(),
            joins: vec![],
        },
        projection: vec![
            column,
            QueryColumn::new(table.into(), "cleanup_id".into()),
            QueryColumn::new(table.into(), "completed".into()),
        ],
        distinct: false,
        predicate: Some(if pending_only {
            QueryExpr::And(
                Box::new(cleanup_device(device_id)),
                Box::new(QueryExpr::Equals(
                    Box::new(QueryExpr::Value(QueryExprValue::Column(QueryColumn::new(
                        table.into(),
                        "completed".into(),
                    )))),
                    Box::new(QueryExpr::Value(QueryExprValue::Value(Value::Bool(false)))),
                )),
            )
        } else {
            cleanup_device(device_id)
        }),
        aggregates: vec![],
        group_by: vec![],
        order_by: vec![],
        text_concats: vec![],
        limit: None,
        offset: None,
        having: None,
    })
}

fn resource_query(device_id: Uuid) -> Query {
    Query::Select(QuerySelect {
        from: resource_from(),
        projection: [
            "id",
            "device_id",
            "owner_subject",
            "application_id",
            "selected_kind",
            "selected_id",
            "selected",
        ]
        .into_iter()
        .map(resource_column)
        .collect(),
        distinct: false,
        predicate: Some(resource_equals("device_id", Value::Uuid(device_id))),
        aggregates: vec![],
        group_by: vec![],
        order_by: vec![],
        text_concats: vec![],
        limit: None,
        offset: None,
        having: None,
    })
}
