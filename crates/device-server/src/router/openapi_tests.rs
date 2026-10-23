use std::sync::Arc;

use axum::{body::Body, http::Request};
use device_service::HostedControlPlane;
use device_service::replica::DbSelectionPolicyRepo;
use idp_model::model::Id;
use management_service::ManagementService;
use management_service::replica::DbPermissionRepo;
use management_service::replica::DbRoleRepo;
use serde_json::Value;
use tower::ServiceExt;

use super::{RouterState, openapi_router};

#[tokio::test]
async fn composed_openapi_requires_current_selected_resources_policy() {
    assert_contract(&document().await);
}

async fn document() -> Value {
    let root = std::env::temp_dir().join(format!("device-openapi-{}", Id::now_v7()));
    std::fs::create_dir_all(&root).expect("create isolated schema export directory");
    let engine =
        Arc::new(db::Database::open(root.join("devices.redb")).expect("open isolated engine"));
    let state = RouterState::new(
        "https://device.example/device",
        Arc::new(ManagementService::new(
            DbPermissionRepo::new(engine.client()),
            DbRoleRepo::new(engine.client()),
        )),
        Arc::new(DbSelectionPolicyRepo::new(Arc::clone(&engine).client())),
        Arc::new(
            HostedControlPlane::new_with_services(
                "http://127.0.0.1",
                "http://127.0.0.1",
                "https://installation.example",
            )
            .expect("construct unused control plane"),
        ),
        "storage-api",
    );
    let response = openapi_router(state, "/device")
        .split_for_parts()
        .0
        .oneshot(
            Request::builder()
                .uri("/device/openapi.json")
                .body(Body::empty())
                .expect("public OpenAPI request"),
        )
        .await
        .expect("serve composed Device OpenAPI");
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read composed Device OpenAPI");
    let document: Value = serde_json::from_slice(&body).expect("public Device OpenAPI JSON");
    let mut expected: Value = serde_json::from_str(include_str!("../../../../openapi/device.json"))
        .expect("exported Device contract");
    expected["servers"] = serde_json::json!([{ "url": "https://device.example/device" }]);
    assert_eq!(document, expected, "runtime Device contract matches export");
    drop(engine);
    std::fs::remove_dir_all(root).expect("remove isolated schema export database");
    document
}

fn assert_contract(document: &Value) {
    let paths = document["paths"].as_object().expect("complete owner paths");
    let operations = paths
        .values()
        .flat_map(|item| item.as_object().expect("path item").values())
        .filter(|operation| operation.get("operationId").is_some())
        .count();
    assert_eq!(operations, 50, "complete Device operation inventory");
    assert!(paths.contains_key("/openapi.json"));
    for (path, method) in [
        ("/replication/owner-cleanup", "post"),
        ("/device", "get"),
        ("/devices", "get"),
        ("/devices/endpoints", "get"),
        ("/devices/endpoints/{endpoint_id}", "get"),
        ("/devices/identities/{device_id}", "get"),
        ("/devices/trusted", "get"),
        ("/devices/enrollments", "post"),
        ("/devices/revoke-self", "post"),
        ("/devices/pairing-accepting", "get"),
        ("/devices/pairing-accepting", "put"),
        ("/devices/{id}", "patch"),
        ("/devices/{id}", "delete"),
        ("/devices/{device_id}/owner", "put"),
    ] {
        assert!(
            document["paths"][path][method].is_object(),
            "Device owns {method} {path}"
        );
    }
    assert!(paths.keys().all(|path| !path.starts_with("/device/")));
    assert_eq!(
        document["servers"][0]["url"],
        "https://device.example/device"
    );

    let schema = &document["components"]["schemas"]["SelectedResourcesResponse"];
    let fields = [
        "adminAllowed",
        "deviceId",
        "endpointId",
        "ownerSubject",
        "resources",
    ];
    assert_eq!(
        schema["properties"]
            .as_object()
            .expect("selected resources policy properties")
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [
            "adminAllowed",
            "deviceId",
            "endpointId",
            "ownerSubject",
            "pendingCleanup",
            "resources"
        ],
    );
    let mut required = schema["required"]
        .as_array()
        .expect("required selected resources policy fields")
        .iter()
        .map(|field| field.as_str().expect("required field name"))
        .collect::<Vec<_>>();
    required.sort_unstable();
    assert_eq!(required, fields);
    for field in ["deviceId", "endpointId", "ownerSubject"] {
        assert_eq!(schema["properties"][field]["type"], "string");
    }
    assert_eq!(schema["properties"]["adminAllowed"]["type"], "boolean");
    assert_eq!(schema["properties"]["resources"]["type"], "array");
}
