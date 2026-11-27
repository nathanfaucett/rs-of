use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::{Next, from_fn},
    response::IntoResponse,
};
use device_service::ofdb::DbSelectionPolicyRepo;
use of_model::{
    contract::{replication::MANAGEMENT_REPLICATION_READ_SCOPE, token_response::TokenResponse},
    device::selection_policy::SelectionPolicy,
    id::Id,
    idp::contract::service_scopes::IDP_TOKEN_VALIDATE_SCOPE,
};
use storage_server::{DeviceClient, IdpClient, IdpIntrospectionError};
use tokio::{net::TcpListener, runtime::Builder, time::timeout};
use tokio_util::sync::CancellationToken;

use super::{CreateRecord, NodeRuntime};

#[test]
fn non_seeded_selection_snapshot_does_not_renew_credentials_under_shared_writer() {
    Builder::new_multi_thread()
        .worker_threads(2)
        .thread_stack_size(64 * 1024 * 1024)
        .enable_all()
        .build()
        .expect("Node runtime")
        .block_on(async { tokio::spawn(check_snapshot()).await.expect("Node test") });
}

#[allow(
    clippy::too_many_lines,
    reason = "keep the cold-to-warm authorization sequence together"
)]
#[allow(
    clippy::significant_drop_tightening,
    reason = "commit consumes and releases the writer"
)]
async fn check_snapshot() {
    let root = tempfile::tempdir().expect("Node root");
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind Node");
    let base = format!("http://{}", listener.local_addr().expect("Node address"));
    let idp = format!("{base}/idp");
    let device = format!("{base}/device");
    let storage = format!("{base}/storage");
    let client = reqwest::Client::new();
    let runtime = NodeRuntime {
        root: root.path().into(),
        base,
        ui: "https://app.node.test".into(),
        pm: "https://password-manager.node.test".into(),
        client: client.clone(),
        database: db::EmbeddedDatabase::open(root.path())
            .await
            .expect("shared Node database"),
    };
    let mut record = CreateRecord::new("node-operator".into()).expect("Create record");
    let shutdown = CancellationToken::new();
    let writer_held = Arc::new(AtomicBool::new(false));
    let renewal_attempts = Arc::new(AtomicUsize::new(0));
    let held = Arc::clone(&writer_held);
    let attempts = Arc::clone(&renewal_attempts);
    let router = Box::pin(runtime.build(
        &mut record,
        Some("test-operator-password"),
        &shutdown,
        &shutdown,
    ))
    .await
    .expect("start non-seeded Node")
    .layer(from_fn(move |request: Request, next: Next| {
        let held = Arc::clone(&held);
        let attempts = Arc::clone(&attempts);
        async move {
            if held.load(Ordering::SeqCst) && request.uri().path() == "/idp/oauth2/token" {
                attempts.fetch_add(1, Ordering::SeqCst);
                return StatusCode::SERVICE_UNAVAILABLE.into_response();
            }
            next.run(request).await
        }
    }));
    let server =
        tokio::spawn(async move { axum::serve(listener, router).await.expect("serve Node") });
    let endpoint = iroh::SecretKey::from_bytes(&record.endpoint)
        .public()
        .to_string();
    let (id, secret) = &record.clients[3];
    let service = service_token(
        &client,
        &idp,
        &record.clients[3],
        MANAGEMENT_REPLICATION_READ_SCOPE,
        &storage,
    )
    .await;
    let (identity_client, identity_secret) = &record.clients[2];
    let storage_idp = IdpClient::new(
        &idp,
        &identity_client.to_string(),
        identity_secret,
        &idp,
        &idp,
    )
    .expect("Storage IdP client");
    let identity_token = service_token(
        &client,
        &idp,
        &record.clients[2],
        IDP_TOKEN_VALIDATE_SCOPE,
        &idp,
    )
    .await;
    let application = Id::now_v7();
    let resource = Id::now_v7();
    DbSelectionPolicyRepo::new(runtime.database.client())
        .set_prevalidated(SelectionPolicy {
            device_id: record.device.expect("provisioned Device"),
            owner_subject: record.user.to_string(),
            application_id: Some(application),
            selected_kind: Some("database".into()),
            selected_id: Some(resource),
            admin_allowed: true,
        })
        .await
        .expect("select the Resource");
    let host = runtime.database.storage();
    let writer = host
        .namespace("publish-test")
        .expect("writer namespace")
        .database();
    let storage_device = DeviceClient::new(&device, &idp, &id.to_string(), secret, &idp, &storage)
        .expect("Storage Device client");
    for warm in [false, true] {
        if warm {
            storage_idp
                .introspect(&identity_token)
                .await
                .expect("ordinary IdP check warms credentials");
            let selection = storage_device
                .selected_resources(&endpoint)
                .await
                .expect("ordinary Selection warms credentials");
            assert_eq!(selection.owner_subject, record.user.to_string());
            assert!(selection.admin_allowed);
        }
        let transaction = writer.transaction().await.expect("publish transaction");
        transaction
            .create_table::<u64, u64>(&format!("publication-{warm}"))
            .expect("force physical writer");
        transaction
            .commit_checked(|| async {
                writer_held.store(true, Ordering::SeqCst);
                timeout(Duration::from_secs(5), async {
                    let inspected = storage_idp
                        .inspect_identity_read_only(&identity_token)
                        .await;
                    if warm {
                        assert_eq!(
                            inspected
                                .expect("current IdP authority under writer")
                                .claims
                                .aud,
                            idp
                        );
                        let selection = storage_device
                            .selection_snapshot(&endpoint)
                            .await
                            .expect("current Selection under writer");
                        assert_eq!(selection.owner_subject, record.user.to_string());
                        assert!(selection.admin_allowed);
                        let [selected] = selection.resources.as_slice() else {
                            panic!("expected the exact Resource Selection");
                        };
                        assert_eq!(selected.application_id, application.to_string());
                        assert_eq!(selected.resource_id, resource.to_string());
                    } else {
                        assert!(matches!(
                            inspected,
                            Err(IdpIntrospectionError::ServiceUnavailable)
                        ));
                        let response = client
                            .get(format!(
                                "{device}/replication/selection-snapshots/{endpoint}"
                            ))
                            .bearer_auth(&service)
                            .send()
                            .await
                            .expect("cold snapshot response");
                        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
                    }
                })
                .await
                .expect("snapshot must not wait for the writer");
                writer_held.store(false, Ordering::SeqCst);
                assert_eq!(
                    renewal_attempts.load(Ordering::SeqCst),
                    0,
                    "publish must not invoke token issuance"
                );
                Ok::<(), String>(())
            })
            .await
            .expect("commit writer")
            .expect("read-only authorization");
    }
    shutdown.cancel();
    server.abort();
}

async fn service_token(
    client: &reqwest::Client,
    idp: &str,
    (id, secret): &(Id, String),
    scope: &str,
    audience: &str,
) -> String {
    client
        .post(format!("{idp}/oauth2/token"))
        .form(&[
            ("grant_type", "client_credentials"),
            ("client_id", &id.to_string()),
            ("client_secret", secret),
            ("scope", scope),
            ("audience", audience),
        ])
        .send()
        .await
        .expect("service token response")
        .error_for_status()
        .expect("issue service token")
        .json::<TokenResponse>()
        .await
        .expect("service token body")
        .access_token
        .0
}
