use std::time::Duration;

use device_service::replica::DbSelectionPolicyRepo;
use idp_model::{
    contract::{GrantType, TokenEndpointAuthMethod},
    model::Id,
};
use idp_service::repo::ClientRepo;
use model::contract::StorageAuthorizationAction;
use storage_model::{StorageRequest, StorageResponse};

use super::{Fixture, Gate, registration};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bundled_storage_direct_idp_checks_current_bearer_registration_and_signer_offline() {
    tokio::time::timeout(Duration::from_secs(240), async {
        let mut fixture = Fixture::new(Gate::default()).await;
        let token = fixture
            .token(
                "storage:filesystems",
                vec![StorageAuthorizationAction::Read],
            )
            .await;
        let database_token = fixture
            .token("storage:databases", vec![StorageAuthorizationAction::Read])
            .await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("test clock")
            .as_secs() as i64;
        let signing_key = fixture
            .keys
            .load_node_signer(&fixture.binding, &fixture.issuer, now)
            .await
            .expect("real current Node signer");
        let (_, claims) = idp_service::oauth2::decode_jwt::<serde_json::Value>(&token)
            .expect("issued bearer claims");
        fixture.idp_listener.stop().await;

        let current = fixture
            .client
            .introspect(&token)
            .await
            .expect("no IdP HTTP listener required");
        assert_eq!(current.claims.sub, fixture.owner.to_string());
        assert_eq!(
            current.application_id,
            fixture.scope.application_id.to_string()
        );
        fixture
            .client
            .introspect(&database_token)
            .await
            .expect("database audience permitted");
        assert!(
            fixture
                .control
                .approved_device_endpoints()
                .await
                .expect("current owner trust")
                .contains(&fixture.identity.endpoint().id().to_string())
        );
        assert!(matches!(
            fixture.client.introspect("invalid").await,
            Err(storage_server::IdpIntrospectionError::InvalidToken)
        ));
        for (field, value) in [
            ("sub", serde_json::json!(Id::now_v7().to_string())),
            ("principal_type", serde_json::json!("client")),
            ("aud", serde_json::json!("idp-services")),
            ("aud", serde_json::json!("storage:unknown")),
            ("exp", serde_json::json!(0)),
        ] {
            let mut invalid = claims.clone();
            invalid[field] = value;
            let signed = idp_service::oauth2::encode_jwt(&signing_key, &invalid)
                .expect("genuinely signed invalid actor/claims");
            assert!(
                matches!(
                    fixture.client.introspect(&signed).await,
                    Err(storage_server::IdpIntrospectionError::InvalidToken)
                ),
                "accepted invalid {field}"
            );
        }
        let mut human = fixture
            .clients
            .find_client_by_client_id("human")
            .await
            .expect("current registration")
            .expect("human Client");
        let scopes = human.allowed_scopes.clone();
        human.allowed_scopes.clear();
        fixture
            .clients
            .update_client(human.clone())
            .await
            .expect("learn scope withdrawal");
        assert!(
            fixture.client.introspect(&token).await.is_err(),
            "registration is rechecked"
        );
        human.allowed_scopes = scopes;
        fixture
            .clients
            .update_client(human)
            .await
            .expect("restore valid registration");
        fixture
            .client
            .introspect(&token)
            .await
            .expect("current registration restored");
        idp_service::replica::DbReplicaSignerRepo::new(fixture.idp.client())
            .revoke(fixture.binding.member_id, &fixture.issuer, now)
            .await
            .expect("learn signer revocation");
        assert!(
            fixture.client.introspect(&token).await.is_err(),
            "signer trust is never cached"
        );
        assert!(
            fixture
                .control
                .approved_device_endpoints()
                .await
                .expect("Device trust does not depend on signer trust")
                .contains(&fixture.identity.endpoint().id().to_string())
        );
        fixture.stop().await;
    })
    .await
    .expect("bounded direct IdP owner qualification");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bundled_storage_direct_idp_preserves_real_local_owner_gate() {
    tokio::time::timeout(Duration::from_secs(240), async {
        let fixture = Fixture::new(Gate::default()).await;
        set_device_allowed(&fixture, true).await;
        let copies = std::sync::Arc::new(std::sync::OnceLock::new());
        assert!(
            copies
                .set(storage_server::StorageCopyRetirement::new(
                    fixture.databases.clone(),
                    fixture.files.clone()
                ))
                .is_ok()
        );
        let policy = crate::support::replication::owner_replication_authorizer(
            device_server::DeviceAuthority::new(fixture.devices.clone(), fixture.control.clone()),
            fixture.identity.endpoint().id(),
            copies.clone(),
        );
        let client = fixture
            .client
            .clone()
            .with_local_device_policy(fixture.identity.endpoint().id(), policy);
        let token = fixture
            .token(
                "storage:filesystems",
                vec![StorageAuthorizationAction::Read],
            )
            .await;
        client
            .introspect(&token)
            .await
            .expect("current local owner is allowed");
        set_device_allowed(&fixture, false).await;
        fixture
            .client
            .introspect(&token)
            .await
            .expect("bearer itself remains valid");
        assert!(
            matches!(
                client.introspect(&token).await,
                Err(storage_server::IdpIntrospectionError::InvalidToken)
            ),
            "local Device policy is still enforced"
        );
        fixture.revoke_user().await;
        assert!(
            fixture.client.introspect(&token).await.is_err(),
            "current principal deletion denies"
        );
        fixture.stop().await;
    })
    .await
    .expect("bounded direct owner gate qualification");
}

async fn reassign_application(fixture: &Fixture) -> Id {
    let other = fixture
        .clients
        .create_client(registration(
            "other-application",
            vec![GrantType::Password, GrantType::TokenExchange],
            vec!["openid".into()],
            vec!["storage:filesystems".into(), "storage:databases".into()],
            TokenEndpointAuthMethod::ClientSecretBasic,
        ))
        .await
        .expect("real distinct Application");
    assert_ne!(other.application_id, fixture.scope.application_id);
    let mut human = fixture
        .clients
        .find_client_by_client_id("human")
        .await
        .expect("current Client")
        .expect("human Client exists");
    human.application_id = other.application_id;
    fixture
        .clients
        .update_client(human)
        .await
        .expect("learn Client Application reassignment");
    let current = fixture
        .clients
        .find_client_by_client_id("human")
        .await
        .expect("reassigned Client read")
        .expect("Client retained");
    assert_eq!(current.application_id, other.application_id);
    other.application_id
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_catalog_response_denies_learned_application_reassignment_without_rollback() {
    tokio::time::timeout(Duration::from_secs(240), async {
        for kind in ["databases", "filesystems"] {
        let gate = Gate::default();
        let fixture = Fixture::with_catalog_gate(Gate::default(), Some(gate.clone())).await;
        let token = fixture.token(&format!("storage:{kind}"), vec![StorageAuthorizationAction::Write]).await;
        let path = format!("/storage/{kind}");
        let mut request = Box::pin(fixture.request(reqwest::Method::POST, &path, &token));
        tokio::select! {
            response = &mut request => panic!("catalog result returned before response gate: {}", response.status()),
            () = gate.reached() => {}
        }
        let expected = if kind == "databases" { 1 } else { 2 };
        assert_eq!(catalog_count(&fixture, kind, &fixture.scope).await, expected);
        let other = reassign_application(&fixture).await;
        gate.release();
        let response = request.await;

        assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED, "old namespace result cannot be disclosed under current Application");
        assert!(response.bytes().await.expect("denial body").is_empty());
        let current = storage_server::authorize_storage_token(&fixture.client, &token).await.expect("same bearer remains valid in current namespace");
        assert_eq!(current.subject, fixture.scope.user_sub);
        assert_eq!(current.application_id, other.to_string());
        assert_eq!(catalog_count(&fixture, kind, &fixture.scope).await, expected);
        assert_eq!(catalog_count(&fixture, kind, &super::StorageNamespace { user_sub: fixture.scope.user_sub.clone(), application_id: other }).await, 0);
        fixture.stop().await;
        }
    }).await.expect("native namespace interleaving completes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_catalog_paused_local_owner_rechecks_before_response_without_rollback() {
    tokio::time::timeout(Duration::from_secs(120), async {
        let gate = Gate::default();
        let fixture = Fixture::with_catalog_gate(Gate::default(), Some(gate.clone())).await;
        let token = fixture.token("storage:filesystems", vec![StorageAuthorizationAction::Write]).await;

        let mut request = Box::pin(fixture.request(reqwest::Method::POST, "/storage/filesystems", &token));
        tokio::select! {
            response = &mut request => panic!("result returned before real response-time introspection: {}", response.status()),
            () = gate.reached() => {}
        }
        assert_eq!(catalog_count(&fixture, "filesystems", &fixture.scope).await, 2, "authorized mutation completed before the paused local evaluation");
        fixture.revoke_user().await;
        gate.release();
        let response = request.await;
        assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
        assert!(response.bytes().await.expect("safe denial body").is_empty());
        assert!(matches!(storage_server::authorize_storage_token(&fixture.client, &token).await, Err(storage_server::StorageAuthorizationError::InvalidToken)));
        assert_eq!(catalog_count(&fixture, "filesystems", &fixture.scope).await, 2);
        fixture.stop().await;
    }).await.expect("native direct owner recheck completes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn independent_catalog_paused_introspection_timeout_returns_503_without_rollback() {
    tokio::time::timeout(Duration::from_secs(120), async {
        let gate = Gate::default();
        let fixture = Fixture::with_idp_backend(Gate::default(), Some(gate.clone()), true).await;
        let token = fixture.token("storage:filesystems", vec![StorageAuthorizationAction::Write]).await;
        let started = std::time::Instant::now();
        let mut request = Box::pin(fixture.request(reqwest::Method::POST, "/storage/filesystems", &token));
        tokio::select! {
            response = &mut request => panic!("result returned before response-time introspection: {}", response.status()),
            () = gate.reached() => {}
        }
        assert_eq!(catalog_count(&fixture, "filesystems", &fixture.scope).await, 2);
        let response = request.await;
        assert_eq!(response.status(), reqwest::StatusCode::SERVICE_UNAVAILABLE);
        assert!(response.bytes().await.expect("safe denial body").is_empty());
        assert!(started.elapsed() >= Duration::from_secs(10), "existing HTTP request deadline, not token/policy denial");
        gate.release();
        storage_server::authorize_storage_token(&fixture.client, &token).await
            .expect("same bearer remains valid after transport recovery");
        assert_eq!(catalog_count(&fixture, "filesystems", &fixture.scope).await, 2);
        fixture.stop().await;
    }).await.expect("independent introspection deadline completes");
}

async fn catalog_count(fixture: &Fixture, kind: &str, scope: &super::StorageNamespace) -> usize {
    if kind == "databases" {
        fixture
            .databases
            .list(scope)
            .expect("native database catalog")
            .len()
    } else {
        fixture
            .files
            .list_resources(scope)
            .await
            .expect("native filesystem catalog")
            .len()
    }
}

async fn set_device_allowed(fixture: &Fixture, allowed: bool) {
    assert!(
        DbSelectionPolicyRepo::new(fixture.devices.clone().client())
            .set_admin_allowed_prevalidated(
                fixture.device.device_id,
                &fixture.device.owner_subject,
                allowed
            )
            .await
            .expect("persist real owner-bound Management restriction")
    );
    let (current, selected) =
        device_server::DeviceAuthority::new(fixture.devices.clone(), fixture.control.clone())
            .replication_policy(&fixture.device)
            .await
            .expect("current local Management evaluation");
    assert_eq!(current, allowed);
    assert!(
        selected.is_empty(),
        "bearer API discovery does not require Device selection"
    );
}

async fn catalog_device_restriction(kind: &str, operation: &str) {
    let gate = Gate::default();
    let fixture = Fixture::with_catalog_gate(Gate::default(), Some(gate.clone())).await;
    set_device_allowed(&fixture, true).await;
    let token = fixture
        .token(
            &format!("storage:{kind}"),
            vec![
                StorageAuthorizationAction::Read,
                StorageAuthorizationAction::Write,
            ],
        )
        .await;
    let target = if kind == "databases" {
        fixture
            .databases
            .create(&fixture.scope, Some("target".into()))
            .expect("database catalog target")
            .0
            .id
            .to_string()
    } else {
        fixture.resource.as_uuid().to_string()
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
        response = &mut request => panic!("{kind}/{operation} returned before response gate: {}", response.status()),
        () = gate.reached() => {}
    }
    let expected_count = match operation {
        "create" => 2,
        "delete" => 0,
        _ => 1,
    };
    let completed_count = catalog_count(&fixture, kind, &fixture.scope).await;
    assert_eq!(completed_count, expected_count);
    set_device_allowed(&fixture, false).await;
    gate.release();
    let response = request.await;
    let expected_status = match operation {
        "create" => reqwest::StatusCode::CREATED,
        "delete" => reqwest::StatusCode::NO_CONTENT,
        _ => reqwest::StatusCode::OK,
    };
    assert_eq!(
        response.status(),
        expected_status,
        "Device replication restriction is not Storage bearer revocation"
    );
    if operation != "delete" {
        let body: serde_json::Value = response.json().await.expect("real catalog response");
        if operation == "list" {
            assert_eq!(body.as_array().expect("catalog list").len(), 1);
        } else if operation == "get" {
            assert_eq!(body["id"], target);
        } else {
            assert!(body["id"].as_str().is_some());
        }
    }
    set_device_allowed(&fixture, false).await;
    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_catalog_device_restrictions_do_not_revoke_bearer_results() {
    tokio::time::timeout(Duration::from_secs(600), async {
        for kind in ["databases", "filesystems"] {
            for operation in ["create", "get", "list", "delete"] {
                catalog_device_restriction(kind, operation).await;
            }
        }
    })
    .await
    .expect("native Management/catalog response matrix completes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_socket_device_restriction_preserves_bearer_response_and_next_operation() {
    tokio::time::timeout(Duration::from_secs(240), async {
        let gate = Gate::default();
        let fixture = Fixture::new(gate.clone()).await;
        set_device_allowed(&fixture, true).await;
        let token = fixture
            .token(
                "storage:filesystems",
                vec![
                    StorageAuthorizationAction::Read,
                    StorageAuthorizationAction::Write,
                ],
            )
            .await;
        let file = fixture.file().await;
        file.write("source.txt", b"source content")
            .await
            .expect("native source content");
        let mut socket = fixture.socket(&token, true).await;
        super::send(
            &mut socket,
            StorageRequest::Read {
                path: "source.txt".into(),
            },
        )
        .await;
        gate.reached().await;
        set_device_allowed(&fixture, false).await;
        gate.release();
        assert_eq!(
            super::response(&mut socket).await,
            StorageResponse::Read {
                content: b"source content".to_vec()
            }
        );
        super::send(
            &mut socket,
            StorageRequest::Write {
                path: "committed.txt".into(),
                content: b"bearer-authorized content".to_vec(),
            },
        )
        .await;
        gate.reached().await;
        assert_eq!(
            file.read("committed.txt")
                .await
                .expect("next write committed on same socket"),
            b"bearer-authorized content"
        );
        set_device_allowed(&fixture, false).await;
        gate.release();
        assert!(matches!(
            super::response(&mut socket).await,
            StorageResponse::Written { .. }
        ));
        fixture.revoke_user().await;
        super::send(
            &mut socket,
            StorageRequest::Write {
                path: "denied.txt".into(),
                content: b"must not commit".to_vec(),
            },
        )
        .await;
        super::assert_no_response(&mut socket).await;
        assert!(file.read("denied.txt").await.is_err());
        assert_eq!(
            file.read("committed.txt")
                .await
                .expect("earlier write retained"),
            b"bearer-authorized content"
        );
        drop(socket);
        fixture.stop().await;
    })
    .await
    .expect("native Management/socket policy distinction completes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_socket_response_binds_namespace_even_when_resource_id_exists_in_new_application() {
    tokio::time::timeout(Duration::from_secs(120), async {
        let gate = Gate::default();
        let fixture = Fixture::new(gate.clone()).await;
        let token = fixture
            .token(
                "storage:filesystems",
                vec![
                    StorageAuthorizationAction::Read,
                    StorageAuthorizationAction::Write,
                ],
            )
            .await;
        let original = fixture.file().await;
        original
            .write("retained.txt", b"original namespace secret")
            .await
            .expect("original content");
        let mut idle = fixture.socket(&token, false).await;
        let mut socket = fixture.socket(&token, true).await;
        super::send(
            &mut socket,
            StorageRequest::Read {
                path: "retained.txt".into(),
            },
        )
        .await;
        gate.reached().await;
        let other = reassign_application(&fixture).await;
        let scope = super::StorageNamespace {
            user_sub: fixture.scope.user_sub.clone(),
            application_id: other,
        };
        fixture
            .files
            .register_selected_resource(&scope, fixture.resource)
            .await
            .expect("native resource projection in new namespace");
        let current_file = fixture
            .files
            .open_resource(&scope, fixture.resource)
            .await
            .expect("same ID in genuinely distinct namespace");
        current_file
            .write("retained.txt", b"new namespace content")
            .await
            .expect("distinct namespace content");
        let current = storage_server::authorize_storage_token(&fixture.client, &token)
            .await
            .expect("same signed bearer is valid in new Application");
        assert_eq!(current.application_id, other.to_string());
        gate.release();
        super::assert_no_response(&mut socket).await;
        assert_eq!(
            original
                .read("retained.txt")
                .await
                .expect("source retained"),
            b"original namespace secret"
        );
        super::send(
            &mut idle,
            StorageRequest::Write {
                path: "must-not-rebind.txt".into(),
                content: b"denied".to_vec(),
            },
        )
        .await;
        super::assert_no_response(&mut idle).await;
        assert!(current_file.read("must-not-rebind.txt").await.is_err());
        assert!(original.read("must-not-rebind.txt").await.is_err());
        drop(idle);
        let mut fresh = fixture.socket(&token, false).await;
        super::send(
            &mut fresh,
            StorageRequest::Read {
                path: "retained.txt".into(),
            },
        )
        .await;
        assert_eq!(
            super::response(&mut fresh).await,
            StorageResponse::Read {
                content: b"new namespace content".to_vec()
            }
        );
        drop(fresh);
        drop(socket);
        fixture.stop().await;
    })
    .await
    .expect("native overlapping-ID namespace boundary completes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_socket_response_denies_application_reassignment_and_retains_completed_write() {
    tokio::time::timeout(Duration::from_secs(240), async {
        let gate = Gate::default();
        let fixture = Fixture::new(gate.clone()).await;
        let token = fixture.token("storage:filesystems", vec![StorageAuthorizationAction::Read, StorageAuthorizationAction::Write]).await;
        let file = fixture.file().await;
        let mut socket = fixture.socket(&token, true).await;
        super::send(&mut socket, StorageRequest::Write { path: "committed.txt".into(), content: b"old namespace write".to_vec() }).await;
        gate.reached().await;
        assert_eq!(file.read("committed.txt").await.expect("write completed before reassignment"), b"old namespace write");
        let other = reassign_application(&fixture).await;
        gate.release();
        super::assert_no_response(&mut socket).await;
        let current = storage_server::authorize_storage_token(&fixture.client, &token).await.expect("bearer remains valid after namespace change");
        assert_eq!(current.application_id, other.to_string());
        assert_eq!(current.subject, fixture.scope.user_sub);
        assert!(matches!(tokio_tungstenite::connect_async(fixture.socket_url(&token, false)).await, Err(tokio_tungstenite::tungstenite::Error::Http(ref response)) if response.status() == reqwest::StatusCode::UNAUTHORIZED), "current Application cannot reopen old namespace resource");
        assert_eq!(file.read("committed.txt").await.expect("completed old namespace write retained"), b"old namespace write");
        drop(socket);
        fixture.stop().await;
    }).await.expect("native socket namespace interleaving completes");
}
