use of_model::{
    http, node_join::NodeJoinRequest, setup::IndependentCreateIntent, storage::sync::SyncStatus,
};

#[test]
fn all_owners_use_one_setup_wire_type() {
    let intent: IndependentCreateIntent = serde_json::from_str(r#"{"administrator":"admin","device":"https://device.test","idp":"https://idp.test","issuer":"https://idp.test","node":"node","operation_nonce":"0123456789abcdef0123456789abcdef","purpose":"create","storage":"https://storage.test"}"#).expect("owner setup intent");
    let device: http::device::IndependentCreateIntent = intent;
    let idp: http::idp::IndependentCreateIntent = device;
    let storage: http::storage::IndependentCreateIntent = idp;
    let _: http::storage_api::IndependentCreateIntent = storage;
}

#[test]
fn join_request_keeps_the_signed_field_order() {
    let fixture = r#"{"purpose":"of.equal-node-join.v1","installation":"https://issuer.test","member_id":"00000000-0000-7000-8000-000000000001","endpoint_id":"endpoint","name":"node","public_signer":{"use":"sig","alg":"EdDSA","kty":"Opk","crv":"Ed25519","x":"key","kid":"00000000-0000-7000-8000-000000000002"},"approving_peer":"peer"}"#;
    let request: NodeJoinRequest = serde_json::from_str(fixture).expect("signed join request");
    let client: http::device::NodeJoinRequest = request;
    let wire = serde_json::to_string(&client).expect("serialize join request");
    assert_eq!(
        wire,
        r#"{"purpose":"of.equal-node-join.v1","installation":"https://issuer.test","member_id":"00000000-0000-7000-8000-000000000001","endpoint_id":"endpoint","name":"node","public_signer":{"use":"sig","kid":"00000000-0000-7000-8000-000000000002","alg":"EdDSA","kty":"Opk","crv":"Ed25519","x":"key"},"approving_peer":"peer"}"#
    );
    assert!(
        serde_json::from_str::<NodeJoinRequest>(
            &fixture.replace("\"name\":\"node\"", "\"name\":\"node\",\"untrusted\":true")
        )
        .is_err()
    );
}

#[test]
fn sync_session_status_has_one_identity_and_camel_case_wire_values() {
    for (status, expected) in [
        (SyncStatus::Connecting, "\"connecting\""),
        (SyncStatus::Syncing, "\"syncing\""),
        (SyncStatus::Completed, "\"completed\""),
        (SyncStatus::Failed, "\"failed\""),
        (SyncStatus::Cancelled, "\"cancelled\""),
    ] {
        let server: of_model::server::storage::resources::SyncStatus = status;
        let client: http::storage::sync_session_response::Status = server;
        let _: http::storage_api::sync_session_response::Status = client;
        assert_eq!(
            serde_json::to_string(&status).expect("sync status"),
            expected
        );
        assert_eq!(
            serde_json::from_str::<SyncStatus>(expected).expect("sync status"),
            status
        );
    }
}

#[test]
fn sdk_sync_payloads_preserve_their_wire_shape_with_alloc_only() {
    use of_model::storage::sdk::ResourceId;
    use of_model::storage::sdk_sync::{DeviceId, SyncResult, SyncSession, SyncStatus};

    let id = of_model::Id::parse_str("00000000-0000-7000-8000-000000000001").expect("fixture ID");
    let session = SyncSession {
        session_id: id,
        resource_id: ResourceId::from_uuid(id),
        source_device: DeviceId::from_uuid(id),
        target_device: DeviceId::from(id),
        status: SyncStatus::Completed,
        bytes_sent: u64::MAX,
        bytes_received: 0,
    };
    let wire = serde_json::to_value(session).expect("SDK session");
    assert_eq!(wire["status"], "completed");
    assert_eq!(wire["bytes_sent"], u64::MAX);
    assert_eq!(wire["resource_id"], "00000000-0000-7000-8000-000000000001");
    let decoded: SyncSession = serde_json::from_value(wire).expect("SDK session round trip");
    assert_eq!(decoded.resource_id.as_uuid(), id);
    assert_eq!(decoded.source_device.as_uuid(), id);
    assert_eq!(of_model::Id::from(decoded.target_device), id);
    assert_eq!(SyncResult::default().status, SyncStatus::Completed);
}

#[cfg(feature = "std")]
#[test]
fn sdk_identifier_constructors_make_v7_ids() {
    use of_model::storage::sdk::{DatabaseId, FilesystemId, KvStoreId, ResourceId};
    use of_model::storage::sdk_sync::DeviceId;

    for id in [
        DatabaseId::new().as_uuid(),
        FilesystemId::new().as_uuid(),
        KvStoreId::new().as_uuid(),
        ResourceId::new().as_uuid(),
        DeviceId::new().as_uuid(),
    ] {
        assert_eq!(id.get_version_num(), 7);
    }
}

#[test]
fn oauth_mutations_reject_unknown_fields() {
    assert!(
        serde_json::from_str::<http::idp::CreateUserRequest>(
            r#"{"name":"user","email":"user@test","password":"secret","admin":true}"#
        )
        .is_err()
    );
    assert!(serde_json::from_str::<http::idp::UpdateUserInfoRequest>(r#"{"admin":true}"#).is_err());
}
