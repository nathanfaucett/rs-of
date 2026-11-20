use chrono::{DateTime, Timelike, Utc};
use db::ofdb::validators::SecurityRow;
use db::ofdb::validators::SecurityTable;
use db::ofdb::validators::allows_authorization;
use of_model::id::Id;
use of_model::idp::model::permission::Permission;
use ofdb_sql::FromRow;
use ofdb_sql::FromRowError;
use ofdb_sql::Query;
use ofdb_sql::QueryColumn;
use ofdb_sql::QueryDelete;
use ofdb_sql::QueryExpr;
use ofdb_sql::QueryExprValue;
use ofdb_sql::QueryFrom;
use ofdb_sql::QueryParams;
use ofdb_sql::QuerySelect;
use ofdb_sql::Row;
use ofdb_sql::Statement;
use ofdb_sql::Uuid;
use ofdb_sql::Value;

use of_model::management_error::ManagementError;

use of_model::management_error::ManagementResult;

use super::insert::{active_rows, grant_ref, insert_policy_record, returned_id, revoke_policy};
use crate::PermissionRepo;

use of_model::management::SyncSelection;

const PERMISSIONS: &str = "permissions";
const ROLES: &str = "roles";
const ROLE_PERMISSIONS: &str = "role_permissions";
const SYNC_SELECTIONS: &str = "sync_selections";
const PERMISSION_COLUMNS: [&str; 9] = [
    "row_id",
    "incarnation",
    "id",
    "application_id",
    "name",
    "description",
    "revoked_at",
    "created_at",
    "updated_at",
];
const ROLE_COLUMNS: [&str; 5] = [
    "row_id",
    "id",
    "application_id",
    "revoked_at",
    "incarnation",
];
const ROLE_PERMISSION_COLUMNS: [&str; 4] = ["id", "role_id", "permission_id", "revoked_at"];
const SYNC_SELECTION_COLUMNS: [&str; 5] = [
    "device_id",
    "resource_id",
    "application_id",
    "owner_subject",
    "selected_at",
];

pub struct DbPermissionRepo {
    engine: ofdb_sql::Client,
}

impl DbPermissionRepo {
    #[must_use]
    pub const fn new(engine: ofdb_sql::Client) -> Self {
        Self { engine }
    }

    async fn rows<T: FromRow>(&self, query: Query) -> ManagementResult<Vec<T>> {
        let mut results = self
            .engine
            .execute(vec![Statement::Query(query)])
            .await
            .map_err(db_error)?;
        results
            .pop()
            .ok_or_else(|| ManagementError::InvalidInput("missing query result".into()))?
            .rows_as::<T>()
            .map_err(row_error)
    }

    async fn insert_permission(
        &self,
        domain_id: Option<Id>,
        application_id: Id,
        name: &str,
        description: Option<&str>,
    ) -> ManagementResult<Permission> {
        let now = now();
        let (id, row_id) = insert_policy_record(
            &self.engine,
            PERMISSIONS,
            domain_id,
            application_id,
            name,
            description,
            now.timestamp(),
        )
        .await?;
        self.ensure_clear(&[SecurityRow::new(SecurityTable::Permission, row_id)])
            .await?;
        Ok(Permission {
            id,
            application_id,
            name: name.into(),
            description: description.map(str::to_owned),
            created_at: now,
            updated_at: now,
        })
    }

    async fn ensure_clear(&self, rows: &[SecurityRow]) -> ManagementResult<()> {
        if allows_authorization(&self.engine, rows)
            .await
            .map_err(db_error)?
        {
            Ok(())
        } else {
            Err(ManagementError::InvalidInput(
                "conflicted permission state".into(),
            ))
        }
    }

    async fn permission(
        &self,
        application_id: Id,
        permission_id: Id,
    ) -> ManagementResult<Option<PermissionRow>> {
        let permission =
            active_rows::<PermissionRow>(&self.engine, PERMISSIONS, &PERMISSION_COLUMNS)
                .await?
                .into_iter()
                .find(|row| {
                    row.id == permission_id
                        && row.application_id == application_id
                        && row.revoked_at.is_none()
                });
        if let Some(permission) = &permission {
            self.ensure_clear(&[SecurityRow::new(
                SecurityTable::Permission,
                permission.row_id,
            )])
            .await?;
        }
        Ok(permission)
    }

    async fn role(&self, application_id: Id, role_id: Id) -> ManagementResult<Option<RoleRow>> {
        let role = active_rows::<RoleRow>(&self.engine, ROLES, &ROLE_COLUMNS)
            .await?
            .into_iter()
            .find(|row| {
                row.id == role_id
                    && row.application_id == application_id
                    && row.revoked_at.is_none()
            });
        if let Some(role) = &role {
            self.ensure_clear(&[SecurityRow::new(SecurityTable::Role, role.row_id)])
                .await?;
        }
        Ok(role)
    }

    async fn role_permission(
        &self,
        role_id: Id,
        permission_id: Id,
    ) -> ManagementResult<Option<RolePermissionRow>> {
        let relation = self
            .rows::<RolePermissionRow>(select(ROLE_PERMISSIONS, &ROLE_PERMISSION_COLUMNS))
            .await?
            .into_iter()
            .find(|row| {
                row.role_id == role_id
                    && row.permission_id == permission_id
                    && row.revoked_at.is_none()
            });
        if let Some(relation) = relation {
            self.ensure_clear(&[SecurityRow::new(SecurityTable::RolePermission, relation.id)])
                .await?;
            Ok(Some(relation))
        } else {
            Ok(None)
        }
    }
}

impl PermissionRepo for DbPermissionRepo {
    async fn list_permissions(
        &self,
        application_id: Id,
        offset: u32,
        limit: u32,
    ) -> ManagementResult<Vec<Permission>> {
        let permissions =
            active_rows::<PermissionRow>(&self.engine, PERMISSIONS, &PERMISSION_COLUMNS)
                .await?
                .into_iter()
                .filter(|row| row.application_id == application_id && row.revoked_at.is_none())
                .collect::<Vec<_>>();
        for permission in &permissions {
            self.ensure_clear(&[SecurityRow::new(
                SecurityTable::Permission,
                permission.row_id,
            )])
            .await?;
        }
        permissions
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .map(Permission::try_from)
            .collect()
    }

    async fn create_permission(
        &self,
        application_id: Id,
        name: &str,
        description: Option<&str>,
    ) -> ManagementResult<Permission> {
        self.insert_permission(None, application_id, name, description)
            .await
    }

    async fn create_permission_with_id(
        &self,
        id: Id,
        application_id: Id,
        name: &str,
        description: Option<&str>,
    ) -> ManagementResult<Permission> {
        if id.is_nil() {
            return Err(ManagementError::InvalidInput(
                "permission ID must not be nil".into(),
            ));
        }
        self.insert_permission(Some(id), application_id, name, description)
            .await
    }

    async fn find_permission_by_id(
        &self,
        application_id: Id,
        permission_id: Id,
    ) -> ManagementResult<Option<Permission>> {
        self.permission(application_id, permission_id)
            .await?
            .map(Permission::try_from)
            .transpose()
    }

    async fn delete_permission_by_id(
        &self,
        application_id: Id,
        permission_id: Id,
    ) -> ManagementResult<()> {
        let permission = self
            .permission(application_id, permission_id)
            .await?
            .ok_or_else(|| ManagementError::InvalidInput("permission not found".into()))?;
        revoke_policy(
            &self.engine,
            PERMISSIONS,
            application_id,
            permission.id,
            permission.incarnation,
            permission.row_id,
        )
        .await
    }

    async fn add_permission_to_role(
        &self,
        application_id: Id,
        role_id: Id,
        permission_id: Id,
    ) -> ManagementResult<()> {
        let role = self
            .role(application_id, role_id)
            .await?
            .ok_or_else(|| ManagementError::InvalidInput("role not found".into()))?;
        let permission = self
            .permission(application_id, permission_id)
            .await?
            .ok_or_else(|| ManagementError::InvalidInput("permission not found".into()))?;
        if self
            .role_permission(role.grant_ref(), permission.grant_ref())
            .await?
            .is_some()
        {
            return Ok(());
        }
        let params = QueryParams::Positional(vec![
            Value::Uuid(role.grant_ref()),
            Value::Uuid(permission.grant_ref()),
            Value::Integer(now().timestamp()),
        ]);
        let results = self.engine.execute_sql(
            "INSERT INTO role_permissions (role_id, permission_id, created_at, updated_at) VALUES ($1, $2, $3, $3) RETURNING id",
            Some(&params),
        ).await.map_err(db_error)?;
        self.ensure_clear(&[SecurityRow::new(
            SecurityTable::RolePermission,
            returned_id(&results)?,
        )])
        .await
    }

    async fn remove_permission_from_role(
        &self,
        application_id: Id,
        role_id: Id,
        permission_id: Id,
    ) -> ManagementResult<()> {
        let role = self
            .role(application_id, role_id)
            .await?
            .ok_or_else(|| ManagementError::InvalidInput("role not found".into()))?;
        let permission = self
            .permission(application_id, permission_id)
            .await?
            .ok_or_else(|| ManagementError::InvalidInput("permission not found".into()))?;
        let relation = self
            .role_permission(role.grant_ref(), permission.grant_ref())
            .await?
            .ok_or_else(|| ManagementError::InvalidInput("role permission not found".into()))?;
        self.engine
            .execute(vec![Statement::Query(delete(
                ROLE_PERMISSIONS,
                "id",
                relation.id,
            ))])
            .await
            .map_err(db_error)?;
        Ok(())
    }

    async fn list_role_permissions(
        &self,
        application_id: Id,
        role_id: Id,
    ) -> ManagementResult<Vec<Permission>> {
        let role = self
            .role(application_id, role_id)
            .await?
            .ok_or_else(|| ManagementError::InvalidInput("role not found".into()))?;
        let relations = self
            .rows::<RolePermissionRow>(select(ROLE_PERMISSIONS, &ROLE_PERMISSION_COLUMNS))
            .await?;
        let permissions =
            active_rows::<PermissionRow>(&self.engine, PERMISSIONS, &PERMISSION_COLUMNS).await?;
        let mut result = Vec::new();
        for relation in relations
            .into_iter()
            .filter(|row| row.role_id == role.grant_ref() && row.revoked_at.is_none())
        {
            self.ensure_clear(&[SecurityRow::new(SecurityTable::RolePermission, relation.id)])
                .await?;
            if let Some(permission) = permissions.iter().find(|row| {
                row.grant_ref() == relation.permission_id
                    && row.application_id == application_id
                    && row.revoked_at.is_none()
            }) {
                self.ensure_clear(&[SecurityRow::new(
                    SecurityTable::Permission,
                    permission.row_id,
                )])
                .await?;
                result.push(Permission::try_from(permission.clone())?);
            }
        }
        result.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(result)
    }

    fn select_resource(
        &self,
        device_id: Id,
        resource_id: Id,
        application_id: Id,
        owner_subject: &str,
    ) -> impl Future<Output = ManagementResult<()>> {
        let query = format!(
            "INSERT INTO {SYNC_SELECTIONS} (device_id, resource_id, application_id, owner_subject, selected_at) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (device_id, resource_id) DO UPDATE SET revoked_at = NULL, selected_at = $5, application_id = $3, owner_subject = $4"
        );
        let params = QueryParams::Positional(vec![
            Value::Uuid(device_id),
            Value::Uuid(resource_id),
            Value::Uuid(application_id),
            Value::Text(owner_subject.into()),
            Value::Integer(now().timestamp()),
        ]);
        async move {
            self.engine
                .execute_sql(&query, Some(&params))
                .await
                .map_err(db_error)?;
            Ok(())
        }
    }

    fn deselect_resource(
        &self,
        device_id: Id,
        resource_id: Id,
    ) -> impl Future<Output = ManagementResult<()>> {
        let now = now().timestamp();
        let query = format!(
            "UPDATE {SYNC_SELECTIONS} SET revoked_at = {now} WHERE device_id = $1 AND resource_id = $2 AND revoked_at IS NULL"
        );
        let params =
            QueryParams::Positional(vec![Value::Uuid(device_id), Value::Uuid(resource_id)]);
        async move {
            self.engine
                .execute_sql(&query, Some(&params))
                .await
                .map_err(db_error)?;
            Ok(())
        }
    }

    fn list_device_selections(
        &self,
        device_id: Id,
    ) -> impl Future<Output = ManagementResult<Vec<SyncSelection>>> {
        let query = format!(
            "SELECT {} FROM {SYNC_SELECTIONS} WHERE device_id = $1 AND revoked_at IS NULL",
            SYNC_SELECTION_COLUMNS.join(", ")
        );
        let params = QueryParams::Positional(vec![Value::Uuid(device_id)]);
        async move {
            let results = self
                .engine
                .execute_sql(&query, Some(&params))
                .await
                .map_err(db_error)?;
            let result = results
                .into_iter()
                .next()
                .ok_or_else(|| ManagementError::InvalidInput("missing query result".into()))?;
            let rows = result.rows_as::<SyncSelectionRow>().map_err(row_error)?;
            Ok(rows.into_iter().map(std::convert::Into::into).collect())
        }
    }

    fn list_resource_sync_targets(
        &self,
        resource_id: Id,
    ) -> impl Future<Output = ManagementResult<Vec<SyncSelection>>> {
        let query = format!(
            "SELECT {} FROM {SYNC_SELECTIONS} WHERE resource_id = $1 AND revoked_at IS NULL",
            SYNC_SELECTION_COLUMNS.join(", ")
        );
        let params = QueryParams::Positional(vec![Value::Uuid(resource_id)]);
        async move {
            let results = self
                .engine
                .execute_sql(&query, Some(&params))
                .await
                .map_err(db_error)?;
            let result = results
                .into_iter()
                .next()
                .ok_or_else(|| ManagementError::InvalidInput("missing query result".into()))?;
            let rows = result.rows_as::<SyncSelectionRow>().map_err(row_error)?;
            Ok(rows.into_iter().map(std::convert::Into::into).collect())
        }
    }

    fn has_selection(
        &self,
        device_id: Id,
        resource_id: Id,
    ) -> impl Future<Output = ManagementResult<bool>> {
        let query = format!(
            "SELECT device_id FROM {SYNC_SELECTIONS} WHERE device_id = $1 AND resource_id = $2 AND revoked_at IS NULL LIMIT 1"
        );
        let params =
            QueryParams::Positional(vec![Value::Uuid(device_id), Value::Uuid(resource_id)]);
        async move {
            let results = self
                .engine
                .execute_sql(&query, Some(&params))
                .await
                .map_err(db_error)?;
            let result = results
                .into_iter()
                .next()
                .ok_or_else(|| ManagementError::InvalidInput("missing query result".into()))?;
            Ok(!result.rows.is_empty())
        }
    }

    fn clear_device_selections(&self, device_id: Id) -> impl Future<Output = ManagementResult<()>> {
        let now = now().timestamp();
        let query = format!(
            "UPDATE {SYNC_SELECTIONS} SET revoked_at = {now} WHERE device_id = $1 AND revoked_at IS NULL"
        );
        let params = QueryParams::Positional(vec![Value::Uuid(device_id)]);
        async move {
            self.engine
                .execute_sql(&query, Some(&params))
                .await
                .map_err(db_error)?;
            Ok(())
        }
    }
}

#[derive(Clone, Debug)]
struct PermissionRow {
    row_id: Uuid,
    incarnation: i64,
    id: Uuid,
    application_id: Uuid,
    name: String,
    description: Option<String>,
    revoked_at: Option<i64>,
    created_at: i64,
    updated_at: i64,
}
impl PermissionRow {
    fn grant_ref(&self) -> Id {
        grant_ref(self.application_id, self.id, self.incarnation)
    }
}

impl FromRow for PermissionRow {
    fn from_row(row: &Row, columns: &[&str]) -> Result<Self, FromRowError> {
        Ok(Self {
            incarnation: ofdb_sql::decode(
                ofdb_sql::value(row, columns, "incarnation")?,
                "incarnation",
            )?,
            row_id: ofdb_sql::decode(ofdb_sql::value(row, columns, "row_id")?, "row_id")?,
            id: ofdb_sql::decode(ofdb_sql::value(row, columns, "id")?, "id")?,
            application_id: ofdb_sql::decode(
                ofdb_sql::value(row, columns, "application_id")?,
                "application_id",
            )?,
            name: ofdb_sql::decode(ofdb_sql::value(row, columns, "name")?, "name")?,
            description: ofdb_sql::decode(
                ofdb_sql::value(row, columns, "description")?,
                "description",
            )?,
            revoked_at: ofdb_sql::decode(
                ofdb_sql::value(row, columns, "revoked_at")?,
                "revoked_at",
            )?,
            created_at: ofdb_sql::decode(
                ofdb_sql::value(row, columns, "created_at")?,
                "created_at",
            )?,
            updated_at: ofdb_sql::decode(
                ofdb_sql::value(row, columns, "updated_at")?,
                "updated_at",
            )?,
        })
    }
}
impl TryFrom<PermissionRow> for Permission {
    type Error = ManagementError;
    fn try_from(row: PermissionRow) -> ManagementResult<Self> {
        Ok(Self {
            id: row.id,
            application_id: row.application_id,
            name: row.name,
            description: row.description,
            created_at: timestamp(row.created_at)?,
            updated_at: timestamp(row.updated_at)?,
        })
    }
}
#[derive(Debug)]
struct RoleRow {
    row_id: Uuid,
    incarnation: i64,
    id: Uuid,
    application_id: Uuid,
    revoked_at: Option<i64>,
}
impl RoleRow {
    fn grant_ref(&self) -> Id {
        grant_ref(self.application_id, self.id, self.incarnation)
    }
}

impl FromRow for RoleRow {
    fn from_row(row: &Row, columns: &[&str]) -> Result<Self, FromRowError> {
        Ok(Self {
            incarnation: ofdb_sql::decode(
                ofdb_sql::value(row, columns, "incarnation")?,
                "incarnation",
            )?,
            row_id: ofdb_sql::decode(ofdb_sql::value(row, columns, "row_id")?, "row_id")?,
            id: ofdb_sql::decode(ofdb_sql::value(row, columns, "id")?, "id")?,
            application_id: ofdb_sql::decode(
                ofdb_sql::value(row, columns, "application_id")?,
                "application_id",
            )?,
            revoked_at: ofdb_sql::decode(
                ofdb_sql::value(row, columns, "revoked_at")?,
                "revoked_at",
            )?,
        })
    }
}

#[derive(Debug)]
struct RolePermissionRow {
    id: Uuid,
    role_id: Uuid,
    permission_id: Uuid,
    revoked_at: Option<i64>,
}
impl FromRow for RolePermissionRow {
    fn from_row(row: &Row, columns: &[&str]) -> Result<Self, FromRowError> {
        Ok(Self {
            id: ofdb_sql::decode(ofdb_sql::value(row, columns, "id")?, "id")?,
            role_id: ofdb_sql::decode(ofdb_sql::value(row, columns, "role_id")?, "role_id")?,
            permission_id: ofdb_sql::decode(
                ofdb_sql::value(row, columns, "permission_id")?,
                "permission_id",
            )?,
            revoked_at: ofdb_sql::decode(
                ofdb_sql::value(row, columns, "revoked_at")?,
                "revoked_at",
            )?,
        })
    }
}
#[derive(Clone, Debug)]
struct SyncSelectionRow {
    device_id: Id,
    resource_id: Id,
    application_id: Id,
    owner_subject: String,
    selected_at: i64,
}

impl FromRow for SyncSelectionRow {
    fn from_row(row: &Row, columns: &[&str]) -> Result<Self, FromRowError> {
        Ok(Self {
            device_id: ofdb_sql::decode(ofdb_sql::value(row, columns, "device_id")?, "device_id")?,
            resource_id: ofdb_sql::decode(
                ofdb_sql::value(row, columns, "resource_id")?,
                "resource_id",
            )?,
            application_id: ofdb_sql::decode(
                ofdb_sql::value(row, columns, "application_id")?,
                "application_id",
            )?,
            owner_subject: ofdb_sql::decode(
                ofdb_sql::value(row, columns, "owner_subject")?,
                "owner_subject",
            )?,
            selected_at: ofdb_sql::decode(
                ofdb_sql::value(row, columns, "selected_at")?,
                "selected_at",
            )?,
        })
    }
}

impl From<SyncSelectionRow> for SyncSelection {
    fn from(row: SyncSelectionRow) -> Self {
        Self {
            device_id: row.device_id,
            resource_id: row.resource_id,
            application_id: row.application_id,
            owner_subject: row.owner_subject,
            selected_at: row.selected_at,
        }
    }
}

fn now() -> DateTime<Utc> {
    Utc::now()
        .with_nanosecond(0)
        .expect("zero nanoseconds is valid")
}
fn timestamp(value: i64) -> ManagementResult<DateTime<Utc>> {
    DateTime::from_timestamp(value, 0)
        .ok_or_else(|| ManagementError::InvalidInput("invalid permission timestamp".into()))
}
fn db_error(error: ofdb_sql::QueryError) -> ManagementError {
    ManagementError::InvalidInput(error.to_string())
}
fn row_error(error: FromRowError) -> ManagementError {
    ManagementError::InvalidInput(error.to_string())
}
fn column(table: &str, name: &str) -> QueryColumn {
    QueryColumn::new(table.into(), name.into())
}
fn select(table: &str, columns: &[&str]) -> Query {
    Query::Select(QuerySelect {
        from: QueryFrom {
            table: table.into(),
            joins: vec![],
        },
        projection: columns.iter().map(|name| column(table, name)).collect(),
        distinct: false,
        predicate: None,
        aggregates: vec![],
        text_concats: vec![],
        group_by: vec![],
        order_by: vec![],
        limit: None,
        offset: None,
        having: None,
    })
}
fn delete(table: &str, key: &str, id: Id) -> Query {
    Query::Delete(QueryDelete {
        from: QueryFrom {
            table: table.into(),
            joins: vec![],
        },
        predicate: Some(QueryExpr::Equals(
            Box::new(QueryExpr::Value(QueryExprValue::Column(column(table, key)))),
            Box::new(QueryExpr::Value(QueryExprValue::Value(Value::Uuid(id)))),
        )),
        returning: None,
    })
}
