use std::{sync::Arc, time::Duration};

use db::{Database, SqlTranslator};
use idp_model::{
    contract::{INSTALLATION_POLICY_ID, IdentityAction},
    model::Id,
};
use idp_service::{oauth2::NodeSignerBinding, replica::DbReplicaSignerRepo, repo::UserRepo};
use iroh::{SecretKey, endpoint::presets};
use iroh_chain::{EndpointIdStore, Server};
use management_service::{
    DeviceRepo, ManagementService,
    replica::{DbPermissionRepo, DbRoleRepo},
};
use tokio::sync::Notify;

use crate::{
    AppConfig, BootstrapProtocolHandler, OwnerProvisioner,
    node_join_tests::{ApprovalFixture, approval_fixture, approval_fixture_for_issuer, user_token},
    node_provisioning_tests::Secrets,
};

async fn authorize_join(fixture: &ApprovalFixture) -> String {
    let service = ManagementService::new(
        DbPermissionRepo::new(fixture.management.clone()),
        DbRoleRepo::new(fixture.management.clone()),
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
        IdentityAction::ReplicaSignersEnroll,
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
    user_token(fixture, management_service::MANAGEMENT_APPLICATION_URI).await
}

#[tokio::test]
async fn node_join_oversized_signed_reply_rejects_before_any_enrollment_mutation() {
    let root = std::env::temp_dir().join(format!("node-join-size-{}", Id::now_v7()));
    let joining = Server::bind_with_secret_key(
        presets::Minimal,
        SecretKey::generate(),
        EndpointIdStore::default(),
    )
    .await
    .expect("joining endpoint");
    joining.set_pairing_enabled(true);
    let config = AppConfig {
        data_dir: root.to_string_lossy().into_owned(),
        ..Default::default()
    };
    let provisioner = OwnerProvisioner::open_with_private_keys(&config, Secrets::default())
        .await
        .expect("independent signer owner");
    provisioner
        .prepare_fresh_setup_secret_inventory(Id::now_v7(), &[])
        .await
        .expect("fresh joining secret inventory");
    provisioner
        .prepare_setup_client_slots(&[])
        .await
        .expect("ready joining secret inventory");
    let mut request = provisioner
        .prepare_node_join(
            &NodeSignerBinding {
                member_id: Id::now_v7(),
                key_id: Id::now_v7(),
                endpoint_id: joining.endpoint().id().to_string(),
            },
            &"n".repeat(256),
            SecretKey::generate().public(),
        )
        .expect("real independent public signer");
    request.installation = "https://installation.test/idp/".into();
    let limit = iroh_chain::MAX_PAIRING_PAYLOAD_LENGTH;
    let padding = limit - 32 - serde_json::to_vec(&request).expect("request size").len();
    request.installation.push_str(&"i".repeat(padding));
    let fixture = approval_fixture_for_issuer(request.installation.clone()).await;
    request.approving_peer = fixture.server.endpoint().id().to_string();
    let approval = crate::NodeJoinApproval::signed_by(
        &fixture.state.device_identity,
        request.clone(),
        fixture.owner,
    )
    .expect("prospective signed approval");
    assert_eq!(
        serde_json::to_vec(&request)
            .expect("offer serialization")
            .len(),
        limit - 32,
        "offer fits the existing pairing limit"
    );
    assert!(
        serde_json::to_vec(&approval)
            .expect("reply serialization")
            .len()
            > limit,
        "signed reply exceeds that same pairing limit"
    );
    fixture.state.engine.client().translate_and_execute(
        "CREATE TABLE IF NOT EXISTS local_node_join_intents (id UUID PRIMARY KEY, binding TEXT NOT NULL)", &SqlTranslator,
    ).await.expect("empty local intent fixture");
    let bearer = authorize_join(&fixture).await;
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
        .expect("bounded fitting offer")
        .expect("authenticated offer");
    let error = fixture
        .state
        .approve_node_join(&offer, &bearer)
        .await
        .expect_err("oversized prospective reply must reject");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    let results = fixture.state.engine.client().translate_and_execute(&format!(
        "SELECT id FROM local_node_join_intents; SELECT id FROM idp_replica_members WHERE member_id = CAST('{}' AS UUID); SELECT id FROM idp_replica_signers WHERE key_id = CAST('{}' AS UUID)",
        request.member_id, request.public_signer.kid,
    ), &SqlTranslator).await.expect("public owner persistence evidence");
    assert_eq!(results.len(), 3);
    for (context, result) in ["intent", "member", "signer"].into_iter().zip(results) {
        assert!(
            result.rows.is_empty(),
            "oversized reply must not create {context}"
        );
    }
    assert!(
        fixture
            .state
            .devices
            .as_ref()
            .expect("injected Management Device owner")
            .find_approved_by_public_key(&request.endpoint_id)
            .await
            .expect("Device lookup")
            .is_none()
    );
    assert!(
        !fixture
            .state
            .replica_signers
            .trusted_signers(&fixture.issuer, i64::MAX)
            .await
            .expect("no new trust")
            .iter()
            .any(|(member, signer)| member.member_id == request.member_id
                || member.endpoint_id == request.endpoint_id
                || signer.public_jwk == request.public_signer)
    );
    sending.abort();
    let _ = sending.await;
    joining.endpoint().close().await;
    router.shutdown().await.expect("stop peer");
    let fixture_root = fixture.root.clone();
    drop(fixture);
    drop(provisioner);
    std::fs::remove_dir_all(fixture_root).expect("remove isolated approving owner");
    std::fs::remove_dir_all(root).expect("remove isolated signing owner");
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
        oauth2: idp_service::oauth2::OAuth2Config {
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
    assert!(
        !fixture
            .state
            .replica_signers
            .trusted_signers(&fixture.issuer, i64::MAX)
            .await
            .expect("current trust")
            .iter()
            .any(|(member, signer)| member.member_id == request.member_id
                || member.endpoint_id == request.endpoint_id
                || signer.public_jwk == request.public_signer),
        "rejected approval must not activate signer/member trust for the changed Device"
    );
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
    assert!(
        !DbReplicaSignerRepo::new(engine.clone())
            .trusted_signers(&issuer, i64::MAX)
            .await
            .expect("persisted trust")
            .iter()
            .any(|(member, signer)| member.member_id == request.member_id
                || member.endpoint_id == request.endpoint_id
                || signer.public_jwk == request.public_signer),
        "restart must not restore trust from rejected approval"
    );
    let management = Arc::new(
        Database::open(root.join("management/management.redb"))
            .expect("offline Management owner reopen"),
    );
    let reopened = management_service::replica::DbDeviceRepo::new(management.clone())
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
async fn node_join_rechecks_approving_device_revocation_separately_from_exact_signer_enrollment() {
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
        oauth2: idp_service::oauth2::OAuth2Config {
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
    let trusted_before_revocation = fixture
        .state
        .replica_signers
        .trusted_signers(&fixture.issuer, i64::MAX)
        .await
        .expect("valid signer enrollment");
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
        vec![db::Row::new(vec![
            db::Value::Integer(1),
            db::Value::Integer(1),
            db::Value::Text(fixture.owner.to_string())
        ])]
    );
    fixture
        .state
        .oauth2_service
        .find_user_info(fixture.owner)
        .await
        .expect("owner remains active");
    assert_eq!(
        fixture
            .state
            .replica_signers
            .trusted_signers(&fixture.issuer, i64::MAX)
            .await
            .expect("signers after Device timestamp revocation"),
        trusted_before_revocation
    );
    assert!(
        fixture
            .state
            .verify_node_join_enrollment(&approval)
            .await
            .is_err(),
        "timestamp-only approving Device revocation must deny exact enrollment despite valid signer and active owner"
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
            .replica_signers
            .trusted_signers(&fixture.issuer, i64::MAX)
            .await
            .expect("independent joining signer trust")
            .iter()
            .any(|(member, signer)| member.member_id == request.member_id
                && member.endpoint_id == request.endpoint_id
                && signer.public_jwk == request.public_signer)
    );
    assert!(
        fixture
            .state
            .verify_node_join_enrollment(&approval)
            .await
            .is_err(),
        "approving endpoint revocation must deny even with exact joining signer enrollment"
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
