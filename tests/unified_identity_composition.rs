#![cfg(unix)]

use std::fs;

#[path = "support/identity.rs"]
mod identity;
use identity::{expect_status, seed_idp, seed_permissions_at, user_token};

#[test]
fn shared_identity_management_writes_change_idp_authorization() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .thread_stack_size(16 * 1024 * 1024)
                .enable_all()
                .build()
                .expect("shared identity runtime")
                .block_on(shared_identity_acceptance());
        })
        .expect("shared identity thread")
        .join()
        .expect("shared identity acceptance");
}

async fn shared_identity_acceptance() {
    let root = std::env::temp_dir().join(format!(
        "shared-identity-{}",
        idp_model::model::Id::now_v7()
    ));
    let mut config = unified_server::UnifiedConfig {
        listen_addr: "127.0.0.1:0".parse().expect("loopback"),
        endpoint_data_dir: root.join("endpoint"),
        idp: idp_server::AppConfig::default(),
        management: management_server::AppConfig::default(),
        device: device_server::AppConfig::default(),
        storage: storage_server::AppConfig::default(),
    };
    config.idp.data_dir = root.join("idp").to_string_lossy().into_owned();
    config.device.data_dir = root.join("device").to_string_lossy().into_owned();
    config.storage.data_dir = root.join("storage").to_string_lossy().into_owned();
    config.idp.oauth2.issuer = format!(
        "https://shared-{}.example/idp",
        idp_model::model::Id::now_v7()
    );
    config.idp.service_audience = Some("idp-services".into());
    config.device.storage_audience = "storage-service".into();
    let endpoint =
        unified_server::UnifiedEndpoint::open_fresh(&config.endpoint_data_dir, &config.idp)
            .await
            .expect("fresh endpoint");
    let mut fixture = seed_idp(
        &root,
        &config.idp.oauth2.issuer,
        &endpoint.endpoint_id().to_string(),
        &iroh::SecretKey::generate().public().to_string(),
    )
    .await;
    config.idp.oauth2.local_signer = Some(fixture.local_signer.clone());
    seed_permissions_at(&root.join("idp/idp.redb"), &mut fixture).await;
    endpoint.close().await;
    let mut runtime = unified_server::UnifiedRuntime::build(config.clone())
        .await
        .expect("shared host");
    let base = format!("http://{}", runtime.local_addr());
    let listener = runtime.take_listener().expect("single HTTP listener");
    let router = runtime.router();
    let stop_http = tokio_util::sync::CancellationToken::new();
    let stopped = stop_http.clone();
    let task = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(stopped.cancelled_owned())
            .await
            .expect("serve shared host");
    });
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .expect("HTTP client");
    let actor = user_token(
        &client,
        &base,
        "administrator",
        management_service::MANAGEMENT_APPLICATION_URI,
    )
    .await;
    let subject = user_token(&client, &base, "unassigned", "idp-services").await;
    let registration = format!("{base}/idp/oauth2/register/human-client");
    expect_status(
        client
            .get(&registration)
            .bearer_auth(&subject)
            .send()
            .await
            .expect("unassigned request"),
        http::StatusCode::FORBIDDEN,
    )
    .await;
    let application = format!("{base}/management/applications/{}", fixture.application_id);
    let permissions = expect_status(
        client
            .get(format!("{application}/permissions"))
            .bearer_auth(&actor)
            .send()
            .await
            .expect("list permissions"),
        http::StatusCode::OK,
    )
    .await;
    let permission = permissions
        .as_array()
        .expect("permissions")
        .iter()
        .find(|permission| permission["name"] == "idp.clients.read")
        .expect("client read permission")["id"]
        .as_str()
        .expect("permission ID");
    let role = expect_status(
        client
            .post(format!("{application}/roles"))
            .bearer_auth(&actor)
            .json(&serde_json::json!({"name": "Shared identity reader"}))
            .send()
            .await
            .expect("create shared role"),
        http::StatusCode::OK,
    )
    .await;
    let role = role["id"].as_str().expect("role ID");
    expect_status(
        client
            .post(format!(
                "{application}/roles/{role}/permissions/{permission}"
            ))
            .bearer_auth(&actor)
            .send()
            .await
            .expect("assign permission"),
        http::StatusCode::OK,
    )
    .await;
    let assignment = format!(
        "{application}/users/{}/roles/{role}",
        fixture.unassigned_user
    );
    expect_status(
        client
            .post(&assignment)
            .bearer_auth(&actor)
            .send()
            .await
            .expect("assign shared role"),
        http::StatusCode::OK,
    )
    .await;
    expect_status(
        client
            .get(&registration)
            .bearer_auth(&subject)
            .send()
            .await
            .expect("IdP sees Management grant immediately"),
        http::StatusCode::OK,
    )
    .await;
    runtime
        .shutdown()
        .await
        .expect("release host without deleting shared rows");
    expect_status(
        client
            .get(&registration)
            .bearer_auth(&subject)
            .send()
            .await
            .expect("retained router still serves shared identity"),
        http::StatusCode::OK,
    )
    .await;
    stop_http.cancel();
    task.await.expect("release shared HTTP connections");
    let mut reopened = unified_server::UnifiedRuntime::build(config)
        .await
        .expect("reopen shared host");
    let base = format!("http://{}", reopened.local_addr());
    let listener = reopened.take_listener().expect("reopened listener");
    let router = reopened.router();
    let task = tokio::spawn(async move {
        axum::serve(listener, router)
            .await
            .expect("serve reopened host");
    });
    let registration = format!("{base}/idp/oauth2/register/human-client");
    expect_status(
        client
            .get(&registration)
            .bearer_auth(&subject)
            .send()
            .await
            .expect("shared grant survives reopen"),
        http::StatusCode::OK,
    )
    .await;
    expect_status(
        client
            .delete(format!(
                "{base}/management/applications/{}/users/{}/roles/{role}",
                fixture.application_id, fixture.unassigned_user
            ))
            .bearer_auth(&actor)
            .send()
            .await
            .expect("revoke shared grant"),
        http::StatusCode::OK,
    )
    .await;
    expect_status(
        client
            .get(&registration)
            .bearer_auth(&subject)
            .send()
            .await
            .expect("IdP sees Management revocation immediately"),
        http::StatusCode::FORBIDDEN,
    )
    .await;
    task.abort();
    let _ = task.await;
    reopened.shutdown().await.expect("release reopened owners");
    fs::remove_dir_all(root).expect("shared identity cleanup");
}
