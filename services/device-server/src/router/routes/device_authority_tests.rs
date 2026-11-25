use std::slice::from_ref;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
    response::Response,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use db::ofdb::up as initialize_policy;
use device_service::HostedControlPlane;
use device_service::ofdb::DbDeviceRepo;
use device_service::ofdb::{DbSelectionPolicyRepo, up as initialize_devices};
use device_service::{Device, DeviceIdentity, DeviceRepo, DeviceState};
use iroh::{Endpoint, SecretKey, endpoint::presets::Minimal};
use management_service::ManagementService;
use management_service::ofdb::DbPermissionRepo;
use management_service::ofdb::DbRoleRepo;
use of_model::contract::{
    principal_type::PrincipalType, standard_claims::StandardClaims, token_type::TokenType,
    token_use::TokenUse,
};
use of_model::device::contract::device_pairing::device_pairing_approval_payload;
use of_model::device::contract::device_self_revocation::device_self_revocation_payload;
use of_model::device::selection_policy::SelectionPolicy;
use of_model::id::Id;
use of_model::idp::contract::introspection_response::IntrospectionResponse;
use of_model::idp::contract::permission_evaluation::{
    IdentityAction, IdentityResource, PermissionEvaluationRequest, PermissionSubject,
    PermissionTarget,
};
use ofdb_sql::{Database, SqlTranslator};
use tower::ServiceExt;

use crate::runtime::build_router;

fn inspect_actor(token: &str, owner: Id) -> Result<IntrospectionResponse, String> {
    let (principal_type, audience, scopes) = match token {
        "user" | "next-user" | "nil-subject" | "invalid-subject" => {
            (PrincipalType::User, "https://device.example", vec![])
        }
        "wrong-audience" => (PrincipalType::User, "idp-services", vec![]),
        "list" => (
            PrincipalType::Client,
            "storage",
            vec!["management.device.list".into()],
        ),
        "lookup" => (
            PrincipalType::Client,
            "storage",
            vec!["management.device.lookup".into()],
        ),
        "unscoped" => (PrincipalType::Client, "storage", vec!["unrelated".into()]),
        "foreign-list" => (
            PrincipalType::Client,
            "other",
            vec!["management.device.list".into()],
        ),
        _ => return Err("actor token was rejected by IdP".into()),
    };
    Ok(IntrospectionResponse {
        claims: StandardClaims {
            r#type: TokenType::Bearer,
            r#use: TokenUse::Access,
            exp: i64::MAX,
            iat: 0,
            nbf: 0,
            iss: "https://installation.example".into(),
            aud: audience.into(),
            client_id: "test-client".into(),
            sub: match token {
                "nil-subject" => Id::nil().to_string(),
                "invalid-subject" => "not-a-user-id".into(),
                _ => owner.to_string(),
            },
            principal_type,
            resource: None,
            authorization_details: None,
            scope: scopes,
        },
        application_id: Id::now_v7().to_string(),
    })
}

struct Fixture {
    router: Router,
    database: Arc<Database>,
    identity: Arc<Database>,
    devices: Arc<DbDeviceRepo>,
    owner: Id,
    next_owner: Id,
    actor_status: Arc<Mutex<Result<(), &'static str>>>,
    idp_calls: Arc<AtomicUsize>,
}

impl Fixture {
    async fn new() -> Self {
        Self::with_device_identity(None).await
    }

    async fn with_device_identity(device_identity: Option<Arc<DeviceIdentity>>) -> Self {
        let identity = Arc::new(Database::in_memory());
        initialize_policy(&identity.client())
            .await
            .expect("identity policy schema");
        let database = Arc::new(Database::in_memory());
        initialize_devices(&database).await.expect("Device schema");
        let owner = Id::now_v7();
        let next_owner = Id::now_v7();
        let actor_status = Arc::new(Mutex::new(Ok(())));
        let current = Arc::clone(&actor_status);
        let idp_calls = Arc::new(AtomicUsize::new(0));
        let inspections = Arc::clone(&idp_calls);
        let lookups = Arc::clone(&idp_calls);
        let control = HostedControlPlane::new_with_services(
            "http://127.0.0.1:1/idp",
            "http://127.0.0.1:1/storage",
            "https://installation.example",
        )
        .expect("unreachable remote owners")
        .with_local_idp(
            move |token| {
                inspections.fetch_add(1, Ordering::SeqCst);
                async move {
                    inspect_actor(
                        &token,
                        if token == "next-user" {
                            next_owner
                        } else {
                            owner
                        },
                    )
                }
            },
            move |id| {
                lookups.fetch_add(1, Ordering::SeqCst);
                let result = if id == owner {
                    *current.lock().expect("actor status")
                } else if id == next_owner {
                    Ok(())
                } else {
                    Err("User not found")
                };
                async move { result.map_err(str::to_owned) }
            },
        );
        let control = match device_identity {
            Some(identity) => control.with_device_identity(identity),
            None => control,
        };
        let devices = Arc::new(DbDeviceRepo::new(database.client()));
        let router = build_router(
            identity.client(),
            database.client(),
            "https://device.example",
            "/device",
            "storage",
            Arc::new(control),
        )
        .await
        .expect("existing owner router");
        Self {
            router,
            database,
            identity,
            devices,
            owner,
            next_owner,
            actor_status,
            idp_calls,
        }
    }

    async fn call(&self, method: &str, path: &str, token: &str, body: &str) -> Response {
        let mut request = Request::builder()
            .method(method)
            .uri(format!("/device{path}"))
            .header("content-type", "application/json");
        if !token.is_empty() {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        self.router
            .clone()
            .oneshot(request.body(Body::from(body.to_owned())).expect("request"))
            .await
            .expect("Device response")
    }

    fn set_actor_status(&self, status: Result<(), &'static str>) {
        *self.actor_status.lock().expect("actor status") = status;
    }

    async fn assert_unchanged(&self, expected: &Device) {
        let current = self.devices.list().await.expect("Devices");
        assert_eq!(
            current.iter().find(|device| device.id == expected.id),
            Some(expected)
        );
    }

    async fn assert_denied(
        &self,
        method: &str,
        path: &str,
        token: &str,
        body: &str,
        status: StatusCode,
        device: &Device,
    ) {
        assert_eq!(
            self.call(method, path, token, body).await.status(),
            status,
            "{method} {path}: {token}"
        );
        self.assert_unchanged(device).await;
    }

    async fn disable_policy(&self) {
        self.identity
            .client()
            .translate_and_execute("DROP TABLE role_permissions", &SqlTranslator)
            .await
            .expect("make policy evaluation unavailable");
    }

    async fn approved(&self) -> Device {
        self.approved_endpoint(self.owner, SecretKey::generate().public().to_string())
            .await
    }

    async fn approved_endpoint(&self, owner: Id, endpoint: String) -> Device {
        let pending = self
            .devices
            .create(
                owner.to_string(),
                "owned".into(),
                endpoint,
                "address".into(),
                vec![1],
                i64::MAX,
            )
            .await
            .expect("pending Device");
        if pending.state == DeviceState::Approved {
            return pending;
        }
        self.devices
            .approve(pending.id, &[1])
            .await
            .expect("approval")
            .expect("Device")
    }

    async fn pending_pairing(&self, accepting: &Device) -> (Device, String) {
        let key = SecretKey::generate();
        let device = self
            .devices
            .create_pairing(
                "Pending Device".into(),
                key.public().to_string(),
                "address".into(),
                accepting.public_key.clone(),
            )
            .await
            .expect("pending pairing");
        let payload = device_pairing_approval_payload(
            device.id,
            &device.name,
            &device.public_key,
            &device.address,
        );
        (device, pairing_proof(&key, &payload))
    }

    fn management(&self) -> ManagementService<DbPermissionRepo, DbRoleRepo> {
        ManagementService::new(
            DbPermissionRepo::new(self.identity.client()),
            DbRoleRepo::new(self.identity.client()),
        )
    }

    async fn grant(&self, action: IdentityAction) {
        self.grant_in(Id::nil(), action).await;
    }

    async fn grant_in(&self, namespace: Id, action: IdentityAction) {
        self.grant_to(namespace, action, &[self.owner]).await;
    }

    async fn grant_to(&self, namespace: Id, action: IdentityAction, subjects: &[Id]) {
        let service = self.management();
        let role = service
            .create_role(namespace, action.permission(), None)
            .await
            .expect("role");
        let permission = service
            .create_permission(namespace, action.permission(), None)
            .await
            .expect("permission");
        service
            .add_permission_to_role(namespace, role.id, permission.id)
            .await
            .expect("role grant");
        for &subject in subjects {
            service
                .add_role_to_user(namespace, subject, role.id)
                .await
                .expect("User grant");
        }
    }
}

#[tokio::test]
async fn device_endpoint_enumeration_requires_exact_service_authority_and_current_owner() {
    let fixture = Fixture::new().await;
    let device = fixture.approved().await;
    let pending_endpoint = SecretKey::generate().public().to_string();
    let pending = fixture
        .devices
        .create(
            fixture.owner.to_string(),
            "pending".into(),
            pending_endpoint,
            "address".into(),
            vec![1],
            i64::MAX,
        )
        .await
        .expect("second Device awaits approval");
    assert!(
        !fixture
            .call(
                "GET",
                &format!("/devices/identities/{}", pending.id),
                "lookup",
                ""
            )
            .await
            .status()
            .is_success()
    );
    for token in ["user", "lookup", "unscoped", "foreign-list"] {
        assert!(
            !fixture
                .call("GET", "/devices/endpoints", token, "")
                .await
                .status()
                .is_success(),
            "{token}"
        );
    }
    let response = fixture.call("GET", "/devices/endpoints", "list", "").await;
    assert_eq!(response.status(), StatusCode::OK);
    let value: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.expect("body"))
            .expect("JSON");
    assert_eq!(value["endpointIds"], serde_json::json!([device.public_key]));
    assert_eq!(
        fixture
            .call(
                "GET",
                &format!("/devices/identities/{}", device.id),
                "lookup",
                ""
            )
            .await
            .status(),
        StatusCode::OK
    );
    fixture.set_actor_status(Err("inactive User"));
    assert!(
        !fixture
            .call(
                "GET",
                &format!("/devices/identities/{}", device.id),
                "lookup",
                ""
            )
            .await
            .status()
            .is_success()
    );
    fixture.set_actor_status(Ok(()));
    fixture
        .database
        .client()
        .translate_and_execute(
            &format!(
                "UPDATE devices SET revoked_at = 9 WHERE id = CAST('{}' AS UUID)",
                device.id
            ),
            &SqlTranslator,
        )
        .await
        .expect("learn timestamp-only revocation");
    let response = fixture.call("GET", "/devices/endpoints", "list", "").await;
    assert_eq!(response.status(), StatusCode::OK);
    let value: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.expect("body"))
            .expect("JSON");
    assert_eq!(value["endpointIds"], serde_json::json!([]));
}

#[tokio::test]
async fn device_transfer_requires_exact_permission_active_new_owner_and_preserves_cleanup_fence() {
    let fixture = Fixture::new().await;
    let device = fixture.approved().await;
    let policies = DbSelectionPolicyRepo::new(fixture.database.client());
    policies
        .set_prevalidated(SelectionPolicy {
            device_id: device.id,
            owner_subject: fixture.owner.to_string(),
            application_id: None,
            selected_kind: None,
            selected_id: None,
            admin_allowed: true,
        })
        .await
        .expect("old-owner policy");
    let path = format!("/devices/{}/owner", device.id);
    let body = format!(r#"{{"ownerSubject":"{}"}}"#, fixture.next_owner);
    assert_eq!(
        fixture.call("PUT", &path, "user", &body).await.status(),
        StatusCode::FORBIDDEN
    );
    fixture.grant(IdentityAction::DevicesRename).await;
    assert_eq!(
        fixture.call("PUT", &path, "user", &body).await.status(),
        StatusCode::FORBIDDEN
    );
    fixture.grant(IdentityAction::DevicesTransfer).await;
    assert!(
        !fixture
            .call("PUT", &path, "wrong-audience", &body)
            .await
            .status()
            .is_success()
    );
    let unknown = format!(r#"{{"ownerSubject":"{}"}}"#, Id::now_v7());
    assert!(
        !fixture
            .call("PUT", &path, "user", &unknown)
            .await
            .status()
            .is_success()
    );
    assert_eq!(
        fixture.call("PUT", &path, "user", &body).await.status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        fixture
            .devices
            .find_approved_by_public_key(&device.public_key)
            .await
            .expect("current binding")
            .expect("Device")
            .owner_subject,
        fixture.next_owner.to_string()
    );
    assert!(
        !policies
            .reconcile_owner_prevalidated(device.id, &fixture.next_owner.to_string())
            .await
            .expect("transfer cleanup"),
        "new owner cannot use retained old-owner copies before cleanup"
    );
    assert!(
        !fixture
            .call(
                "PUT",
                &path,
                "user",
                &format!(r#"{{"ownerSubject":"{}"}}"#, fixture.owner)
            )
            .await
            .status()
            .is_success()
    );
}

#[tokio::test]
async fn device_enrollment_and_listing_require_user_audience_and_exact_actions() {
    let fixture = Fixture::new().await;
    let endpoint = SecretKey::generate().public().to_string();
    let body = format!(r#"{{"name":"first","publicKey":"{endpoint}","address":"address"}}"#);
    assert_eq!(
        fixture
            .call("POST", "/devices/enrollments", "user", &body)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    fixture.grant(IdentityAction::DevicesEnroll).await;
    assert!(
        !fixture
            .call("POST", "/devices/enrollments", "wrong-audience", &body)
            .await
            .status()
            .is_success()
    );
    assert_eq!(
        fixture
            .call("POST", "/devices/enrollments", "user", &body)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        fixture.call("GET", "/devices", "user", "").await.status(),
        StatusCode::FORBIDDEN
    );
    fixture.grant(IdentityAction::DevicesRead).await;
    let response = fixture.call("GET", "/devices", "user", "").await;
    assert_eq!(response.status(), StatusCode::OK);
    let rows: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.expect("body"))
            .expect("Devices");
    assert_eq!(rows[0]["name"], "first");
}

#[tokio::test]
async fn device_rename_requires_active_user_exact_grant_and_eligible_device() {
    let fixture = Fixture::new().await;
    let device = fixture.approved().await;
    let path = format!("/devices/{}", device.id);
    let rename = r#"{"name":"renamed"}"#;
    for grant in [None, Some(IdentityAction::DevicesEnroll)] {
        if let Some(action) = grant {
            fixture.grant(action).await;
        }
        assert_eq!(
            fixture.call("PATCH", &path, "user", rename).await.status(),
            StatusCode::FORBIDDEN
        );
        fixture.assert_unchanged(&device).await;
    }
    fixture.grant(IdentityAction::DevicesRename).await;
    for token in ["invalid", "wrong-audience"] {
        assert_eq!(
            fixture.call("PATCH", &path, token, rename).await.status(),
            StatusCode::UNAUTHORIZED,
            "{token}"
        );
        fixture.assert_unchanged(&device).await;
    }
    fixture.set_actor_status(Err("inactive User"));
    assert_eq!(
        fixture.call("PATCH", &path, "user", rename).await.status(),
        StatusCode::FORBIDDEN
    );
    fixture.assert_unchanged(&device).await;
    fixture.set_actor_status(Ok(()));

    let foreign = fixture
        .approved_endpoint(
            fixture.next_owner,
            SecretKey::generate().public().to_string(),
        )
        .await;
    let foreign_path = format!("/devices/{}", foreign.id);
    assert_eq!(
        fixture
            .call("PATCH", &foreign_path, "user", rename)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    fixture.assert_unchanged(&foreign).await;
    assert_eq!(
        fixture.call("PATCH", &path, "user", rename).await.status(),
        StatusCode::OK
    );
    let renamed = fixture
        .devices
        .find_approved_by_public_key(&device.public_key)
        .await
        .expect("current binding")
        .expect("Device");
    assert_eq!(renamed.name, "renamed");
}

#[tokio::test]
async fn management_requires_the_exact_installation_grant_for_each_device_action() {
    let fixture = Fixture::new().await;
    let app = Id::now_v7();
    let device = fixture.approved().await;
    fixture.grant(IdentityAction::DevicesRead).await;
    let management = fixture.management();
    for action in [
        IdentityAction::DevicesEnroll,
        IdentityAction::DevicesRename,
        IdentityAction::DevicesRevoke,
    ] {
        fixture.grant_in(app, action).await;
        let request = PermissionEvaluationRequest {
            request_id: Id::now_v7(),
            subject: PermissionSubject::User { id: fixture.owner },
            action,
            target: PermissionTarget::Installation {
                resource: IdentityResource::Device {
                    id: Some(device.id),
                },
            },
        };
        assert!(
            !management
                .evaluate_permission(&request)
                .await
                .expect("permission decision"),
            "{action:?}"
        );
        fixture.grant(action).await;
        assert!(
            management
                .evaluate_permission(&request)
                .await
                .expect("permission decision"),
            "{action:?}"
        );
    }
}

#[tokio::test]
async fn device_signed_self_revocation_requires_only_the_matching_endpoint_key() {
    let fixture = Fixture::new().await;
    let secret = SecretKey::generate();
    let device = fixture
        .approved_endpoint(fixture.owner, secret.public().to_string())
        .await;
    let other = fixture.approved().await;
    let request = |public_key: &str, signature: &str| {
        serde_json::json!({"publicKey": public_key, "signature": signature}).to_string()
    };
    let sign = |message: &str| URL_SAFE_NO_PAD.encode(secret.sign(message.as_bytes()).to_bytes());
    let path = "/devices/revoke-self";
    fixture.set_actor_status(Err("IdP unavailable"));
    fixture.disable_policy().await;
    let denied = [
        (
            serde_json::json!({"publicKey": device.public_key}).to_string(),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            request(&device.public_key, "not a signature"),
            StatusCode::FORBIDDEN,
        ),
        (
            request(&device.public_key, &sign("wrong purpose")),
            StatusCode::FORBIDDEN,
        ),
        (
            request(
                &other.public_key,
                &sign(&device_self_revocation_payload(&other.public_key)),
            ),
            StatusCode::FORBIDDEN,
        ),
    ];
    for (body, status) in denied {
        assert_eq!(fixture.call("POST", path, "", &body).await.status(), status);
        fixture.assert_unchanged(&device).await;
        fixture.assert_unchanged(&other).await;
    }
    let body = request(
        &device.public_key,
        &sign(&device_self_revocation_payload(&device.public_key)),
    );
    assert_eq!(
        fixture.call("POST", path, "", &body).await.status(),
        StatusCode::OK
    );
    assert_eq!(fixture.idp_calls.load(Ordering::SeqCst), 0);
    assert!(
        fixture
            .devices
            .find_approved_by_public_key(&device.public_key)
            .await
            .expect("approved endpoint lookup")
            .is_none()
    );
    fixture.assert_unchanged(&other).await;
}

#[tokio::test]
async fn device_administrative_revocation_preserves_protected_and_last_approved_devices() {
    let secret = SecretKey::generate();
    let endpoint = Endpoint::builder(Minimal)
        .secret_key(secret.clone())
        .bind()
        .await
        .expect("local Device endpoint");
    let identity = DeviceIdentity::new(endpoint.clone(), secret).expect("Device identity");
    let fixture = Fixture::with_device_identity(Some(Arc::new(identity))).await;
    let first = fixture.approved().await;
    let path = format!("/devices/{}", first.id);
    fixture.grant(IdentityAction::DevicesRead).await;
    fixture.grant(IdentityAction::DevicesRename).await;
    assert_eq!(
        fixture.call("DELETE", &path, "user", "").await.status(),
        StatusCode::FORBIDDEN
    );
    fixture.assert_unchanged(&first).await;

    fixture.grant(IdentityAction::DevicesRevoke).await;
    assert_eq!(
        fixture.call("DELETE", &path, "user", "").await.status(),
        StatusCode::NOT_FOUND
    );
    fixture.assert_unchanged(&first).await;

    let protected = fixture
        .approved_endpoint(fixture.owner, endpoint.id().to_string())
        .await;
    let protected_path = format!("/devices/{}", protected.id);
    assert_eq!(
        fixture
            .call("DELETE", &protected_path, "user", "")
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    fixture.assert_unchanged(&protected).await;
    assert_eq!(
        fixture.call("DELETE", &path, "user", "").await.status(),
        StatusCode::NO_CONTENT
    );
    assert!(
        fixture
            .devices
            .find_approved_by_public_key(&first.public_key)
            .await
            .expect("approved endpoint lookup")
            .is_none()
    );
    fixture.assert_unchanged(&protected).await;
    endpoint.close().await;
}

#[tokio::test]
async fn device_discovery_denies_ambiguous_endpoint_bindings() {
    let fixture = Fixture::new().await;
    let device = fixture.approved().await;
    fixture
        .database
        .client()
        .translate_and_execute("DROP INDEX devices_public_key", &SqlTranslator)
        .await
        .expect("simulate learned duplicate binding");
    let duplicate = fixture
        .devices
        .create(
            fixture.owner.to_string(),
            "duplicate".into(),
            device.public_key.clone(),
            "address".into(),
            vec![1],
            i64::MAX,
        )
        .await
        .expect("duplicate contender");
    fixture
        .devices
        .approve(duplicate.id, &[1])
        .await
        .expect("approve contender")
        .expect("Device");
    assert!(
        !fixture
            .call("GET", "/devices/endpoints", "list", "")
            .await
            .status()
            .is_success()
    );
    for id in [device.id, duplicate.id] {
        assert!(
            !fixture
                .call("GET", &format!("/devices/identities/{id}"), "lookup", "")
                .await
                .status()
                .is_success()
        );
    }
}

fn pairing_proof(key: &SecretKey, payload: &str) -> String {
    let signature = URL_SAFE_NO_PAD.encode(key.sign(payload.as_bytes()).to_bytes());
    serde_json::json!({"signature": signature}).to_string()
}

#[tokio::test]
async fn exact_pairing_replay_requires_the_original_authorized_requester() {
    let fixture = Fixture::new().await;
    let accepting = fixture.approved().await;
    let mut request = serde_json::json!({
        "name": "Retry Device", "publicKey": SecretKey::generate().public().to_string(),
        "acceptingPublicKey": accepting.public_key, "address": "127.0.0.1:23456",
    });
    fixture
        .grant_to(
            Id::nil(),
            IdentityAction::DevicesEnroll,
            &[fixture.owner, fixture.next_owner],
        )
        .await;
    async fn call(fixture: &Fixture, token: &str, body: &str) -> Response {
        fixture.call("PUT", "/devices/pairings", token, body).await
    }
    let body = request.to_string();
    let (first, replay) =
        tokio::join!(call(&fixture, "user", &body), call(&fixture, "user", &body));
    let id = |response: Response| async {
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 65536)
            .await
            .expect("pairing body");
        assert_eq!(
            status,
            StatusCode::OK,
            "{}",
            String::from_utf8_lossy(&bytes)
        );
        serde_json::from_slice::<serde_json::Value>(&bytes).expect("pairing JSON")["id"].clone()
    };
    let original = id(first).await;
    assert_eq!(original, id(replay).await);
    assert_eq!(
        call(&fixture, "next-user", &body).await.status(),
        StatusCode::BAD_REQUEST
    );
    request["address"] = serde_json::json!("127.0.0.1:23457");
    assert_eq!(
        call(&fixture, "user", &request.to_string()).await.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(original, id(call(&fixture, "user", &body).await).await);
    fixture.grant(IdentityAction::DevicesRename).await;
    let device_id = original.as_str().expect("Device record ID");
    assert_eq!(
        fixture
            .call(
                "PATCH",
                &format!("/devices/{device_id}"),
                "user",
                r#"{"name":"Renamed Device"}"#
            )
            .await
            .status(),
        StatusCode::OK
    );
    request["address"] = serde_json::json!("127.0.0.1:23456");
    request["name"] = serde_json::json!("Renamed Device");
    assert_eq!(
        call(&fixture, "user", &request.to_string()).await.status(),
        StatusCode::BAD_REQUEST
    );
    fixture.set_actor_status(Err("inactive User"));
    assert!(!call(&fixture, "user", &body).await.status().is_success());
}

#[tokio::test]
async fn device_pairing_requires_enrollment_permission_and_endpoint_proof() {
    let fixture = Fixture::new().await;
    let accepting = fixture
        .approved_endpoint(
            fixture.next_owner,
            SecretKey::generate().public().to_string(),
        )
        .await;
    let key = SecretKey::generate();
    let request = serde_json::json!({
        "name": "Second Device", "publicKey": key.public().to_string(),
        "acceptingPublicKey": accepting.public_key, "address": "127.0.0.1:23456",
    })
    .to_string();
    fixture.grant(IdentityAction::DevicesRename).await;
    assert_eq!(
        fixture
            .call("POST", "/devices/pairings", "user", &request)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        fixture.devices.list().await.expect("Devices").as_slice(),
        from_ref(&accepting)
    );
    fixture.grant(IdentityAction::DevicesEnroll).await;
    let response = fixture
        .call("POST", "/devices/pairings", "user", &request)
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let pending = fixture
        .devices
        .list_pending()
        .await
        .expect("pending Devices")
        .pop()
        .expect("pending pairing");
    let path = format!("/devices/pairings/{}/approve", pending.id);
    let response = fixture
        .call(
            "GET",
            &format!("/devices/pairings/{}/payload", pending.id),
            "user",
            "",
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.expect("body"))
            .expect("payload JSON");
    let payload = body["payload"].as_str().expect("payload");
    let proof = |signer: &SecretKey| pairing_proof(signer, payload);
    let legacy_path = format!("/devices/enrollments/{}/approve", pending.id);
    fixture
        .assert_denied(
            "POST",
            &legacy_path,
            "user",
            "",
            StatusCode::NOT_FOUND,
            &pending,
        )
        .await;
    assert_eq!(
        fixture
            .call("POST", &path, "user", &proof(&SecretKey::generate()))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    fixture.assert_unchanged(&pending).await;
    assert_eq!(
        fixture
            .call("POST", &path, "user", &proof(&key))
            .await
            .status(),
        StatusCode::OK
    );
    let approved = fixture
        .devices
        .find_approved_by_public_key(&pending.public_key)
        .await
        .expect("approved endpoint lookup")
        .expect("approved Device");
    assert_eq!(approved.id, pending.id);
    assert_eq!(approved.owner_subject, fixture.next_owner.to_string());

    assert_eq!(
        fixture
            .call("GET", "/devices/pairing-accepting", "user", "")
            .await
            .status(),
        StatusCode::METHOD_NOT_ALLOWED
    );
}

#[tokio::test]
async fn device_pairing_denies_valid_proof_after_accepting_device_revocation() {
    let fixture = Fixture::new().await;
    let accepting = fixture.approved().await;
    fixture.grant(IdentityAction::DevicesEnroll).await;
    let (next, proof) = fixture.pending_pairing(&accepting).await;
    assert!(
        fixture
            .devices
            .revoke_self(&accepting.public_key)
            .await
            .expect("revoke accepting Device")
    );
    let path = format!("/devices/pairings/{}/approve", next.id);
    assert_eq!(
        fixture.call("POST", &path, "user", &proof).await.status(),
        StatusCode::FORBIDDEN
    );
    fixture.assert_unchanged(&next).await;
}

#[tokio::test]
async fn device_approval_and_revocation_deny_inactive_users_and_unavailable_policy() {
    let fixture = Fixture::new().await;
    let revocable = fixture.approved().await;
    let accepting = fixture
        .approved_endpoint(
            fixture.next_owner,
            SecretKey::generate().public().to_string(),
        )
        .await;
    let (pairing, proof) = fixture.pending_pairing(&accepting).await;
    let enrollment = fixture
        .devices
        .create(
            fixture.owner.to_string(),
            "legacy".into(),
            SecretKey::generate().public().to_string(),
            "address".into(),
            vec![1],
            i64::MAX,
        )
        .await
        .expect("pending enrollment");
    fixture.grant(IdentityAction::DevicesRename).await;
    let attempts = [
        (
            "POST",
            format!("/devices/pairings/{}/approve", pairing.id),
            proof.as_str(),
            &pairing,
        ),
        (
            "POST",
            format!("/devices/enrollments/{}/approve", enrollment.id),
            "",
            &enrollment,
        ),
        (
            "DELETE",
            format!("/devices/{}", revocable.id),
            "",
            &revocable,
        ),
    ];
    for (method, path, body, device) in &attempts {
        fixture
            .assert_denied(method, path, "user", body, StatusCode::FORBIDDEN, device)
            .await;
    }
    fixture.grant(IdentityAction::DevicesEnroll).await;
    fixture.grant(IdentityAction::DevicesRevoke).await;
    for (token, active_status, expected) in [
        ("user", Err("inactive User"), StatusCode::FORBIDDEN),
        ("user", Err("User not found"), StatusCode::FORBIDDEN),
        (
            "user",
            Err("IdP User lookup unavailable"),
            StatusCode::FORBIDDEN,
        ),
        ("invalid-subject", Ok(()), StatusCode::SERVICE_UNAVAILABLE),
        ("nil-subject", Ok(()), StatusCode::FORBIDDEN),
    ] {
        fixture.set_actor_status(active_status);
        for (method, path, body, device) in &attempts {
            fixture
                .assert_denied(method, path, token, body, expected, device)
                .await;
        }
    }
    fixture.disable_policy().await;
    for (method, path, body, device) in attempts {
        fixture
            .assert_denied(
                method,
                &path,
                "user",
                body,
                StatusCode::INTERNAL_SERVER_ERROR,
                device,
            )
            .await;
    }
    // A nil pairing ID must fail target validation before touching the unavailable policy store.
    assert_eq!(
        fixture
            .call(
                "POST",
                &format!("/devices/pairings/{}/approve", Id::nil()),
                "user",
                &proof
            )
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    fixture.assert_unchanged(&pairing).await;
}
