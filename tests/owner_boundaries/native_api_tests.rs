use std::time::Duration;

use model::contract::StorageAuthorizationAction;
use storage_model::{StorageRequest, StorageResponse};
use tokio_util::sync::CancellationToken;

use crate::support::native::{
    Fixture, Gate, Listener, StorageNamespace, assert_no_response, registration, response, send,
};

#[path = "native_api_qualification_tests.rs"]
mod qualification_tests;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_owner_cutover_captures_initial_secrets_and_reuses_reopened_owner() {
    tokio::time::timeout(Duration::from_secs(240), async {
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
    })
    .await
    .expect("native owner cutover completes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_local_token_read_response_denies_learned_revocation_before_transmission() {
    tokio::time::timeout(Duration::from_secs(240), async {
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
    })
    .await
    .expect("native API denial completes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_local_tokens_bound_api_actions_resource_kind_and_existing_socket_operations() {
    tokio::time::timeout(Duration::from_secs(240), async {
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
        drop(reader); drop(writer); fixture.stop().await;
    }).await.expect("native local API actions complete");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_local_token_authorized_write_survives_revoked_response_and_next_operation_denial() {
    tokio::time::timeout(Duration::from_secs(240), async {
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
        drop(socket); fixture.stop().await;
    }).await.expect("native in-flight API write completes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn independent_idp_service_loss_denies_response_and_api_without_cached_fallback() {
    tokio::time::timeout(Duration::from_secs(240), async {
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
    })
    .await
    .expect("failed local owner evaluation denies safely");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_catalog_create_response_denies_learned_user_deletion_without_rollback() {
    tokio::time::timeout(Duration::from_secs(240), async {
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
    }).await.expect("native catalog response boundary completes");
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
    source
        .write("source.txt", b"earlier source content")
        .await
        .expect("prior content");
    let target = if kind == "databases" {
        fixture
            .databases
            .create(&fixture.scope, Some("target".into()))
            .expect("native database catalog")
            .0
            .id
            .to_string()
    } else {
        fixture
            .files
            .create_resource(&fixture.scope, Some("target".into()))
            .await
            .expect("native filesystem catalog")
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
        "actual {kind}/{operation} completed before learning denial"
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
        "response denial must not roll back authorized {kind}/{operation}"
    );
    assert_eq!(
        source
            .read("source.txt")
            .await
            .expect("prior source preserved"),
        b"earlier source content"
    );
    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_catalog_all_result_kinds_recheck_after_execution_and_retain_mutations() {
    tokio::time::timeout(Duration::from_secs(600), async {
        for kind in ["databases", "filesystems"] {
            for operation in ["create", "get", "list", "delete"] {
                if kind != "filesystems" || operation != "create" {
                    catalog_response_denial(kind, operation).await;
                }
            }
        }
    })
    .await
    .expect("native catalog result matrix completes");
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
    use of_idp_client::apis::{configuration::Configuration, default_api};

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for export in [
        "idp-client/esm/index.js",
        "management-client/esm/index.js",
        "device-client/esm/index.js",
        "storage-client/esm/index.js",
        "storage-client/esm/generated/index.js",
    ] {
        assert!(
            root.join("packages").join(export).is_file(),
            "missing built SDK export {export}; run `pnpm install --frozen-lockfile --ignore-scripts && pnpm sdk:build` before Cargo tests (or use `just test`)"
        );
    }
    let fixture = Fixture::new(Gate::default()).await;
    let document: serde_json::Value = fixture
        .http
        .get(format!("{}/openapi.json", fixture.idp_listener.base))
        .send()
        .await
        .expect("runtime IdP discovery")
        .error_for_status()
        .expect("public contract")
        .json()
        .await
        .expect("served IdP contract");
    let mut expected: serde_json::Value =
        serde_json::from_str(include_str!("../../openapi/idp.json"))
            .expect("exported IdP contract");
    expected["servers"] = document["servers"].clone();
    assert_eq!(document, expected, "runtime IdP contract matches export");
    let configuration = Configuration {
        base_path: fixture.idp_listener.base.clone(),
        client: fixture.http.clone(),
        ..Configuration::default()
    };
    let mut params = generated_token_params();
    params.authorization = Some("Basic aHVtYW46aHVtYW4tc2VjcmV0".into());
    params.client_id = Some("human".into());
    params.username = Some("administrator".into());
    params.password = Some("valid-password".into());
    params.scope = Some("openid".into());
    params.grant_type = Some("password".into());
    let token = default_api::token(&configuration, params)
        .await
        .expect("generated OAuth form accepted by actual handler");
    assert!(!token.access_token.is_empty());
    let jwks = default_api::jwks(&configuration)
        .await
        .expect("generated JWKS decodes actual signing key variants");
    assert!(
        !jwks.keys.is_empty(),
        "actual IdP publishes its signing key"
    );
    let denied = of_idp_client::apis::super_routes_oauth2_introspect_api::introspect(
        &configuration,
        of_idp_client::apis::super_routes_oauth2_introspect_api::IntrospectParams {
            introspection_request: of_idp_client::models::IntrospectionRequest {
                token: token.access_token.clone(),
                token_type_hint: None,
            },
        },
    )
    .await
    .expect_err("successful decoding cannot authorize an unauthenticated caller");
    assert!(matches!(
        denied,
        of_idp_client::apis::Error::ResponseError(_)
    ));
    let mut invalid = generated_token_params();
    invalid.grant_type = Some("client_credentials".into());
    invalid.client_id = Some("not-registered".into());
    invalid.client_secret = Some("not-a-secret".into());
    assert!(default_api::token(&configuration, invalid).await.is_err());
    let storage = of_storage_client::apis::configuration::Configuration {
        base_path: format!("{}/storage", fixture.storage_listener.base),
        client: fixture.http.clone(),
        ..Default::default()
    };
    assert!(
        of_storage_client::apis::resources_api::list_file_systems(&storage)
            .await
            .is_err(),
        "actual catalog requires authorization"
    );
    let setup = api::IndependentSetupHost::prepare(
        "idp",
        &fixture._directory.0.join("independent"),
        "127.0.0.1:3000".parse().expect("owner address"),
        "https://localhost:5173",
        "",
        false,
    )
    .expect("isolated owner Setup")
    .expect("fresh owner");
    let (host, _commands) = setup
        .into_create(CancellationToken::new())
        .expect("Setup host");
    let mut independent = Listener::start(host.router()).await;
    let setup_config = of_idp_client::apis::configuration::Configuration {
        base_path: independent.base.clone(),
        client: fixture.http.clone(),
        ..Default::default()
    };
    assert!(
        !of_idp_client::apis::independent_setup_api::independent_setup_status(&setup_config)
            .await
            .expect("actual independent status")
            .ready
    );
    assert!(
        of_idp_client::apis::independent_setup_api::independent_setup_recover(&setup_config)
            .await
            .is_err(),
        "discovery does not grant Setup admission"
    );
    let challenge = of_idp_client::models::Challenge {
        challenge: "a".repeat(64),
        operation: Box::default(),
        recipient: "management".into(),
        registration: Some(serde_json::json!({"client_id":"unregistered"})),
    };
    assert!(
        of_idp_client::apis::independent_setup_api::independent_setup_challenge(
            &setup_config,
            of_idp_client::apis::independent_setup_api::IndependentSetupChallengeParams {
                challenge
            }
        )
        .await
        .is_err(),
        "published worker handoff remains operation-bound"
    );
    let status = tokio::task::spawn_blocking({
        let idp = fixture.idp_listener.base.clone();
        let storage = fixture.storage_listener.base.clone();
        let independent = independent.base.clone();
        move || {
            std::process::Command::new("node")
                .arg(root.join("scripts/qualify-built-openapi.mjs"))
                .arg(idp)
                .arg(format!("{storage}/storage"))
                .arg(independent)
                .status()
                .expect("run built JavaScript package consumers")
        }
    })
    .await
    .expect("JavaScript consumer task");
    assert!(
        status.success(),
        "built exported SDK operations against actual routers"
    );
    independent.stop().await;
    fixture.stop().await;
}
