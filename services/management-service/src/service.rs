use of_model::idp::contract::permission_evaluation::INSTALLATION_POLICY_ID;

use of_model::idp::contract::permission_evaluation::IdentityAction;

use of_model::idp::contract::permission_evaluation::PermissionEvaluationRequest;

use of_model::idp::contract::permission_evaluation::PermissionSubject;

use of_model::id::Id;

use of_model::idp::model::permission::Permission;

use of_model::idp::model::role::Role;

use of_model::management_error::ManagementError;

use crate::PermissionRepo;

use crate::RoleRepo;

pub const MANAGEMENT_APPLICATION_URI: &str = "idp-management";

pub const INITIAL_ADMINISTRATOR_ACTIONS: [IdentityAction; 26] = [
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
    IdentityAction::DevicePairingRead,
    IdentityAction::DevicePairingUpdate,
    IdentityAction::ReplicaSignersEnroll,
    IdentityAction::ReplicaSignersRotate,
    IdentityAction::ReplicaSignersRevoke,
    IdentityAction::IdpSecuritySync,
    IdentityAction::ManagementSecuritySync,
    IdentityAction::ClientsRegister,
];

pub struct ManagementService<P, R> {
    permission_repo: P,
    role_repo: R,
}

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

    pub async fn has_user_application_permission(
        &self,
        user_id: of_model::id::Id,
        application_id: of_model::id::Id,
        permission_name: &str,
    ) -> Result<bool, ManagementError> {
        if application_id.is_nil() || user_id.is_nil() {
            return Ok(false);
        }
        self.role_repo
            .has_user_application_permission(user_id, application_id, permission_name)
            .await
    }

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

    pub async fn list_roles(
        &self,
        application_id: of_model::id::Id,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<Role>, ManagementError> {
        self.role_repo
            .list_roles(application_id, offset, limit)
            .await
    }

    pub async fn create_role(
        &self,
        application_id: of_model::id::Id,
        name: &str,
        description: Option<&str>,
    ) -> Result<Role, ManagementError> {
        self.role_repo
            .create_role(application_id, name, description)
            .await
    }

    pub async fn provision_initial_administrator(
        &self,
        user_id: Id,
        role_id: Id,
        permissions: &[(Id, IdentityAction)],
    ) -> Result<Role, ManagementError> {
        let mut action_names = alloc::vec::Vec::new();
        let mut permission_ids = alloc::vec::Vec::new();
        for (permission_id, action) in permissions {
            let name = action.permission();
            if permission_id.is_nil()
                || action.application_scoped()
                || name == "*"
                || action_names.contains(&name)
                || permission_ids.contains(permission_id)
            {
                return Err(ManagementError::InvalidInput(
                    "administrator permissions must be unique explicit installation actions".into(),
                ));
            }
            action_names.push(name);
            permission_ids.push(*permission_id);
        }
        if user_id.is_nil()
            || role_id.is_nil()
            || !action_names.contains(&IdentityAction::DevicePairingRead.permission())
            || !action_names.contains(&IdentityAction::DevicePairingUpdate.permission())
        {
            return Err(ManagementError::InvalidInput(
                "administrator requires valid IDs and both device-pairing permissions".into(),
            ));
        }

        let role = self
            .ensure_role(
                role_id,
                INSTALLATION_POLICY_ID,
                "installation-administrator",
                None,
            )
            .await?;
        let existing_permissions = self
            .list_role_permissions(INSTALLATION_POLICY_ID, role.id)
            .await?;
        if existing_permissions.iter().any(|existing| {
            !permissions.iter().any(|(permission_id, action)| {
                *permission_id == existing.id && action.permission() == existing.name
            })
        }) {
            return Err(ManagementError::InvalidInput(
                "administrator role has permissions outside the explicit grant list".into(),
            ));
        }
        for (permission_id, action) in permissions {
            let permission = self
                .ensure_permission(
                    *permission_id,
                    INSTALLATION_POLICY_ID,
                    action.permission(),
                    None,
                )
                .await?;
            self.add_permission_to_role(INSTALLATION_POLICY_ID, role.id, permission.id)
                .await?;
        }
        self.add_role_to_user(INSTALLATION_POLICY_ID, user_id, role.id)
            .await?;
        Ok(role)
    }

    pub async fn ensure_role(
        &self,
        id: of_model::id::Id,
        application_id: of_model::id::Id,
        name: &str,
        description: Option<&str>,
    ) -> Result<Role, ManagementError> {
        if id.is_nil() || name.is_empty() {
            return Err(ManagementError::InvalidInput(
                "role ID and name must not be empty".into(),
            ));
        }
        if let Some(role) = self.role_repo.find_role_by_id(application_id, id).await? {
            return if role.name == name && role.description.as_deref() == description {
                Ok(role)
            } else {
                Err(ManagementError::InvalidInput(
                    "stable role ID already has different data".into(),
                ))
            };
        }
        match self
            .role_repo
            .create_role_with_id(id, application_id, name, description)
            .await
        {
            Ok(role) => Ok(role),
            Err(error) => match self.role_repo.find_role_by_id(application_id, id).await? {
                Some(role) if role.name == name && role.description.as_deref() == description => {
                    Ok(role)
                }
                _ => Err(error),
            },
        }
    }

    pub async fn find_role_by_id(
        &self,
        application_id: of_model::id::Id,
        role_id: of_model::id::Id,
    ) -> Result<Option<Role>, ManagementError> {
        self.role_repo
            .find_role_by_id(application_id, role_id)
            .await
    }

    pub async fn delete_role_by_id(
        &self,
        application_id: of_model::id::Id,
        role_id: of_model::id::Id,
    ) -> Result<(), ManagementError> {
        self.role_repo
            .delete_role_by_id(application_id, role_id)
            .await
    }

    pub async fn add_role_to_user(
        &self,
        application_id: of_model::id::Id,
        user_id: of_model::id::Id,
        role_id: of_model::id::Id,
    ) -> Result<(), ManagementError> {
        self.role_repo
            .add_role_to_user(application_id, user_id, role_id)
            .await
    }

    pub async fn remove_role_from_user(
        &self,
        application_id: of_model::id::Id,
        user_id: of_model::id::Id,
        role_id: of_model::id::Id,
    ) -> Result<(), ManagementError> {
        self.role_repo
            .remove_role_from_user(application_id, user_id, role_id)
            .await
    }

    pub async fn list_user_roles(
        &self,
        application_id: of_model::id::Id,
        user_id: of_model::id::Id,
    ) -> Result<Vec<Role>, ManagementError> {
        self.role_repo
            .list_user_roles(application_id, user_id)
            .await
    }

    pub async fn list_user_roles_across_applications(
        &self,
        user_id: of_model::id::Id,
    ) -> Result<Vec<Role>, ManagementError> {
        self.role_repo
            .list_user_roles_across_applications(user_id)
            .await
    }

    pub async fn list_permissions(
        &self,
        application_id: of_model::id::Id,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<Permission>, ManagementError> {
        self.permission_repo
            .list_permissions(application_id, offset, limit)
            .await
    }

    pub async fn create_permission(
        &self,
        application_id: of_model::id::Id,
        name: &str,
        description: Option<&str>,
    ) -> Result<Permission, ManagementError> {
        self.permission_repo
            .create_permission(application_id, name, description)
            .await
    }

    pub async fn ensure_permission(
        &self,
        id: of_model::id::Id,
        application_id: of_model::id::Id,
        name: &str,
        description: Option<&str>,
    ) -> Result<Permission, ManagementError> {
        if id.is_nil() || name.is_empty() {
            return Err(ManagementError::InvalidInput(
                "permission ID and name must not be empty".into(),
            ));
        }
        if let Some(permission) = self
            .permission_repo
            .find_permission_by_id(application_id, id)
            .await?
        {
            return if permission.name == name && permission.description.as_deref() == description {
                Ok(permission)
            } else {
                Err(ManagementError::InvalidInput(
                    "stable permission ID already has different data".into(),
                ))
            };
        }
        match self
            .permission_repo
            .create_permission_with_id(id, application_id, name, description)
            .await
        {
            Ok(permission) => Ok(permission),
            Err(error) => match self
                .permission_repo
                .find_permission_by_id(application_id, id)
                .await?
            {
                Some(permission)
                    if permission.name == name
                        && permission.description.as_deref() == description =>
                {
                    Ok(permission)
                }
                _ => Err(error),
            },
        }
    }

    pub async fn find_permission_by_id(
        &self,
        application_id: of_model::id::Id,
        permission_id: of_model::id::Id,
    ) -> Result<Option<Permission>, ManagementError> {
        self.permission_repo
            .find_permission_by_id(application_id, permission_id)
            .await
    }

    pub async fn delete_permission_by_id(
        &self,
        application_id: of_model::id::Id,
        permission_id: of_model::id::Id,
    ) -> Result<(), ManagementError> {
        self.permission_repo
            .delete_permission_by_id(application_id, permission_id)
            .await
    }

    pub async fn list_role_permissions(
        &self,
        application_id: of_model::id::Id,
        role_id: of_model::id::Id,
    ) -> Result<Vec<Permission>, ManagementError> {
        self.permission_repo
            .list_role_permissions(application_id, role_id)
            .await
    }

    pub async fn add_permission_to_role(
        &self,
        application_id: of_model::id::Id,
        role_id: of_model::id::Id,
        permission_id: of_model::id::Id,
    ) -> Result<(), ManagementError> {
        self.permission_repo
            .add_permission_to_role(application_id, role_id, permission_id)
            .await
    }

    pub async fn remove_permission_from_role(
        &self,
        application_id: of_model::id::Id,
        role_id: of_model::id::Id,
        permission_id: of_model::id::Id,
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
        let permissions: Vec<_> = crate::service::INITIAL_ADMINISTRATOR_ACTIONS
            .iter()
            .map(|action| (Id::now_v7(), *action))
            .collect();
        service
            .provision_initial_administrator(administrator, Id::now_v7(), &permissions)
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
    }

    #[tokio::test]
    async fn independent_policy_provisioning_preserves_grants() {
        let left = ofdb_sql::Database::in_memory();
        let right = ofdb_sql::Database::in_memory();
        let a = ManagementService::new(
            DbPermissionRepo::new(left.client()),
            DbRoleRepo::new(left.client()),
        );
        let b = ManagementService::new(
            DbPermissionRepo::new(right.client()),
            DbRoleRepo::new(right.client()),
        );
        let (app, role, permission, user) =
            (Id::now_v7(), Id::now_v7(), Id::now_v7(), Id::now_v7());
        up(&left.client()).await.expect("left schema");
        a.ensure_role(role, app, "operator", None)
            .await
            .expect("left role");
        a.ensure_permission(permission, app, "operate", None)
            .await
            .expect("left permission");
        a.add_role_to_user(app, user, role)
            .await
            .expect("assign role");
        a.add_permission_to_role(app, role, permission)
            .await
            .expect("assign permission");
        up(&right.client())
            .await
            .expect("right schema after left provisioning");
        b.ensure_role(role, app, "operator", None)
            .await
            .expect("independent role");
        b.ensure_permission(permission, app, "operate", None)
            .await
            .expect("independent permission");
        reconcile(&left, &right).await;
        for owner in [&a, &b] {
            assert!(
                owner
                    .has_user_application_permission(user, app, "operate")
                    .await
                    .expect("evaluate replicated grant")
            );
        }
        let stale = ofdb_sql::Database::in_memory();
        let stale_owner = ManagementService::new(
            DbPermissionRepo::new(stale.client()),
            DbRoleRepo::new(stale.client()),
        );
        let stale_user = Id::now_v7();
        a.delete_permission_by_id(app, permission)
            .await
            .expect("delete shared permission incarnation");
        a.delete_role_by_id(app, role)
            .await
            .expect("delete shared role incarnation");
        // This peer has not seen the deletion and creates fresh physical rows for the old incarnation.
        up(&stale.client()).await.expect("stale schema");
        stale_owner
            .ensure_role(role, app, "operator", None)
            .await
            .expect("offline role");
        stale_owner
            .ensure_permission(permission, app, "operate", None)
            .await
            .expect("offline permission");
        stale_owner
            .add_role_to_user(app, stale_user, role)
            .await
            .expect("offline grant against stale incarnation");
        stale_owner
            .add_permission_to_role(app, role, permission)
            .await
            .expect("offline permission grant");
        reconcile(&left, &stale).await;
        assert!(
            a.find_role_by_id(app, role)
                .await
                .expect("read deleted role")
                .is_none()
        );
        assert!(
            a.find_permission_by_id(app, permission)
                .await
                .expect("read deleted permission")
                .is_none()
        );
        reconcile(&left, &right).await;
        for owner in [&a, &b] {
            owner
                .ensure_role(role, app, "operator", None)
                .await
                .expect("independent role recreation");
            owner
                .ensure_permission(permission, app, "operate", None)
                .await
                .expect("independent permission recreation");
            assert!(
                !owner
                    .has_user_application_permission(user, app, "operate")
                    .await
                    .expect("old grant remains revoked")
            );
        }
        a.add_role_to_user(app, user, role)
            .await
            .expect("new assignment");
        a.add_permission_to_role(app, role, permission)
            .await
            .expect("new permission grant");
        reconcile(&left, &right).await;
        reconcile(&left, &stale).await;
        for owner in [&a, &b, &stale_owner] {
            assert!(
                owner
                    .has_user_application_permission(user, app, "operate")
                    .await
                    .expect("new grant survives convergence")
            );
            assert!(
                !owner
                    .has_user_application_permission(stale_user, app, "operate")
                    .await
                    .expect("stale grant remains denied")
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
    async fn recreating_a_role_does_not_restore_old_grants() {
        let engine = ofdb_sql::Database::in_memory();
        up(&engine.client())
            .await
            .expect("initialize policy schema");
        let service = ManagementService::new(
            DbPermissionRepo::new(engine.client()),
            DbRoleRepo::new(engine.client()),
        );
        let (application, role, user) = (Id::now_v7(), Id::now_v7(), Id::now_v7());
        service
            .ensure_role(role, application, "role", None)
            .await
            .expect("create role");
        service
            .add_role_to_user(application, user, role)
            .await
            .expect("grant role");
        service
            .delete_role_by_id(application, role)
            .await
            .expect("delete role");
        service
            .ensure_role(role, application, "replacement", None)
            .await
            .expect("create replacement");
        assert!(
            service
                .list_user_roles(application, user)
                .await
                .expect("list assigned roles")
                .is_empty()
        );
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
    async fn ensure_rbac_records_retry_by_stable_id_and_reject_conflicts() {
        let engine = ofdb_sql::Database::in_memory();
        up(&engine.client())
            .await
            .expect("initialize management schema");
        let service = ManagementService::new(
            DbPermissionRepo::new(engine.client()),
            DbRoleRepo::new(engine.client()),
        );
        let role_id = Id::parse_str("6c1566bd-5f64-4b04-9bc6-52a1b70930aa").expect("domain ID");
        let application_id = Id::now_v7();
        assert!(
            service
                .ensure_role(Id::nil(), application_id, "administrator", None)
                .await
                .is_err()
        );

        let created = service
            .ensure_role(role_id, application_id, "administrator", None)
            .await
            .expect("create role with stable ID");
        let retried = service
            .ensure_role(role_id, application_id, "administrator", None)
            .await
            .expect("retry returns the same role");
        assert_eq!(created.id, role_id);
        assert_eq!(created, retried);
        let permission_id =
            Id::parse_str("d3496cc3-b3b5-4fe5-99e0-83db03d281b0").expect("domain ID");
        let permission = service
            .ensure_permission(permission_id, application_id, "test-permission", None)
            .await
            .expect("create permission with stable ID");
        let retried = service
            .ensure_permission(permission_id, application_id, "test-permission", None)
            .await
            .expect("retry returns the same permission");
        assert_eq!(permission.id, permission_id);
        assert_eq!(permission, retried);
        let conflicting = service
            .ensure_permission(permission_id, application_id, "conflicting-name", None)
            .await;
        assert!(conflicting.is_err());
    }
}
