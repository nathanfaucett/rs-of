use of_model::{
    config::ServerConfig,
    http::device::selection_request::SelectionRequest as ClientSelectionRequest,
    server::{device::device_selection::SelectionRequest, storage::sql::SqlRequest},
    storage::resource_catalog::ResourceKind,
};

#[test]
fn device_client_and_server_accept_the_same_selection_contract() {
    let fixture = r#"{
        "applicationId":"00000000-0000-7000-8000-000000000001",
        "kind":"database",
        "id":"00000000-0000-7000-8000-000000000002",
        "storageAccessToken":"scoped-token"
    }"#;
    let client: ClientSelectionRequest =
        serde_json::from_str(fixture).expect("client selection contract");
    let request: SelectionRequest =
        serde_json::from_value(serde_json::to_value(client).expect("serialize client request"))
            .expect("server accepts client request");
    assert_eq!(request.kind.resource_kind(), ResourceKind::Database);
    assert_eq!(request.kind.storage_audience(), "storage:databases");
    assert_eq!(request.kind.policy_kind(), "database");
    assert_eq!(request.storage_access_token, "scoped-token");
    serde_json::from_str::<SelectionRequest>(
        r#"{"applicationId":"00000000-0000-7000-8000-000000000001","kind":"database","id":"00000000-0000-7000-8000-000000000002"}"#,
    )
    .expect_err("selection requires a storage access token");
}

#[test]
fn sql_contract_keeps_default_parameters_and_rejects_unknown_fields() {
    let request: SqlRequest =
        serde_json::from_str(r#"{"statements":[{"sql":"SELECT id FROM users"}]}"#)
            .expect("SQL request without parameters");
    assert!(request.statements[0].params.is_empty());
    serde_json::from_str::<SqlRequest>(r#"{"statements":[],"untrustedOption":true}"#)
        .expect_err("SQL requests reject unknown fields");
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
    use of_model::server::management::permissions::CreatePermissionRequest;
    use of_model::server::storage::resources::EnsureDatabaseRequest;
    use utoipa::PartialSchema;

    for schema in [
        SelectionRequest::schema(),
        SqlRequest::schema(),
        CreatePermissionRequest::schema(),
        EnsureDatabaseRequest::schema(),
    ] {
        let value = serde_json::to_value(schema).expect("serialize owner schema");
        assert!(value.get("properties").is_some());
    }
}
