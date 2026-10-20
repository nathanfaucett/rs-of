use std::sync::Arc;

use axum::{body::Body, http::Request};
use idp_model::model::Id;
use management_service::{
    HostedControlPlane, ManagementService,
    replica::{DbPermissionRepo, DbRoleRepo, DbSelectionPolicyRepo},
};
use serde_json::Value;
use tower::ServiceExt;

use super::{RouterState, openapi_router};

#[tokio::test]
async fn composed_openapi_requires_current_selected_resources_policy() {
    assert_contract(&document().await);
}

#[tokio::test]
#[ignore = "explicit generation workflow supplies OF_MANAGEMENT_OPENAPI_OUT; no host or provider changes"]
async fn export_management_openapi_for_client_generation() {
    let path = std::env::var_os("OF_MANAGEMENT_OPENAPI_OUT")
        .expect("explicit Management schema destination");
    let document = document().await;
    assert_contract(&document);
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&document).expect("Management schema JSON"),
    )
    .expect("write requested public Management schema");
}

async fn document() -> Value {
    let root = std::env::temp_dir().join(format!("management-openapi-{}", Id::now_v7()));
    std::fs::create_dir_all(&root).expect("create isolated schema export directory");
    let engine =
        Arc::new(db::Database::open(root.join("management.redb")).expect("open isolated engine"));
    let state = RouterState::new(
        "https://management.example",
        Arc::new(ManagementService::new(
            DbPermissionRepo::new(Arc::clone(&engine)),
            DbRoleRepo::new(Arc::clone(&engine)),
        )),
        Arc::new(DbSelectionPolicyRepo::new(Arc::clone(&engine))),
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
    let response = openapi_router(state, "/management")
        .split_for_parts()
        .0
        .oneshot(
            Request::builder()
                .uri("/management/openapi.json")
                .body(Body::empty())
                .expect("public OpenAPI request"),
        )
        .await
        .expect("serve composed Management OpenAPI");
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read composed Management OpenAPI");
    let document: Value = serde_json::from_slice(&body).expect("public Management OpenAPI JSON");
    assert_eq!(
        document,
        serde_json::to_value(crate::openapi_document(
            "https://management.example",
            "/management",
        ))
        .expect("state-free Management OpenAPI JSON"),
    );
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
    assert_eq!(operations, 38, "complete Management operation inventory");
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
            "Management owns {method} {path}"
        );
    }
    assert!(paths.keys().all(|path| !path.starts_with("/management/")));
    assert_eq!(
        document["servers"][0]["url"],
        "https://management.example/management"
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
