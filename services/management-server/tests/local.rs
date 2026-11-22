use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use db::adapters::MemoryDatabase;
use ofdb_sql::Database as SqlDatabase;
use tower::util::ServiceExt;

#[tokio::test]
async fn local_constructor_accepts_shared_and_owner_databases_without_setup() {
    for injected in [true, false] {
        let root =
            std::env::temp_dir().join(format!("management-local-{}", of_model::id::Id::now_v7()));
        let shared: Option<Arc<SqlDatabase>> = if injected {
            let adapter = MemoryDatabase::new().await.expect("shared schema");
            Some(Arc::new(adapter.database()))
        } else {
            None
        };
        let router = management_server::runtime::build_local_router(
            &root,
            1355,
            "http://127.0.0.1:1355/idp",
            "http://127.0.0.1:1355/storage",
            shared,
            None,
        )
        .await
        .expect("local constructor");
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/management/health")
                    .body(Body::empty())
                    .expect("health request"),
            )
            .await
            .expect("health response");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(root.join("unified.redb").exists(), !injected);
        assert!(!root.join("management.setup-host").exists());
        if root.exists() {
            std::fs::remove_dir_all(root).expect("remove owner data");
        }
    }
}

#[tokio::test]
async fn synchronized_management_history_preserves_policy_and_selections() {
    use management_service::{
        ManagementService,
        replica::{DbPermissionRepo, DbRoleRepo},
    };
    use of_model::id::Id;
    use of_model::idp::contract::permission_evaluation::{
        IdentityAction, IdentityResource, PermissionEvaluationRequest, PermissionSubject,
        PermissionTarget,
    };

    let root = std::env::temp_dir().join(format!("management-history-{}", Id::now_v7()));
    std::fs::create_dir(&root).expect("private test directory");
    let path = root.join("policy.redb");
    let source = SqlDatabase::open(&path).expect("open owner database");
    db::ofdb::up(&source.client())
        .await
        .expect("initialize owner schema");
    let service = ManagementService::new(
        DbPermissionRepo::new(source.client()),
        DbRoleRepo::new(source.client()),
    );
    let administrator = Id::now_v7();
    management_service::provision_initial_administrator(
        &source.client(),
        administrator,
        &management_service::INITIAL_ADMINISTRATOR_ACTIONS,
    )
    .await
    .expect("provision policy on owner");
    let (device, resource, application) = (Id::now_v7(), Id::now_v7(), Id::now_v7());
    service
        .select_resource(device, resource, application, "owner")
        .await
        .expect("select resource");
    drop(service);
    drop(source);

    let source = SqlDatabase::open_existing(&path).expect("restart owner");
    let peer = SqlDatabase::in_memory();
    peer.import_history(source.export_history().await.expect("export owner history"))
        .await
        .expect("apply owner history to peer");
    let replica = ManagementService::new(
        DbPermissionRepo::new(peer.client()),
        DbRoleRepo::new(peer.client()),
    );
    assert!(
        replica
            .evaluate_permission(&PermissionEvaluationRequest {
                request_id: Id::now_v7(),
                subject: PermissionSubject::User { id: administrator },
                action: IdentityAction::ClientsRegister,
                target: PermissionTarget::Installation {
                    resource: IdentityResource::Client { client_id: None }
                },
            })
            .await
            .expect("evaluate peer policy")
    );
    assert!(
        replica
            .has_selection(device, resource)
            .await
            .expect("read peer Selection")
    );
    replica
        .deselect_resource(device, resource)
        .await
        .expect("revoke peer Selection");
    source
        .import_history(peer.export_history().await.expect("export peer history"))
        .await
        .expect("apply peer history to owner");
    let reopened = ManagementService::new(
        DbPermissionRepo::new(source.client()),
        DbRoleRepo::new(source.client()),
    );
    assert!(
        !reopened
            .has_selection(device, resource)
            .await
            .expect("observe peer revocation")
    );
    drop(reopened);
    drop(source);
    std::fs::remove_dir_all(root).expect("remove owner data");
}
