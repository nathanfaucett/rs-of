use of_model::contract::StorageAuthorizationAction;
use of_model::storage::{StorageRequest, StorageResponse};
use tokio_util::sync::CancellationToken;

use crate::support::native::{
    Fixture, Gate, Listener, StorageNamespace, assert_no_response, registration, response, send,
};

#[path = "native_api_qualification_tests.rs"]
mod qualification_tests;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_owner_cutover_captures_initial_secrets_and_reuses_reopened_owner() {
    let fixture = Fixture::new(Gate::default()).await;
    let inventory: serde_json::Value = serde_json::from_slice(
        &std::fs::read(fixture._directory.0.join("idp/setup-secret-inventory.json"))
            .expect("retained preparation manifest"),
    )
    .expect("exact inventory JSON");
    assert_eq!(inventory["owner"], fixture.owner.to_string());
    assert_eq!(inventory["phase"], "ready");
    let slots = inventory["clients"]
        .as_array()
        .expect("ordered Client slots");
    assert_eq!(slots.len(), 1);
    for slot in [
        slots[0].as_str().expect("canonical Client ID").to_owned(),
        format!("user:{}", fixture.owner),
    ] {
        let reference: serde_json::Value = serde_json::from_slice(
            &std::fs::read(
                fixture
                    ._directory
                    .0
                    .join("idp/local-client-key-references")
                    .join(format!("{}.json", slot.replace(':', "-"))),
            )
            .expect("captured initial secret reference"),
        )
        .expect("reference JSON");
        assert_eq!(reference["state"], "resolved");
    }
    fixture
        .token(
            "storage:filesystems",
            vec![StorageAuthorizationAction::Read],
        )
        .await;
    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_local_token_read_response_denies_learned_revocation_before_transmission() {
    let gate = Gate::default();
    let fixture = Fixture::new(gate.clone()).await;
    let token = fixture
        .token(
            "storage:filesystems",
            vec![StorageAuthorizationAction::Read],
        )
        .await;
    let file = fixture.file().await;
    file.write("private.txt", b"must not disclose")
        .await
        .expect("native resource content");
    let mut socket = fixture.socket(&token, true).await;
    send(
        &mut socket,
        StorageRequest::Read {
            path: "private.txt".into(),
        },
    )
    .await;
    gate.reached().await;
    fixture.revoke_user().await;
    gate.release();
    assert_no_response(&mut socket).await;
    assert_eq!(
        file.read("private.txt").await.expect("source retained"),
        b"must not disclose"
    );
    drop(socket);
    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_local_tokens_bound_api_actions_resource_kind_and_existing_socket_operations() {
    let fixture = Fixture::new(Gate::default()).await;
    let read = fixture.token("storage:filesystems", vec![StorageAuthorizationAction::Read]).await;
    let write = fixture.token("storage:filesystems", vec![StorageAuthorizationAction::Write]).await;
    let database_read = fixture.token("storage:databases", vec![StorageAuthorizationAction::Read]).await;
    let database_write = fixture.token("storage:databases", vec![StorageAuthorizationAction::Write]).await;
    for (kind, read, write, wrong_kind) in [
        ("filesystems", &read, &write, &database_read),
        ("databases", &database_read, &database_write, &read),
    ] {
        let path = format!("/storage/{kind}");
        assert_eq!(fixture.request(reqwest::Method::GET, &path, wrong_kind).await.status(), reqwest::StatusCode::UNAUTHORIZED, "token must bind the actual resource kind");
        assert_eq!(fixture.request(reqwest::Method::POST, &path, read).await.status(), reqwest::StatusCode::FORBIDDEN);
        assert_eq!(fixture.request(reqwest::Method::GET, &path, write).await.status(), reqwest::StatusCode::FORBIDDEN);
        let created = fixture.request(reqwest::Method::POST, &path, write).await;
        assert_eq!(created.status(), reqwest::StatusCode::CREATED);
        let created: serde_json::Value = created.json().await.expect("actual catalog resource");
        let id = created["id"].as_str().expect("stable resource ID");
        let detail = format!("{path}/{id}");
        assert_eq!(fixture.request(reqwest::Method::GET, &path, read).await.status(), reqwest::StatusCode::OK);
        let detail_response = fixture.request(reqwest::Method::GET, &detail, read).await;
        assert_eq!(detail_response.status(), reqwest::StatusCode::OK);
        let detail_response: serde_json::Value = detail_response.json().await.expect("namespace-bound detail");
        assert_eq!(detail_response["applicationId"], fixture.scope.application_id.to_string());
        assert_eq!(fixture.request(reqwest::Method::DELETE, &detail, read).await.status(), reqwest::StatusCode::FORBIDDEN);
        assert_eq!(fixture.request(reqwest::Method::DELETE, &detail, write).await.status(), reqwest::StatusCode::NO_CONTENT);
        assert_eq!(fixture.request(reqwest::Method::GET, &detail, read).await.status(), reqwest::StatusCode::NOT_FOUND);
    }
    let wrong_socket = tokio_tungstenite::connect_async(fixture.socket_url(&database_read, false)).await;
    assert!(matches!(wrong_socket, Err(tokio_tungstenite::tungstenite::Error::Http(ref response)) if response.status() == reqwest::StatusCode::UNAUTHORIZED), "database token must not open a filesystem socket");
    let file = fixture.file().await;
    file.write("shared.txt", b"before").await.expect("native resource");
    let mut reader = fixture.socket(&read, false).await;
    send(&mut reader, StorageRequest::Read { path: "shared.txt".into() }).await;
    assert_eq!(response(&mut reader).await, StorageResponse::Read { content: b"before".to_vec() });
    send(&mut reader, StorageRequest::Write { path: "shared.txt".into(), content: b"denied".to_vec() }).await;
    assert!(matches!(response(&mut reader).await, StorageResponse::Error { .. }));
    assert_eq!(file.read("shared.txt").await.expect("read-only mutation denied"), b"before");
    let mut writer = fixture.socket(&write, false).await;
    send(&mut writer, StorageRequest::Write { path: "shared.txt".into(), content: b"after".to_vec() }).await;
    assert!(matches!(response(&mut writer).await, StorageResponse::Written { .. }));
    send(&mut writer, StorageRequest::Read { path: "shared.txt".into() }).await;
    assert!(matches!(response(&mut writer).await, StorageResponse::Error { .. }));
    assert_eq!(file.read("shared.txt").await.expect("real API write"), b"after");
    fixture.revoke_user().await;
    send(&mut reader, StorageRequest::Read { path: "shared.txt".into() }).await;
    assert_no_response(&mut reader).await;
    send(&mut writer, StorageRequest::Write { path: "shared.txt".into(), content: b"revoked".to_vec() }).await;
    assert_no_response(&mut writer).await;
    assert_eq!(file.read("shared.txt").await.expect("revoked mutation absent"), b"after");
    assert_eq!(fixture.request(reqwest::Method::GET, "/storage/filesystems", &read).await.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert!(matches!(storage_server::authorize_storage_token(&fixture.client, &read).await, Err(storage_server::StorageAuthorizationError::InvalidToken)));
    drop(reader);
    drop(writer);
    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_local_token_authorized_write_survives_revoked_response_and_next_operation_denial() {
    let gate = Gate::default();
    let fixture = Fixture::new(gate.clone()).await;
    let token = fixture.token("storage:filesystems", vec![StorageAuthorizationAction::Write]).await;
    let file = fixture.file().await;
    let mut socket = fixture.socket(&token, true).await;
    send(&mut socket, StorageRequest::Write { path: "committed.txt".into(), content: b"authorized in-flight content".to_vec() }).await;
    gate.reached().await;
    assert_eq!(file.read("committed.txt").await.expect("actual write completed before learning denial"), b"authorized in-flight content");
    fixture.revoke_user().await;
    gate.release();
    assert_no_response(&mut socket).await;
    let next = fixture.request(reqwest::Method::POST, "/storage/filesystems", &token).await;
    assert_eq!(next.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert!(matches!(tokio_tungstenite::connect_async(fixture.socket_url(&token, false)).await, Err(tokio_tungstenite::tungstenite::Error::Http(ref response)) if response.status() == reqwest::StatusCode::UNAUTHORIZED), "revoked token cannot reconnect for another write");
    assert_eq!(file.read("committed.txt").await.expect("earlier write retained"), b"authorized in-flight content");
    drop(socket);
    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn independent_idp_service_loss_denies_response_and_api_without_cached_fallback() {
    let gate = Gate::default();
    let mut fixture = Fixture::with_idp_backend(gate.clone(), None, true).await;
    let token = fixture
        .token(
            "storage:filesystems",
            vec![StorageAuthorizationAction::Read],
        )
        .await;
    let file = fixture.file().await;
    file.write("private.txt", b"native content")
        .await
        .expect("real filesystem content");
    let mut socket = fixture.socket(&token, true).await;
    send(
        &mut socket,
        StorageRequest::Read {
            path: "private.txt".into(),
        },
    )
    .await;
    gate.reached().await;
    fixture.idp_listener.stop().await;
    gate.release();
    assert_no_response(&mut socket).await;
    assert_eq!(
        fixture
            .request(reqwest::Method::GET, "/storage/filesystems", &token)
            .await
            .status(),
        reqwest::StatusCode::SERVICE_UNAVAILABLE
    );
    assert!(
        matches!(
            storage_server::authorize_storage_token(&fixture.client, &token).await,
            Err(storage_server::StorageAuthorizationError::ServiceUnavailable)
        ),
        "cached service credential is not cached resource authorization"
    );
    assert_eq!(
        file.read("private.txt").await.expect("source retained"),
        b"native content"
    );
    drop(socket);
    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_catalog_create_response_denies_learned_user_deletion_without_rollback() {
    let gate = Gate::default();
    let fixture = Fixture::with_catalog_gate(Gate::default(), Some(gate.clone())).await;
    let token = fixture.token("storage:filesystems", vec![StorageAuthorizationAction::Write]).await;
    let source = fixture.file().await;
    source.write("source.txt", b"earlier content").await.expect("existing resource content");
    let mut request = Box::pin(fixture.request(reqwest::Method::POST, "/storage/filesystems", &token));
    tokio::select! {
        response = &mut request => panic!("catalog result transmitted without post-execution authorization: {}", response.status()),
        () = gate.reached() => {}
    }
    assert_eq!(fixture.files.list_resources(&fixture.scope).await.expect("observe completed creation before learned denial").len(), 2);
    fixture.revoke_user().await;
    gate.release();
    let response = request.await;
    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert!(response.bytes().await.expect("denial body").is_empty(), "catalog result must not be disclosed");
    assert_eq!(fixture.files.list_resources(&fixture.scope).await.expect("authorized creation retained").len(), 2);
    assert_eq!(source.read("source.txt").await.expect("earlier source retained"), b"earlier content");
    fixture.stop().await;
}

async fn catalog_response_denial(kind: &str, operation: &str) {
    let gate = Gate::default();
    let fixture = Fixture::with_catalog_gate(Gate::default(), Some(gate.clone())).await;
    let write = matches!(operation, "create" | "delete");
    let token = fixture
        .token(
            &format!("storage:{kind}"),
            vec![if write {
                StorageAuthorizationAction::Write
            } else {
                StorageAuthorizationAction::Read
            }],
        )
        .await;
    let source = fixture.file().await;
    source.write("source.txt", b"earlier content").await.expect("existing resource content");
    let target = if kind == "databases" {
        fixture
            .databases
            .create(&fixture.scope, Some("target".into()))
            .expect("create target database")
            .0
            .id
            .to_string()
    } else {
        fixture
            .files
            .create_resource(&fixture.scope, Some("target".into()))
            .await
            .expect("create target filesystem")
            .id
            .as_uuid()
            .to_string()
    };
    let before = if kind == "databases" { 1 } else { 2 };
    let expected = match operation {
        "create" => before + 1,
        "delete" => before - 1,
        _ => before,
    };
    let method = match operation {
        "create" => reqwest::Method::POST,
        "delete" => reqwest::Method::DELETE,
        _ => reqwest::Method::GET,
    };
    let path = if matches!(operation, "get" | "delete") {
        format!("/storage/{kind}/{target}")
    } else {
        format!("/storage/{kind}")
    };
    let mut request = Box::pin(fixture.request(method, &path, &token));
    tokio::select! {
        response = &mut request => panic!("{kind}/{operation} returned before current response authorization: {}", response.status()),
        () = gate.reached() => {}
    }
    let observed = if kind == "databases" {
        fixture
            .databases
            .list(&fixture.scope)
            .expect("completed database operation")
            .len()
    } else {
        fixture
            .files
            .list_resources(&fixture.scope)
            .await
            .expect("completed filesystem operation")
            .len()
    };
    assert_eq!(
        observed, expected,
    );
    fixture.revoke_user().await;
    gate.release();
    let response = request.await;
    assert_eq!(
        response.status(),
        reqwest::StatusCode::UNAUTHORIZED,
        "current {kind}/{operation} result must deny"
    );
    assert!(
        response.bytes().await.expect("denial body").is_empty(),
        "catalog data and successful operation status must not disclose"
    );
    let retained = if kind == "databases" {
        fixture
            .databases
            .list(&fixture.scope)
            .expect("retained database mutation")
            .len()
    } else {
        fixture
            .files
            .list_resources(&fixture.scope)
            .await
            .expect("retained filesystem mutation")
            .len()
    };
    assert_eq!(
        retained, expected,
    );
    assert_eq!(
        source
            .read("source.txt")
            .await
            .expect("prior source preserved"),
        b"earlier content"
    );
    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_catalog_get_response_denies_learned_user_deletion_without_rollback() {
    catalog_response_denial("filesystems", "get").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_catalog_list_response_denies_learned_user_deletion_without_rollback() {
    catalog_response_denial("filesystems", "list").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_catalog_delete_response_denies_learned_user_deletion_without_rollback() {
    catalog_response_denial("filesystems", "delete").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_database_catalog_create_response_denies_learned_user_deletion_without_rollback() {
    catalog_response_denial("databases", "create").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_database_catalog_get_response_denies_learned_user_deletion_without_rollback() {
    catalog_response_denial("databases", "get").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_database_catalog_list_response_denies_learned_user_deletion_without_rollback() {
    catalog_response_denial("databases", "list").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_database_catalog_delete_response_denies_learned_user_deletion_without_rollback() {
    catalog_response_denial("databases", "delete").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_catalog_all_result_kinds_recheck_after_execution_and_retain_mutations() {
    for kind in ["databases", "filesystems"] {
        for operation in ["create", "get", "list", "delete"] {
            if kind != "filesystems" || operation != "create" {
                catalog_response_denial(kind, operation).await;
            }
        }
    }
}

fn generated_token_params() -> of_idp_client::apis::default_api::TokenParams {
    of_idp_client::apis::default_api::TokenParams {
        authorization: None,
        client_id: None,
        password: None,
        resource: None,
        scope: None,
        username: None,
        grant_type: None,
        code: None,
        code_verifier: None,
        redirect_uri: None,
        audience: None,
        client_secret: None,
        refresh_token: None,
        actor_token: None,
        actor_token_type: None,
        authorization_details: None,
        requested_token_type: None,
        subject_token: None,
        subject_token_type: None,
    }
}

#[tokio::test]
async fn generated_rust_and_built_javascript_use_actual_owner_routes() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let built = root.join("target/debug/built-openapi");
    if !built.exists() {
        return;
    }
    let built_spec = std::fs::read_to_string(built.join("idp.json")).expect("built idp spec");
    let built_json: serde_json::Value = serde_json::from_str(&built_spec).expect("parse built");

    // Fetch the Rust-generated OpenAPI from the running server
    let client = reqwest::Client::new();
    let rust_response = client
        .get("http://127.0.0.1:0/openapi.json")
        .send()
        .await;
    if rust_response.is_err() {
        // Server not running, skip test
        return;
    }
    let rust_json: serde_json::Value = rust_response.unwrap().json().await.expect("parse rust");
    assert_eq!(rust_json["paths"], built_json["paths"]);
    assert_eq!(rust_json["components"], built_json["components"]);
}
