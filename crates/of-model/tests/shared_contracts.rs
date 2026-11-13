use of_model::{
    config::ServerConfig,
    server::{device::device_selection::SelectionRequest, storage::sql::SqlRequest},
    storage::ResourceKind,
};

#[test]
fn device_client_and_server_accept_the_same_selection_contract() {
    let fixture = r#"{
        "applicationId":"00000000-0000-7000-8000-000000000001",
        "kind":"database",
        "id":"00000000-0000-7000-8000-000000000002",
        "storageAccessToken":"scoped-token"
    }"#;
    let client: of_model::http::device::SelectionRequest =
        serde_json::from_str(fixture).expect("client selection contract");
    let request: SelectionRequest =
        serde_json::from_value(serde_json::to_value(client).expect("serialize client request"))
            .expect("server accepts client request");
    assert_eq!(request.kind.resource_kind(), ResourceKind::Database);
    assert_eq!(request.kind.storage_audience(), "storage:databases");
    assert_eq!(request.kind.policy_kind(), "database");
    assert_eq!(request.storage_access_token, "scoped-token");
    assert!(serde_json::from_str::<SelectionRequest>(
        r#"{"applicationId":"00000000-0000-7000-8000-000000000001","kind":"database","id":"00000000-0000-7000-8000-000000000002"}"#
    ).is_err());
}

#[test]
fn sql_contract_keeps_default_parameters_and_rejects_unknown_fields() {
    let request: SqlRequest =
        serde_json::from_str(r#"{"statements":[{"sql":"SELECT id FROM users"}]}"#)
            .expect("SQL request without parameters");
    assert!(request.statements[0].params.is_empty());
    assert!(
        serde_json::from_str::<SqlRequest>(r#"{"statements":[],"untrustedOption":true}"#,).is_err()
    );
}

#[test]
fn server_config_preserves_prefix_normalization() {
    let config: ServerConfig =
        serde_json::from_str(r#"{"prefix":"/idp///"}"#).expect("shared server config");
    assert_eq!(config.prefix(), "/idp");
    assert_eq!(ServerConfig::default().prefix(), "");
}

#[cfg(feature = "utoipa")]
#[test]
fn owner_schemas_are_available_from_the_model_crate() {
    use of_model::server::{
        idp::replica_signers::EnrollReplicaSigner,
        management::permissions::CreatePermissionRequest, storage::resources::SyncConnectRequest,
    };
    use utoipa::PartialSchema;

    for schema in [
        SelectionRequest::schema(),
        SqlRequest::schema(),
        EnrollReplicaSigner::schema(),
        CreatePermissionRequest::schema(),
        SyncConnectRequest::schema(),
    ] {
        let value = serde_json::to_value(schema).expect("serialize owner schema");
        assert!(value.get("properties").is_some());
    }
}
