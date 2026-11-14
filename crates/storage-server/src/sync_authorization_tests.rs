use std::sync::{Arc, Mutex};

use axum::{
    Json, Router,
    routing::{get, post},
};
use serde_json::{Value, json};

use super::{StoreIdentity, SyncAuthError, authorize_sync};
use crate::{DeviceClient, IdpClient};

#[tokio::test]
async fn ws_authorization_requires_exact_current_resource_policy() {
    let store_id = uuid::Uuid::now_v7();
    let policy = Arc::new(Mutex::new(json!({
        "deviceId": "device", "endpointId": "local", "ownerSubject": "alice",
        "adminAllowed": true,
        "resources": [{ "ownerSubject": "alice", "applicationId": "app",
            "kind": "database", "resourceId": store_id.to_string() }],
    })));
    let selected = policy.clone();
    let app = Router::new()
        .route("/idp/oauth2/token", post(|| async { Json(json!({
            "access_token": "service-token", "token_type": "Bearer", "expires_in": 300,
            "scope": "management.device.list management.replication.read management.replication.admit",
            "iss": "https://idp.example",
        })) }))
        .route("/device/replication/devices/local/selections", get(move || {
            let selected = selected.clone();
            async move { Json(selected.lock().expect("policy lock").clone()) }
        }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind owners");
    let base = format!("http://{}", listener.local_addr().expect("owner address"));
    let server =
        tokio::spawn(async move { axum::serve(listener, app).await.expect("serve owners") });
    let device = DeviceClient::new(
        &format!("{base}/device"),
        &format!("{base}/idp"),
        "service",
        "secret",
        "https://idp.example",
        "idp-management",
    )
    .expect("Device client");
    let idp = IdpClient::local(|_| async {
        Ok(serde_json::from_value(json!({
            "claims": { "type": "Bearer", "use": "Access", "exp": i64::MAX,
                "iat": 0, "nbf": 0, "iss": "https://idp.example", "aud": "storage:sync",
                "client_id": "client", "sub": "alice", "principal_type": "user", "scope": "" },
            "applicationId": "app",
        }))
        .expect("introspection fixture"))
    });
    let identity = StoreIdentity {
        store_id,
        replica_id: uuid::Uuid::now_v7(),
        creation_epoch: 1,
        schema_fingerprint: [0; 32],
    };
    assert!(
        authorize_sync(&idp, &device, "user-token", &identity, "local")
            .await
            .is_ok()
    );
    let baseline = policy.lock().expect("policy lock").clone();
    let mutations = [
        ("/ownerSubject", json!("bob")),
        ("/endpointId", json!("another-endpoint")),
        ("/adminAllowed", json!(false)),
        ("/resources/0/ownerSubject", json!("bob")),
        ("/resources/0/kind", json!("filesystem")),
        ("/resources/0/applicationId", json!("other-app")),
        (
            "/resources/0/resourceId",
            json!(uuid::Uuid::now_v7().to_string()),
        ),
        ("/resources", json!([])),
    ];
    for (pointer, value) in mutations {
        let mut changed = baseline.clone();
        *changed.pointer_mut(pointer).expect("fixture pointer") = value;
        *policy.lock().expect("policy lock") = changed;
        assert_eq!(
            authorize_sync(&idp, &device, "user-token", &identity, "local").await,
            Err(SyncAuthError::Unauthorized),
            "must reject {pointer}"
        );
    }
    let mut cleanup = baseline;
    cleanup["pendingCleanup"] = json!({ "cleanupId": "cleanup", "deviceId": "device", "endpointId": "local",
        "oldOwnerSubject": "bob", "newOwnerSubject": "alice" });
    *policy.lock().expect("policy lock") = cleanup;
    assert_eq!(
        authorize_sync(&idp, &device, "user-token", &identity, "local").await,
        Err(SyncAuthError::Unauthorized)
    );
    *policy.lock().expect("policy lock") = Value::Null;
    assert_eq!(
        authorize_sync(&idp, &device, "user-token", &identity, "local").await,
        Err(SyncAuthError::BackendUnavailable)
    );
    server.abort();
    let _ = server.await;
}

#[tokio::test]
async fn local_ws_requires_database_capability_and_rechecks_revocation() {
    let identity = StoreIdentity {
        store_id: uuid::Uuid::now_v7(),
        replica_id: uuid::Uuid::now_v7(),
        creation_epoch: 0,
        schema_fingerprint: [0; 32],
    };
    let response = Arc::new(Mutex::new(json!({
        "claims": { "type": "Bearer", "use": "Access", "exp": i64::MAX, "iat": 0, "nbf": 0,
            "iss": "https://idp.example", "aud": "storage:sync", "resource": "storage:sync",
            "authorization_details": [{ "type": "storage", "actions": ["read", "write"] }],
            "client_id": "client", "sub": "alice", "principal_type": "user", "scope": "" },
        "applicationId": uuid::Uuid::now_v7().to_string()
    })));
    let current = response.clone();
    let idp = IdpClient::local(move |_| {
        let current = current.clone();
        async move {
            Ok(
                serde_json::from_value(current.lock().expect("claims lock").clone())
                    .expect("introspection"),
            )
        }
    });
    let baseline = response.lock().expect("claims lock").clone();
    let expected = super::authorize_session(&idp, None, "resource-token", &identity, "local")
        .await
        .expect("scoped capability");
    let authorization = super::SessionAuthorization {
        idp: &idp,
        device: None,
        token: "resource-token",
        identity: &identity,
        endpoint_id: "local",
        expected: &expected,
    };
    for (pointer, value) in [
        ("/claims/aud", json!("https://idp.example/userinfo")),
        ("/claims/resource", json!("storage:filesystems")),
        ("/claims/principal_type", json!("client")),
        ("/claims/exp", json!(0)),
        ("/claims/authorization_details/0/actions", json!(["read"])),
        ("/claims/authorization_details/0/actions", json!(["write"])),
    ] {
        let mut changed = baseline.clone();
        *changed.pointer_mut(pointer).expect("fixture field") = value;
        *response.lock().expect("claims lock") = changed;
        assert!(
            authorization.check().await.is_err(),
            "must reject {pointer}"
        );
    }
    for (pointer, value) in [
        ("/claims/sub", json!("bob")),
        ("/applicationId", json!(uuid::Uuid::now_v7().to_string())),
    ] {
        let mut changed = baseline.clone();
        *changed.pointer_mut(pointer).expect("fixture field") = value;
        *response.lock().expect("claims lock") = changed;
        assert!(
            authorization.check().await.is_err(),
            "session scope cannot change at {pointer}"
        );
    }
    *response.lock().expect("claims lock") = baseline;
    authorization
        .check()
        .await
        .expect("same capability can retry");
}
