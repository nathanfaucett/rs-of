use chrono::{DateTime, Timelike, Utc};
use db::ofdb::validators::SecurityRow;
use db::ofdb::validators::SecurityTable;
use db::ofdb::validators::allows_authorization;
use of_model::id::Id;
use of_model::idp::model::permission::Permission;
use of_model::idp::model::role::Role;
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

use super::insert::{insert_policy_record, returned_id};
use crate::RoleRepo;

const ROLES: &str = "roles";
const PERMISSIONS: &str = "permissions";
const ROLE_PERMISSIONS: &str = "role_permissions";
const USER_ROLES: &str = "application_user_roles";
const ROLE_COLUMNS: [&str; 8] = [
    "row_id",
    "id",
    "application_id",
    "name",
    "description",
    "revoked_at",
    "created_at",
    "updated_at",
];
const PERMISSION_COLUMNS: [&str; 8] = [
    "row_id",
    "id",
    "application_id",
    "name",
    "description",
    "revoked_at",
    "created_at",
    "updated_at",
];
const ROLE_PERMISSION_COLUMNS: [&str; 4] = ["id", "role_id", "permission_id", "revoked_at"];
const USER_ROLE_COLUMNS: [&str; 5] = ["id", "user_id", "application_id", "role_id", "revoked_at"];

pub struct DbRoleRepo {
    engine: ofdb_sql::Client,
}

impl DbRoleRepo {
    #[must_use]
    pub const fn new(engine: ofdb_sql::Client) -> Self {
        Self { engine }
    }

    async fn rows<T: FromRow>(&self, table: &str, columns: &[&str]) -> ManagementResult<Vec<T>> {
        let mut results = self
            .engine
            .execute(vec![Statement::Query(select(table, columns))])
            .await
            .map_err(db_error)?;
        results
            .pop()
            .ok_or_else(|| ManagementError::InvalidInput("missing query result".into()))?
            .rows_as::<T>()
            .map_err(row_error)
    }

    async fn roles(&self) -> ManagementResult<Vec<RoleRow>> {
        self.rows(ROLES, &ROLE_COLUMNS).await
    }

    async fn permissions(&self) -> ManagementResult<Vec<PermissionRow>> {
        self.rows(PERMISSIONS, &PERMISSION_COLUMNS).await
    }

    async fn role_permissions(&self) -> ManagementResult<Vec<RolePermissionRow>> {
        self.rows(ROLE_PERMISSIONS, &ROLE_PERMISSION_COLUMNS).await
    }

    async fn user_roles(&self) -> ManagementResult<Vec<UserRoleRow>> {
        self.rows(USER_ROLES, &USER_ROLE_COLUMNS).await
    }

    async fn insert_role(
        &self,
        domain_id: Option<Id>,
        application_id: Id,
        name: &str,
        description: Option<&str>,
    ) -> ManagementResult<Role> {
        let now = now();
        let (id, _) = insert_policy_record(
            &self.engine,
            ROLES,
            domain_id,
            application_id,
            name,
            description,
            now.timestamp(),
        )
        .await?;
        Ok(Role {
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
                "conflicted authorization state".into(),
            ))
        }
    }

    async fn ensure_role_clear(&self, role_id: Id) -> ManagementResult<()> {
        self.ensure_clear(&[SecurityRow::new(SecurityTable::Role, role_id)])
            .await
    }

    async fn role(&self, application_id: Id, role_id: Id) -> ManagementResult<Option<RoleRow>> {
        let role = self.roles().await?.into_iter().find(|role| {
            role.id == role_id && role.application_id == application_id && role.revoked_at.is_none()
        });
        if let Some(role) = &role {
            self.ensure_role_clear(role.row_id).await?;
        }
        Ok(role)
    }

    async fn assigned_roles(
        &self,
        user_id: Id,
        application_id: Option<Id>,
    ) -> ManagementResult<Vec<RoleRow>> {
        let assignments = self
            .user_roles()
            .await?
            .into_iter()
            .filter(|row| {
                row.user_id == user_id
                    && row.revoked_at.is_none()
                    && application_id.is_none_or(|id| row.application_id == id)
            })
            .collect::<Vec<_>>();
        self.ensure_clear(
            &assignments
                .iter()
                .map(|row| SecurityRow::new(SecurityTable::UserRole, row.id))
                .collect::<Vec<_>>(),
        )
        .await?;
        let mut roles = self
            .roles()
            .await?
            .into_iter()
            .filter(|role| {
                role.revoked_at.is_none()
                    && assignments.iter().any(|row| {
                        row.role_id == role.row_id && row.application_id == role.application_id
                    })
            })
            .collect::<Vec<_>>();
        self.ensure_clear(
            &roles
                .iter()
                .map(|row| SecurityRow::new(SecurityTable::Role, row.row_id))
                .collect::<Vec<_>>(),
        )
        .await?;
        roles.sort_by(|left, right| {
            (left.application_id, &left.name).cmp(&(right.application_id, &right.name))
        });
        Ok(roles)
    }

    async fn active_user_role(
        &self,
        application_id: Id,
        user_id: Id,
        role_id: Id,
    ) -> ManagementResult<Option<UserRoleRow>> {
        let user_role = self.user_roles().await?.into_iter().find(|user_role| {
            user_role.application_id == application_id
                && user_role.user_id == user_id
                && user_role.role_id == role_id
                && user_role.revoked_at.is_none()
        });
        if let Some(user_role) = &user_role {
            self.ensure_clear(&[SecurityRow::new(SecurityTable::UserRole, user_role.id)])
                .await?;
        }
        Ok(user_role)
    }
}

impl RoleRepo for DbRoleRepo {
    async fn list_roles(
        &self,
        application_id: Id,
        offset: u32,
        limit: u32,
    ) -> ManagementResult<Vec<Role>> {
        let mut roles = self
            .roles()
            .await?
            .into_iter()
            .filter(|role| role.application_id == application_id && role.revoked_at.is_none())
            .collect::<Vec<_>>();
        roles.sort_by_key(|role| role.name.clone());
        let roles = roles
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect::<Vec<_>>();
        for role in &roles {
            self.ensure_role_clear(role.row_id).await?;
        }
        roles.into_iter().map(Role::try_from).collect()
    }

    async fn create_role(
        &self,
        application_id: Id,
        name: &str,
        description: Option<&str>,
    ) -> ManagementResult<Role> {
        self.insert_role(None, application_id, name, description)
            .await
    }

    async fn create_role_with_id(
        &self,
        id: Id,
        application_id: Id,
        name: &str,
        description: Option<&str>,
    ) -> ManagementResult<Role> {
        if id.is_nil() {
            return Err(ManagementError::InvalidInput(
                "role ID must not be nil".into(),
            ));
        }
        self.insert_role(Some(id), application_id, name, description)
            .await
    }

    async fn find_role_by_id(
        &self,
        application_id: Id,
        role_id: Id,
    ) -> ManagementResult<Option<Role>> {
        self.role(application_id, role_id)
            .await?
            .map(Role::try_from)
            .transpose()
    }

    async fn delete_role_by_id(&self, application_id: Id, role_id: Id) -> ManagementResult<()> {
        let role = self
            .role(application_id, role_id)
            .await?
            .ok_or_else(|| ManagementError::InvalidInput("role not found".into()))?;
        let mut relationships = self
            .role_permissions()
            .await?
            .into_iter()
            .filter(|role_permission| {
                role_permission.role_id == role.row_id && role_permission.revoked_at.is_none()
            })
            .map(|role_permission| {
                SecurityRow::new(SecurityTable::RolePermission, role_permission.id)
            })
            .collect::<Vec<_>>();
        relationships.extend(
            self.user_roles()
                .await?
                .into_iter()
                .filter(|user_role| {
                    user_role.role_id == role.row_id && user_role.revoked_at.is_none()
                })
                .map(|user_role| SecurityRow::new(SecurityTable::UserRole, user_role.id)),
        );
        self.ensure_clear(&relationships).await?;
        self.engine
            .execute(vec![Statement::Query(Query::Delete(QueryDelete {
                from: from(ROLES),
                predicate: Some(equals(ROLES, "row_id", Value::Uuid(role.row_id))),
                returning: None,
            }))])
            .await
            .map_err(db_error)?;
        Ok(())
    }

    async fn add_role_to_user(
        &self,
        application_id: Id,
        user_id: Id,
        role_id: Id,
    ) -> ManagementResult<()> {
        let role = self
            .role(application_id, role_id)
            .await?
            .ok_or_else(|| ManagementError::InvalidInput("role not found".into()))?;
        if self
            .active_user_role(application_id, user_id, role.row_id)
            .await?
            .is_some()
        {
            return Ok(());
        }
        let params = QueryParams::Positional(vec![
            Value::Uuid(user_id),
            Value::Uuid(application_id),
            Value::Uuid(role.row_id),
            Value::Integer(now().timestamp()),
        ]);
        let results = self.engine.execute_sql(
            "INSERT INTO application_user_roles (user_id, application_id, role_id, created_at, updated_at) VALUES ($1, $2, $3, $4, $4) RETURNING id",
            Some(&params),
        ).await.map_err(db_error)?;
        self.ensure_clear(&[SecurityRow::new(
            SecurityTable::UserRole,
            returned_id(&results)?,
        )])
        .await
    }

    async fn remove_role_from_user(
        &self,
        application_id: Id,
        user_id: Id,
        role_id: Id,
    ) -> ManagementResult<()> {
        let role = self
            .role(application_id, role_id)
            .await?
            .ok_or_else(|| ManagementError::InvalidInput("role not found".into()))?;
        let user_role = self
            .active_user_role(application_id, user_id, role.row_id)
            .await?
            .ok_or_else(|| ManagementError::InvalidInput("user role not found".into()))?;
        self.engine
            .execute(vec![Statement::Query(Query::Delete(QueryDelete {
                from: from(USER_ROLES),
                predicate: Some(equals(USER_ROLES, "id", Value::Uuid(user_role.id))),
                returning: None,
            }))])
            .await
            .map_err(db_error)?;
        Ok(())
    }

    async fn list_user_roles(
        &self,
        application_id: Id,
        user_id: Id,
    ) -> ManagementResult<Vec<Role>> {
        self.assigned_roles(user_id, Some(application_id))
            .await?
            .into_iter()
            .map(Role::try_from)
            .collect()
    }

    async fn list_user_roles_across_applications(
        &self,
        user_id: Id,
    ) -> ManagementResult<Vec<Role>> {
        self.assigned_roles(user_id, None)
            .await?
            .into_iter()
            .map(Role::try_from)
            .collect()
    }

    async fn list_user_permissions(
        &self,
        application_id: Id,
        user_id: Id,
    ) -> ManagementResult<Vec<Permission>> {
        let roles = self.assigned_roles(user_id, Some(application_id)).await?;
        let role_ids = roles.iter().map(|role| role.row_id).collect::<Vec<_>>();
        let role_permissions = self
            .role_permissions()
            .await?
            .into_iter()
            .filter(|row| role_ids.contains(&row.role_id) && row.revoked_at.is_none())
            .collect::<Vec<_>>();
        self.ensure_clear(
            &role_permissions
                .iter()
                .map(|row| SecurityRow::new(SecurityTable::RolePermission, row.id))
                .collect::<Vec<_>>(),
        )
        .await?;
        let permission_ids = role_permissions
            .iter()
            .map(|row| row.permission_id)
            .collect::<Vec<_>>();
        let mut permissions = self
            .permissions()
            .await?
            .into_iter()
            .filter(|permission| {
                permission.application_id == application_id
                    && permission_ids.contains(&permission.row_id)
                    && permission.revoked_at.is_none()
            })
            .collect::<Vec<_>>();
        permissions.sort_by_key(|permission| permission.name.clone());
        self.ensure_clear(
            &permissions
                .iter()
                .map(|permission| SecurityRow::new(SecurityTable::Permission, permission.row_id))
                .collect::<Vec<_>>(),
        )
        .await?;
        permissions.into_iter().map(Permission::try_from).collect()
    }

    async fn has_user_application_permission(
        &self,
        user_id: Id,
        application_id: Id,
        permission_name: &str,
    ) -> ManagementResult<bool> {
        let permissions = self.list_user_permissions(application_id, user_id).await?;
        Ok(permissions
            .iter()
            .any(|permission| permission_matches(&permission.name, permission_name)))
    }
}

#[derive(Debug)]
struct RoleRow {
    row_id: Uuid,
    id: Uuid,
    application_id: Uuid,
    name: String,
    description: Option<String>,
    revoked_at: Option<i64>,
    created_at: i64,
    updated_at: i64,
}

impl FromRow for RoleRow {
    fn from_row(row: &Row, columns: &[&str]) -> Result<Self, FromRowError> {
        Ok(Self {
            row_id: decode(row, columns, "row_id")?,
            id: decode(row, columns, "id")?,
            application_id: decode(row, columns, "application_id")?,
            name: decode(row, columns, "name")?,
            description: decode(row, columns, "description")?,
            revoked_at: decode(row, columns, "revoked_at")?,
            created_at: decode(row, columns, "created_at")?,
            updated_at: decode(row, columns, "updated_at")?,
        })
    }
}

impl TryFrom<RoleRow> for Role {
    type Error = ManagementError;

    fn try_from(row: RoleRow) -> ManagementResult<Self> {
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
struct PermissionRow {
    row_id: Uuid,
    id: Uuid,
    application_id: Uuid,
    name: String,
    description: Option<String>,
    revoked_at: Option<i64>,
    created_at: i64,
    updated_at: i64,
}

impl FromRow for PermissionRow {
    fn from_row(row: &Row, columns: &[&str]) -> Result<Self, FromRowError> {
        Ok(Self {
            row_id: decode(row, columns, "row_id")?,
            id: decode(row, columns, "id")?,
            application_id: decode(row, columns, "application_id")?,
            name: decode(row, columns, "name")?,
            description: decode(row, columns, "description")?,
            revoked_at: decode(row, columns, "revoked_at")?,
            created_at: decode(row, columns, "created_at")?,
            updated_at: decode(row, columns, "updated_at")?,
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
struct RolePermissionRow {
    id: Uuid,
    role_id: Uuid,
    permission_id: Uuid,
    revoked_at: Option<i64>,
}

impl FromRow for RolePermissionRow {
    fn from_row(row: &Row, columns: &[&str]) -> Result<Self, FromRowError> {
        Ok(Self {
            id: decode(row, columns, "id")?,
            role_id: decode(row, columns, "role_id")?,
            permission_id: decode(row, columns, "permission_id")?,
            revoked_at: decode(row, columns, "revoked_at")?,
        })
    }
}

#[derive(Debug)]
struct UserRoleRow {
    id: Uuid,
    user_id: Uuid,
    application_id: Uuid,
    role_id: Uuid,
    revoked_at: Option<i64>,
}

impl FromRow for UserRoleRow {
    fn from_row(row: &Row, columns: &[&str]) -> Result<Self, FromRowError> {
        Ok(Self {
            id: decode(row, columns, "id")?,
            user_id: decode(row, columns, "user_id")?,
            application_id: decode(row, columns, "application_id")?,
            role_id: decode(row, columns, "role_id")?,
            revoked_at: decode(row, columns, "revoked_at")?,
        })
    }
}

fn decode<T: ofdb_sql::FromValue>(
    row: &Row,
    columns: &[&str],
    name: &str,
) -> Result<T, FromRowError> {
    ofdb_sql::decode(ofdb_sql::value(row, columns, name)?, name)
}

fn timestamp(value: i64) -> ManagementResult<DateTime<Utc>> {
    DateTime::from_timestamp(value, 0)
        .ok_or_else(|| ManagementError::InvalidInput("invalid timestamp".into()))
}

fn now() -> DateTime<Utc> {
    Utc::now()
        .with_nanosecond(0)
        .expect("zero nanoseconds is valid")
}

fn db_error(error: ofdb_sql::QueryError) -> ManagementError {
    ManagementError::InvalidInput(error.to_string())
}

fn row_error(error: FromRowError) -> ManagementError {
    ManagementError::InvalidInput(error.to_string())
}

fn from(table: &str) -> QueryFrom {
    QueryFrom {
        table: table.into(),
        joins: vec![],
    }
}

fn column(table: &str, name: &str) -> QueryColumn {
    QueryColumn::new(table.into(), name.into())
}

fn equals(table: &str, name: &str, value: Value) -> QueryExpr {
    QueryExpr::Equals(
        Box::new(QueryExpr::Value(QueryExprValue::Column(column(
            table, name,
        )))),
        Box::new(QueryExpr::Value(QueryExprValue::Value(value))),
    )
}

fn select(table: &str, columns: &[&str]) -> Query {
    Query::Select(QuerySelect {
        from: from(table),
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

fn permission_matches(granted: &str, required: &str) -> bool {
    if granted == "*" || granted == required {
        return true;
    }
    let Some(prefix) = granted.strip_suffix('*') else {
        return false;
    };
    if required.starts_with(prefix) {
        return true;
    }
    prefix
        .strip_suffix(':')
        .is_some_and(|prefix| required.starts_with(&format!("{prefix}.")))
        || prefix
            .strip_suffix('.')
            .is_some_and(|prefix| required.starts_with(&format!("{prefix}:")))
}
