use alloc::vec::Vec;

use of_model::idp::contract::permission_evaluation::{INSTALLATION_POLICY_ID, IdentityAction};

use of_model::idp::contract::permission_evaluation::PermissionEvaluationRequest;

use of_model::idp::contract::permission_evaluation::PermissionSubject;

use of_model::id::Id;

use of_model::idp::model::permission::Permission;

use of_model::idp::model::role::Role;

use of_model::management_error::ManagementError;

use crate::PermissionRepo;

use crate::RoleRepo;

pub use of_model::idp::contract::permission_evaluation::MANAGEMENT_APPLICATION_URI;

pub const ROLES_READ_PERMISSION: &str = "roles.read";
pub const ROLES_WRITE_PERMISSION: &str = "roles.write";
pub const PERMISSIONS_READ_PERMISSION: &str = "permissions.read";
pub const PERMISSIONS_WRITE_PERMISSION: &str = "permissions.write";
pub const APPLICATION_POLICY_ADMINISTRATION: [&str; 4] = [
    ROLES_READ_PERMISSION,
    ROLES_WRITE_PERMISSION,
    PERMISSIONS_READ_PERMISSION,
    PERMISSIONS_WRITE_PERMISSION,
];

pub const INITIAL_ADMINISTRATOR_ACTIONS: [IdentityAction; 24] = [
    IdentityAction::InfrastructureClientsRead,
    IdentityAction::InfrastructureClientsCreate,
    IdentityAction::InfrastructureClientsUpdate,
    IdentityAction::InfrastructureClientsDelete,
    IdentityAction::ApplicationsRead,
    IdentityAction::ApplicationsCreate,
    IdentityAction::ApplicationsUpdate,
    IdentityAction::ApplicationsDelete,
    IdentityAction::UsersRead,
    IdentityAction::UsersUpdate,
    IdentityAction::UsersDelete,
    IdentityAction::UsersResetPassword,
    IdentityAction::ConsentsRead,
    IdentityAction::ConsentsRevoke,
    IdentityAction::KeysRead,
    IdentityAction::KeysRotate,
    IdentityAction::KeysRevoke,
    IdentityAction::DevicesRead,
    IdentityAction::DevicesEnroll,
    IdentityAction::DevicePairingRead,
    IdentityAction::DevicePairingUpdate,
    IdentityAction::IdpSecuritySync,
    IdentityAction::ManagementSecuritySync,
    IdentityAction::ClientsRegister,
];

#[derive(Debug)]
pub struct ManagementService<P, R> {
    permission_repo: P,
    role_repo: R,
}

#[allow(
    clippy::future_not_send,
    reason = "PermissionRepo and RoleRepo do not require Send futures"
)]
impl<P, R> ManagementService<P, R>
where
    P: PermissionRepo,
    R: RoleRepo,
{
    pub const fn new(permission_repo: P, role_repo: R) -> Self {
        Self {
            permission_repo,
            role_repo,
        }
    }

    /// # Errors
    ///
    /// Returns [`ManagementError`] when the role repository fails.
    pub async fn has_user_application_permission(
        &self,
        user_id: Id,
        application_id: Id,
        permission_name: &str,
    ) -> Result<bool, ManagementError> {
        if application_id.is_nil() || user_id.is_nil() {
            return Ok(false);
        }
        self.role_repo
            .has_user_application_permission(user_id, application_id, permission_name)
            .await
    }

    /// # Errors
    ///
    /// Returns [`ManagementError`] when the role repository fails.
    pub async fn has_installation_policy_permission(
        &self,
        user_id: Id,
        permission: &str,
    ) -> Result<bool, ManagementError> {
        if user_id.is_nil() || !APPLICATION_POLICY_ADMINISTRATION.contains(&permission) {
            return Ok(false);
        }
        let grants = self
            .role_repo
            .list_user_permissions(INSTALLATION_POLICY_ID, user_id)
            .await?;
        Ok(grants.iter().any(|grant| grant.name == permission))
    }

    /// # Errors
    ///
    /// Returns [`ManagementError`] when the role repository fails.
    pub async fn evaluate_permission(
        &self,
        request: &PermissionEvaluationRequest,
    ) -> Result<bool, ManagementError> {
        let Some(namespace) = request.policy_namespace() else {
            return Ok(false);
        };
        let PermissionSubject::User { id } = request.subject;
        if id.is_nil() {
            return Ok(false);
        }
        self.role_repo
            .has_user_application_permission(id, namespace, request.action.permission())
            .await
    }

    /// # Errors
    ///
    /// Returns [`ManagementError`] when the role repository fails.
    pub async fn list_roles(
        &self,
        application_id: Id,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<Role>, ManagementError> {
        self.role_repo
            .list_roles(application_id, offset, limit)
            .await
    }

    /// # Errors
    ///
    /// Returns [`ManagementError`] when the role repository fails.
    pub async fn create_role(
        &self,
        application_id: Id,
        name: &str,
        description: Option<&str>,
    ) -> Result<Role, ManagementError> {
        self.role_repo
            .create_role(application_id, name, description)
            .await
    }

    /// # Errors
    ///
    /// Returns [`ManagementError`] when the role repository fails.
    pub async fn find_role_by_id(
        &self,
        application_id: Id,
        role_id: Id,
    ) -> Result<Option<Role>, ManagementError> {
        self.role_repo
            .find_role_by_id(application_id, role_id)
            .await
    }

    /// # Errors
    ///
    /// Returns [`ManagementError`] when the role repository fails.
    pub async fn delete_role_by_id(
        &self,
        application_id: Id,
        role_id: Id,
    ) -> Result<(), ManagementError> {
        self.role_repo
            .delete_role_by_id(application_id, role_id)
            .await
    }

    /// # Errors
    ///
    /// Returns [`ManagementError`] when the role repository fails.
    pub async fn add_role_to_user(
        &self,
        application_id: Id,
        user_id: Id,
        role_id: Id,
    ) -> Result<(), ManagementError> {
        self.role_repo
            .add_role_to_user(application_id, user_id, role_id)
            .await
    }

    /// # Errors
    ///
    /// Returns [`ManagementError`] when the role repository fails.
    pub async fn remove_role_from_user(
        &self,
        application_id: Id,
        user_id: Id,
        role_id: Id,
    ) -> Result<(), ManagementError> {
        self.role_repo
            .remove_role_from_user(application_id, user_id, role_id)
            .await
    }

    /// # Errors
    ///
    /// Returns [`ManagementError`] when the role repository fails.
    pub async fn list_user_roles(
        &self,
        application_id: Id,
        user_id: Id,
    ) -> Result<Vec<Role>, ManagementError> {
        self.role_repo
            .list_user_roles(application_id, user_id)
            .await
    }

    /// # Errors
    ///
    /// Returns [`ManagementError`] when the role repository fails.
    pub async fn list_user_roles_across_applications(
        &self,
        user_id: Id,
    ) -> Result<Vec<Role>, ManagementError> {
        self.role_repo
            .list_user_roles_across_applications(user_id)
            .await
    }

    /// # Errors
    ///
    /// Returns [`ManagementError`] when the permission repository fails.
    pub async fn list_permissions(
        &self,
        application_id: Id,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<Permission>, ManagementError> {
        self.permission_repo
            .list_permissions(application_id, offset, limit)
            .await
    }

    /// # Errors
    ///
    /// Returns [`ManagementError`] when the permission repository fails.
    pub async fn create_permission(
        &self,
        application_id: Id,
        name: &str,
        description: Option<&str>,
    ) -> Result<Permission, ManagementError> {
        self.permission_repo
            .create_permission(application_id, name, description)
            .await
    }

    /// # Errors
    ///
    /// Returns [`ManagementError`] when the permission repository fails.
    pub async fn find_permission_by_id(
        &self,
        application_id: Id,
        permission_id: Id,
    ) -> Result<Option<Permission>, ManagementError> {
        self.permission_repo
            .find_permission_by_id(application_id, permission_id)
            .await
    }

    /// # Errors
    ///
    /// Returns [`ManagementError`] when the permission repository fails.
    pub async fn delete_permission_by_id(
        &self,
        application_id: Id,
        permission_id: Id,
    ) -> Result<(), ManagementError> {
        self.permission_repo
            .delete_permission_by_id(application_id, permission_id)
            .await
    }

    /// # Errors
    ///
    /// Returns [`ManagementError`] when the permission repository fails.
    pub async fn list_role_permissions(
        &self,
        application_id: Id,
        role_id: Id,
    ) -> Result<Vec<Permission>, ManagementError> {
        self.permission_repo
            .list_role_permissions(application_id, role_id)
            .await
    }

    /// # Errors
    ///
    /// Returns [`ManagementError`] when the permission repository fails.
    pub async fn add_permission_to_role(
        &self,
        application_id: Id,
        role_id: Id,
        permission_id: Id,
    ) -> Result<(), ManagementError> {
        self.permission_repo
            .add_permission_to_role(application_id, role_id, permission_id)
            .await
    }

    /// # Errors
    ///
    /// Returns [`ManagementError`] when the permission repository fails.
    pub async fn remove_permission_from_role(
        &self,
        application_id: Id,
        role_id: Id,
        permission_id: Id,
    ) -> Result<(), ManagementError> {
        self.permission_repo
            .remove_permission_from_role(application_id, role_id, permission_id)
            .await
    }
}

#[cfg(all(test, feature = "ofdb"))]
mod tests {
    use of_model::id::Id;

    use crate::service::ManagementService;

    use crate::ofdb::DbPermissionRepo;

    use crate::ofdb::DbRoleRepo;

    use db::ofdb::up;

    #[tokio::test]
    async fn initial_administrator_has_explicit_client_registration_permission() {
        use of_model::idp::contract::permission_evaluation::{
            IdentityAction, IdentityResource, PermissionEvaluationRequest, PermissionSubject,
            PermissionTarget,
        };
        let engine = ofdb_sql::Database::in_memory();
        up(&engine.client())
            .await
            .expect("initialize policy schema");
        let service = ManagementService::new(
            DbPermissionRepo::new(engine.client()),
            DbRoleRepo::new(engine.client()),
        );
        let administrator = Id::now_v7();
        let actions = &crate::INITIAL_ADMINISTRATOR_ACTIONS;
        crate::provision_initial_administrator(&engine.client(), administrator, actions)
            .await
            .expect("provision explicit administrator grants");
        let mut request = PermissionEvaluationRequest {
            request_id: Id::now_v7(),
            subject: PermissionSubject::User { id: administrator },
            action: IdentityAction::ClientsRegister,
            target: PermissionTarget::Installation {
                resource: IdentityResource::Client { client_id: None },
            },
        };
        assert!(
            service
                .evaluate_permission(&request)
                .await
                .expect("administrator may register")
        );
        request.subject = PermissionSubject::User { id: Id::now_v7() };
        assert!(
            !service
                .evaluate_permission(&request)
                .await
                .expect("not granted to all Users")
        );
        request.subject = PermissionSubject::User { id: administrator };
        request.target = PermissionTarget::Application {
            application_id: Id::now_v7(),
            resource: IdentityResource::Client { client_id: None },
        };
        assert!(
            !service
                .evaluate_permission(&request)
                .await
                .expect("registration is not Application-scoped")
        );
        let scope = of_model::idp::contract::permission_evaluation::INSTALLATION_POLICY_ID;
        let permission = service
            .list_permissions(scope, 0, 100)
            .await
            .expect("list setup permissions")
            .into_iter()
            .find(|permission| permission.name == IdentityAction::ClientsRegister.permission())
            .expect("registration permission");
        service
            .delete_permission_by_id(scope, permission.id)
            .await
            .expect("delete bootstrap permission");
        crate::provision_initial_administrator(&engine.client(), administrator, actions)
            .await
            .expect("exact setup retry");
        request.target = PermissionTarget::Installation {
            resource: IdentityResource::Client { client_id: None },
        };
        assert!(
            !service
                .evaluate_permission(&request)
                .await
                .expect("setup retry cannot restore a deleted grant")
        );
    }

    #[tokio::test]
    async fn stale_peer_cannot_restore_deleted_policy_or_grants() {
        for delete_role in [true, false] {
            let left = ofdb_sql::Database::in_memory();
            let right = ofdb_sql::Database::in_memory();
            up(&left.client()).await.expect("owner schema");
            let a = ManagementService::new(
                DbPermissionRepo::new(left.client()),
                DbRoleRepo::new(left.client()),
            );
            let b = ManagementService::new(
                DbPermissionRepo::new(right.client()),
                DbRoleRepo::new(right.client()),
            );
            let (app, user, offline_user) = (Id::now_v7(), Id::now_v7(), Id::now_v7());
            let role = a
                .create_role(app, "operator", None)
                .await
                .expect("create role")
                .id;
            let permission = a
                .create_permission(app, "operate", None)
                .await
                .expect("create permission")
                .id;
            a.add_role_to_user(app, user, role)
                .await
                .expect("assign role");
            a.add_permission_to_role(app, role, permission)
                .await
                .expect("grant permission");
            reconcile(&left, &right).await;
            assert!(
                b.has_user_application_permission(user, app, "operate")
                    .await
                    .expect("replicated grant")
            );

            if delete_role {
                a.delete_role_by_id(app, role).await.expect("delete role");
            } else {
                a.delete_permission_by_id(app, permission)
                    .await
                    .expect("delete permission");
            }
            b.add_role_to_user(app, offline_user, role)
                .await
                .expect("offline assignment");
            assert!(
                b.has_user_application_permission(offline_user, app, "operate")
                    .await
                    .expect("offline grant")
            );
            reconcile(&right, &left).await;
            for owner in [&a, &b] {
                if delete_role {
                    assert!(
                        owner
                            .find_role_by_id(app, role)
                            .await
                            .expect("read deleted role")
                            .is_none()
                    );
                } else {
                    assert!(
                        owner
                            .find_permission_by_id(app, permission)
                            .await
                            .expect("read deleted permission")
                            .is_none()
                    );
                }
                for subject in [user, offline_user] {
                    assert!(
                        !owner
                            .has_user_application_permission(subject, app, "operate")
                            .await
                            .expect("stale grants deny")
                    );
                }
            }

            let (new_role, new_permission) = if delete_role {
                let replacement = a
                    .create_role(app, "operator", None)
                    .await
                    .expect("replace role");
                assert_ne!(replacement.id, role);
                (replacement.id, permission)
            } else {
                let replacement = a
                    .create_permission(app, "operate", None)
                    .await
                    .expect("replace permission");
                assert_ne!(replacement.id, permission);
                (role, replacement.id)
            };
            reconcile(&left, &right).await;
            for owner in [&a, &b] {
                assert!(
                    owner
                        .list_role_permissions(app, new_role)
                        .await
                        .expect("list replacement grants")
                        .is_empty()
                );
                assert!(
                    !owner
                        .has_user_application_permission(user, app, "operate")
                        .await
                        .expect("replacement has no old grants")
                );
            }
            a.add_permission_to_role(app, new_role, new_permission)
                .await
                .expect("explicit permission grant");
            a.add_role_to_user(app, user, new_role)
                .await
                .expect("explicit assignment");
            reconcile(&left, &right).await;
            assert!(
                b.has_user_application_permission(user, app, "operate")
                    .await
                    .expect("replacement grants replicate")
            );
            let granted = b
                .list_role_permissions(app, new_role)
                .await
                .expect("list replicated grant");
            assert_eq!(granted.len(), 1);
            assert_eq!(granted[0].id, new_permission);
            if delete_role {
                assert!(
                    !b.has_user_application_permission(offline_user, app, "operate")
                        .await
                        .expect("offline assignment is still inert")
                );
                a.remove_role_from_user(app, user, new_role)
                    .await
                    .expect("remove the replacement assignment");
            } else {
                a.remove_permission_from_role(app, new_role, new_permission)
                    .await
                    .expect("remove the replacement grant");
            }
            assert!(
                !a.has_user_application_permission(user, app, "operate")
                    .await
                    .expect("removing the explicit grant denies access")
            );
        }
    }

    async fn reconcile(a: &ofdb_sql::Database, b: &ofdb_sql::Database) {
        for (source, target) in [(a, b), (b, a)] {
            target
                .import_history(
                    source
                        .export_history()
                        .await
                        .expect("export policy history"),
                )
                .await
                .expect("reconcile policy history");
        }
    }

    #[tokio::test]
    async fn ordinary_creates_return_persisted_uuidv7_ids() {
        let engine = ofdb_sql::Database::in_memory();
        up(&engine.client())
            .await
            .expect("initialize policy schema");
        let service = ManagementService::new(
            DbPermissionRepo::new(engine.client()),
            DbRoleRepo::new(engine.client()),
        );
        let application = Id::now_v7();
        let role = service
            .create_role(application, "operator", None)
            .await
            .expect("create role");
        let permission = service
            .create_permission(application, "operate", None)
            .await
            .expect("create permission");
        assert_eq!(role.id.get_version_num(), 7);
        assert_eq!(permission.id.get_version_num(), 7);
        assert_ne!(role.id, permission.id);
        assert_eq!(
            service
                .find_role_by_id(application, role.id)
                .await
                .expect("find role"),
            Some(role)
        );
        assert_eq!(
            service
                .find_permission_by_id(application, permission.id)
                .await
                .expect("find permission"),
            Some(permission)
        );
    }

    #[tokio::test]
    async fn cross_application_references_do_not_grant_policy() {
        let engine = ofdb_sql::Database::in_memory();
        up(&engine.client())
            .await
            .expect("initialize policy schema");
        let service = ManagementService::new(
            DbPermissionRepo::new(engine.client()),
            DbRoleRepo::new(engine.client()),
        );
        let role_application = Id::now_v7();
        let permission_application = Id::now_v7();
        let user = Id::now_v7();
        let role = service
            .create_role(role_application, "operator", None)
            .await
            .expect("create role");
        let permission = service
            .create_permission(permission_application, "operate", None)
            .await
            .expect("create permission");

        // Synchronized references to another application's policy must stay inert.
        for (sql, params) in [
            (
                "INSERT INTO application_user_roles (user_id, application_id, role_id, created_at, updated_at) VALUES ($1, $2, $3, $4, $4)",
                ofdb_sql::QueryParams::Positional(vec![
                    ofdb_sql::Value::Uuid(user),
                    ofdb_sql::Value::Uuid(permission_application),
                    ofdb_sql::Value::Uuid(role.id),
                    ofdb_sql::Value::Integer(0),
                ]),
            ),
            (
                "INSERT INTO role_permissions (role_id, permission_id, created_at, updated_at) VALUES ($1, $2, $3, $3)",
                ofdb_sql::QueryParams::Positional(vec![
                    ofdb_sql::Value::Uuid(role.id),
                    ofdb_sql::Value::Uuid(permission.id),
                    ofdb_sql::Value::Integer(0),
                ]),
            ),
        ] {
            engine
                .client()
                .execute_sql(sql, Some(&params))
                .await
                .expect("insert cross-application reference");
        }

        assert!(
            service
                .list_user_roles(permission_application, user)
                .await
                .expect("list scoped roles")
                .is_empty()
        );
        assert!(
            service
                .list_user_roles_across_applications(user)
                .await
                .expect("list roles across applications")
                .is_empty()
        );
        assert!(
            !service
                .has_user_application_permission(user, permission_application, "operate")
                .await
                .expect("cross-application policy must not authorize")
        );
    }
}
