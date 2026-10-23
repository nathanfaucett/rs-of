use std::sync::{Arc, OnceLock};

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use db::Database;
use device_service::HostedControlPlane;
use idp_model::{
    contract::{IdentityAction, IntrospectionResponse},
    model::Id,
};
use model::contract::{PrincipalType, StandardClaims, TokenType, TokenUse};
use tower::ServiceExt;

#[tokio::test]
async fn device_http_and_storage_share_isolated_authority_and_recheck_revocation() {
    let identity = Arc::new(Database::in_memory());
    idp_model::replica::up(&identity)
        .await
        .expect("identity owner schema");
    let devices = Arc::new(Database::in_memory());
    device_service::replica::up(&devices)
        .await
        .expect("Device owner schema");
    let owner = Id::now_v7();
    let application = Id::now_v7();
    let permissions = [
        IdentityAction::DevicePairingRead,
        IdentityAction::DevicePairingUpdate,
        IdentityAction::DevicesEnroll,
        IdentityAction::DevicesRevoke,
        IdentityAction::DeviceRestrictionsUpdate,
    ]
    .map(|action| (Id::now_v7(), action));
    management_server::provision_initial_administrator(
        identity.clone(),
        owner,
        Id::now_v7(),
        &permissions,
    )
    .await
    .expect("explicit Device administrator grants");
    let control = Arc::new(
        HostedControlPlane::new_with_issuer("https://node.example", "https://installation.example")
            .expect("owner interface")
            .with_local_idp(
                move |_| async move {
                    Ok(IntrospectionResponse {
                        claims: StandardClaims {
                            r#type: TokenType::Bearer,
                            r#use: TokenUse::Access,
                            exp: i64::MAX,
                            iat: 0,
                            nbf: 0,
                            iss: "https://installation.example".into(),
                            aud: management_service::MANAGEMENT_APPLICATION_URI.into(),
                            client_id: "human".into(),
                            sub: owner.to_string(),
                            principal_type: PrincipalType::User,
                            resource: None,
                            authorization_details: None,
                            scope: vec![],
                        },
                        application_id: application.to_string(),
                    })
                },
                move |user| async move {
                    if user == owner {
                        Ok(())
                    } else {
                        Err("inactive User".into())
                    }
                },
            ),
    );
    let router = crate::compose_router(
        axum::Router::new(),
        management_server::build_router(
            identity.client(),
            "https://node.example",
            "/management",
            control.clone(),
        )
        .await
        .expect("Management router"),
        device_server::build_router(
            identity.clone(),
            devices.clone(),
            "https://node.example",
            "/device",
            "storage",
            control.clone(),
            None,
        )
        .await
        .expect("Device router"),
        axum::Router::new(),
    );
    let endpoint = iroh::SecretKey::generate().public();
    let request = |method: &str, path: &str, body: String| {
        Request::builder()
            .method(method)
            .uri(path)
            .header("authorization", "Bearer user")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .expect("request")
    };
    let response = router
        .clone()
        .oneshot(request(
            "POST",
            "/device/devices/enrollments",
            serde_json::json!({
                "name": "replica", "publicKey": endpoint.to_string(), "address": "address",
            })
            .to_string(),
        ))
        .await
        .expect("enrollment response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 65536)
        .await
        .expect("enrollment body");
    let enrollment: device_service::DeviceEnrollment =
        serde_json::from_slice(&body).expect("enrollment");
    assert_eq!(enrollment.state, device_service::DeviceState::Approved);
    let copies = Arc::new(OnceLock::new());
    let storage = crate::runtime::owner_replication_authorizer(
        device_server::DeviceAuthority::new(devices.clone(), control),
        iroh::SecretKey::generate().public(),
        copies,
    );
    assert!(
        storage.local_owner(endpoint).await.is_err(),
        "missing policy must fail closed"
    );
    let restriction = format!("/device/devices/{}/restriction", enrollment.id);
    let response = router
        .clone()
        .oneshot(request(
            "PUT",
            &restriction,
            "{\"adminAllowed\":true}".into(),
        ))
        .await
        .expect("policy response");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        storage
            .local_owner(endpoint)
            .await
            .expect("Storage sees approved Device"),
        owner.to_string()
    );
    assert!(
        storage
            .local_owner(iroh::SecretKey::generate().public())
            .await
            .is_err(),
        "unknown endpoint must fail closed"
    );
    for table in [
        "devices",
        "device_selection_policies",
        "device_resource_selections",
        "local_device_enrollments",
    ] {
        assert!(
            identity
                .client()
                .translate_and_execute(
                    &format!("SELECT * FROM {table} LIMIT 0"),
                    &db::SqlTranslator
                )
                .await
                .is_err(),
            "Device records must not enter the identity schema: {table}"
        );
    }
    for path in [
        "/management/devices",
        "/management/replication/admission",
        "/management/node-joins/acceptance",
    ] {
        assert_eq!(
            router
                .clone()
                .oneshot(request("GET", path, String::new()))
                .await
                .expect("old route")
                .status(),
            StatusCode::NOT_FOUND
        );
    }
    // Keep another approved member: revoking the last approved Device is forbidden.
    use device_service::DeviceRepo;
    let peers = device_service::replica::DbDeviceRepo::new(devices);
    let peer = peers
        .create(
            owner.to_string(),
            "peer".into(),
            iroh::SecretKey::generate().public().to_string(),
            "address".into(),
            vec![1],
            i64::MAX,
        )
        .await
        .expect("peer enrollment");
    peers
        .approve(peer.id, &[1])
        .await
        .expect("peer approval")
        .expect("approved peer");
    let response = router
        .oneshot(request(
            "DELETE",
            &format!("/device/devices/{}", enrollment.id),
            String::new(),
        ))
        .await
        .expect("revoke response");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(
        storage.local_owner(endpoint).await.is_err(),
        "revocation must invalidate existing Storage authority"
    );
}
