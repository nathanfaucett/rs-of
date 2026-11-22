use of_model::http;
use of_model::node_join::NodeJoinRequest;
use of_model::setup::IndependentCreateIntent;

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
    let fixture = r#"{"purpose":"of.equal-node-join.v1","installation":"https://issuer.test","member_id":"00000000-0000-7000-8000-000000000001","endpoint_id":"endpoint","name":"node","approving_peer":"peer"}"#;
    let request: NodeJoinRequest = serde_json::from_str(fixture).expect("signed join request");
    let client: http::device::NodeJoinRequest = request;
    let wire = serde_json::to_string(&client).expect("serialize join request");
    assert_eq!(
        wire,
        r#"{"purpose":"of.equal-node-join.v1","installation":"https://issuer.test","member_id":"00000000-0000-7000-8000-000000000001","endpoint_id":"endpoint","name":"node","approving_peer":"peer"}"#
    );
    serde_json::from_str::<NodeJoinRequest>(
        &fixture.replace("\"name\":\"node\"", "\"name\":\"node\",\"untrusted\":true"),
    )
    .unwrap_err();
}

#[test]
fn sdk_sync_result_preserves_its_wire_shape_with_alloc_only() {
    use of_model::storage::sdk_sync::SyncResult;
    let wire = serde_json::to_value(SyncResult::default()).expect("SDK result");
    assert_eq!(wire["status"], "completed");
}

#[cfg(feature = "std")]
#[test]
fn sdk_identifier_constructors_make_v7_ids() {
    use of_model::storage::sdk::DatabaseId;
    use of_model::storage::sdk::FilesystemId;
    use of_model::storage::sdk::KvStoreId;
    use of_model::storage::sdk::ResourceId;
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
    serde_json::from_str::<http::idp::CreateUserRequest>(
        r#"{"name":"user","email":"user@test","password":"secret","admin":true}"#,
    )
    .unwrap_err();
    serde_json::from_str::<http::idp::UpdateUserInfoRequest>(r#"{"admin":true}"#).unwrap_err();
}
