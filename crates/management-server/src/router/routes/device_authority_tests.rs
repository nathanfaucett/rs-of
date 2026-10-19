use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use db::Database;
use idp_model::{
    contract::{IdentityAction, IntrospectionResponse},
    model::Id,
};
use management_service::{
    DeviceRepo, HostedControlPlane, MANAGEMENT_APPLICATION_URI, ManagementService,
    replica::{DbDeviceRepo, DbPermissionRepo, DbRoleRepo, DbSelectionPolicyRepo, SelectionPolicy},
};
use model::contract::{PrincipalType, StandardClaims, TokenType, TokenUse};
use tower::ServiceExt;

struct Fixture {
    router: Router,
    database: Arc<Database>,
    devices: Arc<DbDeviceRepo>,
    owner: Id,
    next_owner: Id,
    active: Arc<AtomicBool>,
}

impl Fixture {
    async fn new() -> Self {
        let database = Arc::new(Database::in_memory());
        management_service::replica::up(&database)
            .await
            .expect("Management schema");
        let owner = Id::now_v7();
        let next_owner = Id::now_v7();
        let active = Arc::new(AtomicBool::new(true));
        let current = active.clone();
        let control = HostedControlPlane::new_with_services(
            "http://127.0.0.1:1/idp",
            "http://127.0.0.1:1/storage",
            "https://installation.example",
        )
        .expect("unreachable remote owners")
        .with_local_idp(
            move |token| async move {
                let (principal_type, audience, scopes) = match token.as_str() {
                    "user" => (PrincipalType::User, MANAGEMENT_APPLICATION_URI, vec![]),
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
                        sub: owner.to_string(),
                        principal_type,
                        resource: None,
                        authorization_details: None,
                        scope: scopes,
                    },
                    application_id: Id::now_v7().to_string(),
                })
            },
            move |id| {
                let active = current.clone();
                async move {
                    if active.load(Ordering::SeqCst) && (id == owner || id == next_owner) {
                        Ok(())
                    } else {
                        Err("inactive User".into())
                    }
                }
            },
        );
        let devices = Arc::new(DbDeviceRepo::new(database.clone()));
        let router = crate::build_router(
            database.clone(),
            "https://management.example",
            "/management",
            "storage",
            Arc::new(control),
        )
        .await
        .expect("existing owner router");
        Self {
            router,
            database,
            devices,
            owner,
            next_owner,
            active,
        }
    }

    async fn call(
        &self,
        method: &str,
        path: &str,
        token: &str,
        body: &str,
    ) -> axum::response::Response {
        self.router
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(format!("/management{path}"))
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_owned()))
                    .expect("request"),
            )
            .await
            .expect("Management response")
    }

    async fn approved(&self) -> management_service::Device {
        let endpoint = iroh::SecretKey::generate().public().to_string();
        let pending = self
            .devices
            .create(
                self.owner.to_string(),
                "owned".into(),
                endpoint,
                "address".into(),
                vec![1],
                i64::MAX,
            )
            .await
            .expect("pending Device");
        if pending.state == management_service::DeviceState::Approved {
            return pending;
        }
        self.devices
            .approve(pending.id, &[1])
            .await
            .expect("approval")
            .expect("Device")
    }

    async fn grant(&self, action: IdentityAction) {
        let service = ManagementService::new(
            DbPermissionRepo::new(self.database.clone()),
            DbRoleRepo::new(self.database.clone()),
        );
        let role = service
            .create_role(Id::nil(), action.permission(), None)
            .await
            .expect("role");
        let permission = service
            .create_permission(Id::nil(), action.permission(), None)
            .await
            .expect("permission");
        service
            .add_permission_to_role(Id::nil(), role.id, permission.id)
            .await
            .expect("role grant");
        service
            .add_role_to_user(Id::nil(), self.owner, role.id)
            .await
            .expect("User grant");
    }
}

#[tokio::test]
async fn management_endpoint_enumeration_requires_exact_service_authority_and_current_owner() {
    let fixture = Fixture::new().await;
    let device = fixture.approved().await;
    let pending_endpoint = iroh::SecretKey::generate().public().to_string();
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
    fixture.active.store(false, Ordering::SeqCst);
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
    fixture.active.store(true, Ordering::SeqCst);
    fixture
        .database
        .client()
        .translate_and_execute(
            &format!(
                "UPDATE devices SET revoked_at = 9 WHERE id = CAST('{}' AS UUID)",
                device.id
            ),
            &db::SqlTranslator,
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
async fn management_transfer_requires_exact_permission_active_new_owner_and_preserves_cleanup_fence()
 {
    let fixture = Fixture::new().await;
    let device = fixture.approved().await;
    let policies = DbSelectionPolicyRepo::new(fixture.database.clone());
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
async fn management_device_lifecycle_enforces_user_audience_owner_and_exact_actions() {
    let fixture = Fixture::new().await;
    let endpoint = iroh::SecretKey::generate().public().to_string();
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
    let path = format!("/devices/{}", rows[0]["id"].as_str().expect("Device ID"));
    assert_eq!(
        fixture
            .call("PATCH", &path, "user", r#"{"name":"renamed"}"#)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    fixture.grant(IdentityAction::DevicesRename).await;
    assert_eq!(
        fixture
            .call("PATCH", &path, "user", r#"{"name":"renamed"}"#)
            .await
            .status(),
        StatusCode::OK
    );
    fixture.active.store(false, Ordering::SeqCst);
    assert!(
        !fixture
            .call("PATCH", &path, "user", r#"{"name":"inactive"}"#)
            .await
            .status()
            .is_success()
    );
}

#[tokio::test]
async fn management_signed_self_revocation_checks_endpoint_possession_without_idp_device_proxy() {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    let fixture = Fixture::new().await;
    let secret = iroh::SecretKey::generate();
    let public = secret.public().to_string();
    let device = fixture
        .devices
        .create(
            fixture.owner.to_string(),
            "self".into(),
            public.clone(),
            "address".into(),
            vec![],
            0,
        )
        .await
        .expect("approved first Device");
    let request = |signature: String| {
        serde_json::json!({"publicKey":public, "signature":signature}).to_string()
    };
    let bad = URL_SAFE_NO_PAD.encode(secret.sign(b"wrong purpose").to_bytes());
    assert_eq!(
        fixture
            .call("POST", "/devices/revoke-self", "invalid", &request(bad))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    let signature = URL_SAFE_NO_PAD.encode(
        secret
            .sign(management_service::device_self_revocation_payload(&public).as_bytes())
            .to_bytes(),
    );
    assert_eq!(
        fixture
            .call(
                "POST",
                "/devices/revoke-self",
                "invalid",
                &request(signature)
            )
            .await
            .status(),
        StatusCode::OK
    );
    assert!(
        fixture
            .devices
            .find_approved_by_public_key(&device.public_key)
            .await
            .expect("current Device")
            .is_none()
    );
}

#[tokio::test]
async fn management_revoke_requires_exact_grant_and_cannot_be_replaced_by_read_or_rename() {
    let fixture = Fixture::new().await;
    let device = fixture.approved().await;
    let _surviving_device = fixture.approved().await;
    let path = format!("/devices/{}", device.id);
    fixture.grant(IdentityAction::DevicesRead).await;
    fixture.grant(IdentityAction::DevicesRename).await;
    assert_eq!(
        fixture.call("DELETE", &path, "user", "").await.status(),
        StatusCode::FORBIDDEN
    );
    fixture.grant(IdentityAction::DevicesRevoke).await;
    assert_eq!(
        fixture.call("DELETE", &path, "user", "").await.status(),
        StatusCode::NO_CONTENT
    );
    assert!(
        fixture
            .devices
            .find_approved_by_public_key(&device.public_key)
            .await
            .expect("current Device")
            .is_none()
    );
}

#[tokio::test]
async fn management_device_discovery_denies_ambiguous_endpoint_bindings() {
    let fixture = Fixture::new().await;
    let device = fixture.approved().await;
    fixture
        .database
        .client()
        .translate_and_execute("DROP INDEX devices_public_key", &db::SqlTranslator)
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

#[tokio::test]
async fn management_pairing_controls_require_exact_permissions_and_an_injected_transport_capability()
 {
    let fixture = Fixture::new().await;
    assert_eq!(
        fixture
            .call("GET", "/devices/pairing-accepting", "user", "")
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        fixture
            .call(
                "PUT",
                "/devices/pairing-accepting",
                "user",
                r#"{"accepting":true}"#
            )
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    fixture.grant(IdentityAction::DevicePairingRead).await;
    assert_eq!(
        fixture
            .call("GET", "/devices/pairing-accepting", "user", "")
            .await
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        fixture
            .call(
                "PUT",
                "/devices/pairing-accepting",
                "user",
                r#"{"accepting":true}"#
            )
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    fixture.grant(IdentityAction::DevicePairingUpdate).await;
    assert_eq!(
        fixture
            .call(
                "PUT",
                "/devices/pairing-accepting",
                "user",
                r#"{"accepting":true}"#
            )
            .await
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}
