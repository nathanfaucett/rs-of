use std::{sync::Arc, time::Duration};

use device_service::hosted_join::{NodeJoinApprovalExt, NodeJoinRequestExt};

use ofdb_sql::Database;

use device_service::device_repo::DeviceRepo;
use idp_service::{oauth2::node_signer::NodeSignerBinding, repo::user_repo::UserRepo};
use iroh::{SecretKey, endpoint::presets};
use iroh_chain::{EndpointIdStore, Server};
use management_service::ofdb::DbPermissionRepo;
use management_service::ofdb::DbRoleRepo;
use management_service::service::ManagementService;
use of_model::id::Id;
use of_model::idp::contract::permission_evaluation::INSTALLATION_POLICY_ID;
use of_model::idp::contract::permission_evaluation::IdentityAction;
use ofdb_sql::SqlTranslator;
use tokio::sync::Notify;

use crate::{
    bootstrap::handler::BootstrapProtocolHandler,
    config::app_config::AppConfig,
    node_join_tests::{ApprovalFixture, approval_fixture, approval_fixture_for_issuer, user_token},
    node_provisioning_tests::Secrets,
    provisioning::OwnerProvisioner,
};

async fn authorize_join(fixture: &ApprovalFixture) -> String {
    let service = ManagementService::new(
        DbPermissionRepo::new(fixture.management.client()),
        DbRoleRepo::new(fixture.management.client()),
    );
    let role = service
        .create_role(INSTALLATION_POLICY_ID, "Join review approver", None)
        .await
        .expect("explicit approver role");
    service
        .add_role_to_user(INSTALLATION_POLICY_ID, fixture.owner, role.id)
        .await
        .expect("approver User");
    for action in [
        IdentityAction::DevicePairingUpdate,
        IdentityAction::DevicesEnroll,
    ] {
        let permission = service
            .create_permission(INSTALLATION_POLICY_ID, action.permission(), None)
            .await
            .expect("exact join permission");
        service
            .add_permission_to_role(INSTALLATION_POLICY_ID, role.id, permission.id)
            .await
            .expect("assign join permission");
    }
    user_token(
        fixture,
        management_service::service::MANAGEMENT_APPLICATION_URI,
    )
    .await
}

#[tokio::test]
async fn node_join_owner_interleaving_rejection_has_no_active_trust_after_reopen() {
    let mut fixture = approval_fixture().await;
    let bearer = authorize_join(&fixture).await;
    let joining = Server::bind_with_secret_key(
        presets::Minimal,
        SecretKey::generate(),
        EndpointIdStore::default(),
    )
    .await
    .expect("joining endpoint");
    joining.set_pairing_enabled(true);
    let device = fixture
        .state
        .devices
        .as_ref()
        .expect("injected Management Device owner")
        .create(
            fixture.owner.to_string(),
            "Joining Node".into(),
            joining.endpoint().id().to_string(),
            String::new(),
            b"review-enrollment-hash".to_vec(),
            i64::MAX,
        )
        .await
        .expect("Device-only enrollment");
    fixture
        .state
        .devices
        .as_ref()
        .expect("injected Management Device owner")
        .approve(device.id, b"review-enrollment-hash")
        .await
        .expect("Device approval")
        .expect("approved Device");
    let config = AppConfig {
        data_dir: fixture.root.join("joining").to_string_lossy().into_owned(),
        oauth2: idp_service::oauth2::config::OAuth2Config {
            issuer: fixture.issuer.clone(),
            ..Default::default()
        },
        ..Default::default()
    };
    let provisioner = OwnerProvisioner::open_with_private_keys(&config, Secrets::default())
        .await
        .expect("independent joining signer");
    provisioner
        .prepare_fresh_setup_secret_inventory(fixture.owner, &[])
        .await
        .expect("fresh joining secret inventory");
    provisioner
        .prepare_setup_client_slots(&[])
        .await
        .expect("ready joining secret inventory");
    let request = provisioner
        .prepare_node_join(
            &NodeSignerBinding {
                member_id: device.id,
                key_id: Id::now_v7(),
                endpoint_id: joining.endpoint().id().to_string(),
            },
            "Joining Node",
            fixture.server.endpoint().id(),
        )
        .expect("join request");
    let reached = Arc::new(Notify::new());
    let resume = Arc::new(Notify::new());
    fixture.state.before_node_join_activation = Some((reached.clone(), resume.clone()));
    let router = fixture.server.router(BootstrapProtocolHandler);
    fixture.server.set_pairing_enabled(true);
    let mut offers = fixture.server.subscribe_pairing_offers();
    let sending = tokio::spawn({
        let joining = joining.clone();
        let address = fixture.server.endpoint().addr();
        let request = request.clone();
        async move { request.send(&joining, address).await }
    });
    let offer = tokio::time::timeout(Duration::from_secs(15), offers.recv())
        .await
        .expect("bounded offer")
        .expect("authenticated pairing offer");
    let approving = tokio::spawn({
        let state = fixture.state.clone();
        async move { state.approve_node_join(&offer, &bearer).await }
    });
    tokio::time::timeout(Duration::from_secs(15), reached.notified())
        .await
        .expect("approval reached activation boundary");
    let replacement = fixture
        .state
        .oauth2_service
        .user_repo
        .create_user_with_password("replacement-owner", "valid-password")
        .await
        .expect("real replacement User");
    fixture.management.client().translate_and_execute(&format!("UPDATE devices SET owner_subject = '{}', name = 'Changed Node' WHERE id = CAST('{}' AS UUID)", replacement.id, device.id), &SqlTranslator).await.expect("independent owner-state writer wins before activation");
    resume.notify_one();
    tokio::time::timeout(Duration::from_secs(15), approving)
        .await
        .expect("bounded approval")
        .expect("approval task")
        .expect_err("changed Device binding rejects approval");

    let current = fixture
        .state
        .devices
        .as_ref()
        .expect("injected Management Device owner")
        .find_approved_by_public_key(&request.endpoint_id)
        .await
        .expect("current Device")
        .expect("Device preserved");
    assert_eq!(current.owner_subject, replacement.id.to_string());
    assert_eq!(current.name, "Changed Node");
    sending.abort();
    let _ = sending.await;
    joining.endpoint().close().await;
    router.shutdown().await.expect("stop peer router");
    drop(provisioner);
    let root = fixture.root.clone();
    let issuer = fixture.issuer.clone();
    drop(fixture);
    let engine =
        Arc::new(Database::open(root.join("idp.redb")).expect("offline native owner reopen"));

    let management = Arc::new(
        Database::open(root.join("management/management.redb"))
            .expect("offline Management owner reopen"),
    );
    let reopened =
        device_service::ofdb::device_repo::DbDeviceRepo::new(management.clone().client())
            .find_approved_by_public_key(&request.endpoint_id)
            .await
            .expect("reopened Device admission")
            .expect("replacement owner's Device remains approved");
    assert_eq!(reopened.id, device.id);
    assert_eq!(reopened.owner_subject, replacement.id.to_string());
    assert_eq!(reopened.name, "Changed Node");
    drop(management);
    drop(engine);
    std::fs::remove_dir_all(root).expect("remove isolated native stores");
}

#[tokio::test]
async fn node_join_rechecks_approving_device_revocation_after_device_enrollment() {
    let fixture = approval_fixture().await;
    let bearer = authorize_join(&fixture).await;
    let joining = Server::bind_with_secret_key(
        presets::Minimal,
        SecretKey::generate(),
        EndpointIdStore::default(),
    )
    .await
    .expect("joining endpoint");
    joining.set_pairing_enabled(true);
    let config = AppConfig {
        data_dir: fixture.root.join("joining").to_string_lossy().into_owned(),
        oauth2: idp_service::oauth2::config::OAuth2Config {
            issuer: fixture.issuer.clone(),
            ..Default::default()
        },
        ..Default::default()
    };
    let provisioner = OwnerProvisioner::open_with_private_keys(&config, Secrets::default())
        .await
        .expect("independent joining signer");
    provisioner
        .prepare_fresh_setup_secret_inventory(fixture.owner, &[])
        .await
        .expect("fresh joining secret inventory");
    provisioner
        .prepare_setup_client_slots(&[])
        .await
        .expect("ready joining secret inventory");
    let request = provisioner
        .prepare_node_join(
            &NodeSignerBinding {
                member_id: Id::now_v7(),
                key_id: Id::now_v7(),
                endpoint_id: joining.endpoint().id().to_string(),
            },
            "Joining Node",
            fixture.server.endpoint().id(),
        )
        .expect("exact join request");
    let router = fixture.server.router(BootstrapProtocolHandler);
    fixture.server.set_pairing_enabled(true);
    let mut offers = fixture.server.subscribe_pairing_offers();
    let sending = tokio::spawn({
        let joining = joining.clone();
        let address = fixture.server.endpoint().addr();
        let request = request.clone();
        async move { request.send(&joining, address).await }
    });
    let offer = tokio::time::timeout(Duration::from_secs(15), offers.recv())
        .await
        .expect("bounded offer")
        .expect("authenticated offer");
    let approval = fixture
        .state
        .enroll_node_join_offer(&offer, &bearer)
        .await
        .expect("exact enrollment");
    fixture
        .state
        .verify_node_join_enrollment(&approval)
        .await
        .expect("current enrollment");
    let approving_device = fixture
        .state
        .devices
        .as_ref()
        .expect("injected Management Device owner")
        .find_approved_by_public_key(&request.approving_peer)
        .await
        .expect("approving Device lookup")
        .expect("approved endpoint");
    fixture
        .management
        .client()
        .translate_and_execute(
            &format!(
                "UPDATE devices SET revoked_at = 1 WHERE id = CAST('{}' AS UUID)",
                approving_device.id
            ),
            &SqlTranslator,
        )
        .await
        .expect("learn timestamp-only approving Device revocation");
    let rows = fixture.management.client().translate_and_execute(&format!(
        "SELECT state, revoked_at, owner_subject FROM devices WHERE id = CAST('{}' AS UUID)", approving_device.id),
        &SqlTranslator).await.expect("timestamp-only approving Device state");
    assert_eq!(
        rows[0].rows,
        vec![ofdb_sql::Row::new(vec![
            ofdb_sql::Value::Integer(1),
            ofdb_sql::Value::Integer(1),
            ofdb_sql::Value::Text(fixture.owner.to_string())
        ])]
    );
    fixture
        .state
        .oauth2_service
        .find_user_info(fixture.owner)
        .await
        .expect("owner remains active");

    assert!(
        fixture
            .state
            .verify_node_join_enrollment(&approval)
            .await
            .is_err(),
        "timestamp-only approving Device revocation must deny exact enrollment despite the active owner"
    );
    assert!(
        fixture
            .state
            .reply_node_join(&offer, &bearer, &approval)
            .await
            .is_err(),
        "timestamp-only approving Device revocation must deny reply publication"
    );
    assert!(
        fixture
            .state
            .enroll_node_join_offer(&offer, &bearer)
            .await
            .is_err(),
        "timestamp-only approving Device revocation must deny approval retry"
    );
    assert!(
        fixture
            .state
            .devices
            .as_ref()
            .expect("injected Management Device owner")
            .revoke_self(&request.approving_peer)
            .await
            .expect("revoke approving Device")
    );

    assert!(
        fixture
            .state
            .verify_node_join_enrollment(&approval)
            .await
            .is_err(),
        "approving endpoint revocation must deny after Device enrollment"
    );
    assert!(
        fixture
            .state
            .reply_node_join(&offer, &bearer, &approval)
            .await
            .is_err(),
        "revoked approving Device cannot publish a previously enrolled reply"
    );
    assert!(
        fixture
            .state
            .enroll_node_join_offer(&offer, &bearer)
            .await
            .is_err(),
        "revoked approving Device cannot approve a retry"
    );
    sending.abort();
    let _ = sending.await;
    joining.endpoint().close().await;
    router.shutdown().await.expect("stop approving router");
    drop(provisioner);
    let root = fixture.root.clone();
    drop(fixture);
    std::fs::remove_dir_all(root).expect("remove isolated enrollment fixture");
}
