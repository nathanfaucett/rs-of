use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use tokio::runtime::Builder;
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;

use super::{CreateRecord, NodeRuntime};

const BASE: &str = "https://node.test";
const IDP: &str = "https://node.test/idp";
const DEVICE: &str = "https://node.test/device";
const PASSWORD: &str = "test-operator-password";

#[test]
fn create_allows_only_storage_to_introspect_storage_sync() {
    run_on_node_runtime(check_storage_sync_introspection());
}

pub fn run_on_node_runtime(test: impl Future<Output = ()> + Send + 'static) {
    Builder::new_multi_thread()
        .worker_threads(1)
        .thread_stack_size(64 * 1024 * 1024)
        .enable_all()
        .build()
        .expect("Node test runtime")
        .block_on(async {
            tokio::spawn(test).await.expect("Node runtime test");
        });
}

async fn check_storage_sync_introspection() {
    let root = tempfile::tempdir().expect("fresh Node root");
    let mut record = CreateRecord::new("node-operator".into()).expect("Create record");
    let shutdown = CancellationToken::new();
    for password in [Some(PASSWORD), None] {
        let runtime = NodeRuntime {
            root: root.path().into(),
            base: BASE.into(),
            ui: "https://app.node.test".into(),
            pm: "https://password-manager.node.test".into(),
            client: reqwest::Client::new(),
            database: db::EmbeddedDatabase::open(root.path())
                .await
                .expect("open Node database"),
        };
        let router = runtime
            .build(&mut record, password, &shutdown, &shutdown)
            .await
            .expect("start Create runtime");
        let login = record.login.to_string();
        let user = token(
            &router,
            &[
                ("grant_type", "password"),
                ("client_id", &login),
                ("username", "node-operator"),
                ("password", PASSWORD),
                ("scope", "openid"),
            ],
        )
        .await;
        let sync = exchange(&router, &login, &user, "storage:sync").await;
        let storage = service_token(&router, &record, 2).await;
        let (status, body) = introspect(&router, &storage, &sync).await;
        assert_eq!(status, StatusCode::OK, "Storage sync introspection: {body}");
        assert_eq!(body["claims"]["aud"], "storage:sync");
        assert_eq!(body["claims"]["resource"], "storage:sync");
        assert_eq!(body["claims"]["principal_type"], "user");

        let control = service_token(&router, &record, 1).await;
        assert_eq!(
            introspect(&router, &control, &sync).await.0,
            StatusCode::UNAUTHORIZED
        );
        let device = exchange(&router, &login, &user, DEVICE).await;
        assert_eq!(
            introspect(&router, &storage, &device).await.0,
            StatusCode::UNAUTHORIZED
        );
    }
    shutdown.cancel();
}

async fn exchange(router: &Router, login: &str, user: &str, audience: &str) -> String {
    token(
        router,
        &[
            (
                "grant_type",
                "urn:ietf:params:oauth:grant-type:token-exchange",
            ),
            ("client_id", login),
            ("subject_token", user),
            (
                "subject_token_type",
                "urn:ietf:params:oauth:token-type:access_token",
            ),
            ("resource", audience),
        ],
    )
    .await
}

async fn service_token(router: &Router, record: &CreateRecord, slot: usize) -> String {
    let (id, secret) = &record.clients[slot];
    token(
        router,
        &[
            ("grant_type", "client_credentials"),
            ("client_id", &id.to_string()),
            ("client_secret", secret),
            ("scope", "idp.token.validate"),
            ("audience", IDP),
        ],
    )
    .await
}

async fn token(router: &Router, form: &[(&str, &str)]) -> String {
    let mut url = reqwest::Url::parse(IDP).expect("IdP URL");
    url.query_pairs_mut().extend_pairs(form);
    let (status, body) = send(
        router,
        Request::post("/idp/oauth2/token")
            .header("content-type", "application/x-www-form-urlencoded")
            .body(Body::from(url.query().expect("form data").to_owned()))
            .expect("token request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["access_token"].as_str().expect("access token").into()
}

async fn introspect(router: &Router, caller: &str, inspected: &str) -> (StatusCode, Value) {
    send(
        router,
        Request::post("/idp/oauth2/introspect")
            .header("authorization", format!("Bearer {caller}"))
            .header("content-type", "application/json")
            .body(Body::from(json!({ "token": inspected }).to_string()))
            .expect("introspection request"),
    )
    .await
}

async fn send(router: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = router.clone().oneshot(request).await.expect("response");
    let status = response.status();
    let body = to_bytes(response.into_body(), 32 * 1024)
        .await
        .expect("response body");
    (
        status,
        serde_json::from_slice(&body).expect("JSON response"),
    )
}
