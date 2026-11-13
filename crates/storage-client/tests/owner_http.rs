#![cfg(feature = "native")]

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::{
    Form, Json, Router,
    http::HeaderMap,
    routing::{get, post},
};
use of_storage_client::{DeviceId, ResourceId, native::StorageClient};
use serde_json::{Value, json};
use uuid::Uuid;

#[tokio::test]
async fn resource_clients_use_owner_scoped_tokens_and_current_http_contracts() {
    let resource = Uuid::now_v7();
    let target = Uuid::now_v7();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let exchanges = requests.clone();
    let router = Router::new()
        .route("/idp/oauth2/token", post(move |Form(form): Form<HashMap<String, String>>| {
            let exchanges = exchanges.clone();
            async move {
                assert_eq!(form["grant_type"], "urn:ietf:params:oauth:grant-type:token-exchange");
                assert_eq!(form["subject_token"], "subject-token");
                assert_eq!(form["client_id"], "client");
                let details: Value = serde_json::from_str(&form["authorization_details"]).expect("authorization details");
                assert_eq!(details[0]["type"], "storage");
                let resource = form["resource"].clone();
                exchanges.lock().expect("exchanges lock").push(resource.clone());
                Json(json!({"access_token": resource, "token_type": "Bearer"}))
            }
        }))
        .route("/storage/databases", get(move |headers: HeaderMap| async move {
            assert_eq!(headers["authorization"], "Bearer storage:databases");
            Json(json!([{"id": resource, "name": "records"}]))
        }))
        .route("/storage/filesystems", get(move |headers: HeaderMap| async move {
            assert_eq!(headers["authorization"], "Bearer storage:filesystems");
            Json(json!([{"id": resource, "name": "files"}]))
        }))
        .route("/sync/connect", post(move |headers: HeaderMap, Json(request): Json<Value>| async move {
            assert_eq!(headers["authorization"], "Bearer storage:sync");
            assert_eq!(request, json!({"resourceId": resource, "targetDevice": target}));
            Json(json!({"sessionId": Uuid::now_v7(), "resourceId": resource, "targetDevice": target, "sourceDevice": Uuid::now_v7(), "status": "completed", "bytesSent": 0, "bytesReceived": 0}))
        }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("owner listener");
    let base = format!("http://{}", listener.local_addr().expect("owner address"));
    let task =
        tokio::spawn(async move { axum::serve(listener, router).await.expect("owner service") });
    let client = StorageClient::new(&base, &format!("{base}/idp"), "client", |_, _| {
        Box::pin(async { Ok("subject-token".into()) })
    });
    assert_eq!(
        client.list_sql_dbs().await.expect("SQL resources")[0].id.0,
        resource
    );
    assert_eq!(
        client.list_kv_stores().await.expect("KV resources")[0].id.0,
        resource
    );
    assert_eq!(
        client
            .list_filesystems()
            .await
            .expect("filesystem resources")[0]
            .id
            .0,
        resource
    );
    let session = client
        .sync_to_device(ResourceId(resource), DeviceId(target))
        .await
        .expect("current session response");
    assert_eq!(session.target_device.0, target);
    assert_eq!(
        requests.lock().expect("requests lock").as_slice(),
        [
            "storage:databases",
            "storage:databases",
            "storage:filesystems",
            "storage:sync"
        ]
    );
    task.abort();
}
