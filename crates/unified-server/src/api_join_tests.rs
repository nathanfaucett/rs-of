use std::{collections::HashMap, fs, sync::Arc, time::Duration};

use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use super::{Fixture, HttpsHost, exchange, pending_http, token};
use crate::{
    BootstrapConfig, LocalProvisioner, LocalTls, SetupHost, local_provisioning::provisioning_stage,
};

#[derive(Clone, Copy)]
enum Case {
    Interrupted,
    Offline,
    Finalization,
    Readiness,
}

impl Case {
    fn selected() -> Self {
        match std::env::var("OF_TEST_API_JOIN")
            .expect("parent API case")
            .as_str()
        {
            "interrupted" => Self::Interrupted,
            "offline" => Self::Offline,
            "finalization" => Self::Finalization,
            "readiness" => Self::Readiness,
            _ => panic!("unknown API Join test case"),
        }
    }
}

pub(super) async fn from_source(
    a: &Fixture,
    b: &mut Fixture,
    tls: &LocalTls,
    target_tls: LocalTls,
) {
    let listener = tokio::net::TcpListener::bind(a.config.listen_addr)
        .await
        .expect("one source host listener");
    let mut local =
        LocalProvisioner::create(a.config.clone(), "operator", "HTTPS source", &a.password)
            .expect("existing production source fixture");
    let preparation = CancellationToken::new();
    let runtime = provisioning_stage(|| {
        local.prepare_hosted_runtime("fixture-initial-password", true, &preparation)
    })
    .await
    .expect("prepare real source owners once");
    if matches!(Case::selected(), Case::Readiness) {
        let role = a.journal()["role"]
            .as_str()
            .expect("initial role")
            .parse()
            .expect("role ID");
        crate::api_create::tests::status_tracks_revocation_in_existing_runtime(
            runtime,
            local,
            tls,
            &a.root.join("readiness-host"),
            role,
        )
        .await;
        return;
    }
    let login = local
        .login_client()
        .expect("source public login")
        .client_id
        .expect("source Client ID");
    let source = HttpsHost::start_on(runtime, tls, Some(listener)).await;
    local
        .mark_complete()
        .expect("source completion after hosted readiness");
    let http = LocalTls::verification_client().expect("isolated native HTTPS client");
    let session = token(
        &http,
        &source.base,
        &[
            ("grant_type", "password"),
            ("client_id", &login),
            ("username", "operator"),
            ("password", "fixture-initial-password"),
            ("scope", "openid"),
        ],
    )
    .await;
    let approving = exchange(
        &http,
        &source.base,
        &login,
        &session.access_token.0,
        management_service::MANAGEMENT_APPLICATION_URI,
    )
    .await;
    let idp_only = exchange(
        &http,
        &source.base,
        &login,
        &session.access_token.0,
        a.config.idp_audience(),
    )
    .await;
    assert_eq!(
        http.put(format!("{}/management/node-joins/acceptance", source.base))
            .bearer_auth(&idp_only)
            .json(&json!({"accepting": true}))
            .send()
            .await
            .expect("wrong-audience source Join request")
            .status(),
        StatusCode::FORBIDDEN,
        "an IdP-audience session cannot authorize Management Source Join"
    );
    assert_eq!(
        http.get(format!("{}/idp/node-joins/pending", source.base))
            .bearer_auth(&approving)
            .send()
            .await
            .expect("retired IdP Source Join route")
            .status(),
        StatusCode::NOT_FOUND
    );
    let storage = exchange(
        &http,
        &source.base,
        &login,
        &session.access_token.0,
        "storage:filesystems",
    )
    .await;
    assert_eq!(
        http.put(format!("{}/management/node-joins/acceptance", source.base))
            .bearer_auth(&approving)
            .json(&json!({"accepting": true}))
            .send()
            .await
            .expect("authorized source acceptance")
            .status(),
        StatusCode::OK
    );
    b.config.idp.oauth2.issuer = a.config.idp.oauth2.issuer.clone();
    run(
        b,
        target_tls,
        source,
        Some(local),
        &http,
        &approving,
        &storage,
    )
    .await;
}

pub(super) fn credential_store(
    nodes: Arc<keyring_core::CredentialStore>,
    relationships: Arc<keyring_core::CredentialStore>,
) -> Arc<keyring_core::CredentialStore> {
    Arc::new(JoinStore {
        nodes,
        relationships,
    })
}

struct JoinStore {
    nodes: Arc<keyring_core::CredentialStore>,
    relationships: Arc<keyring_core::CredentialStore>,
}

impl keyring_core::api::CredentialStoreApi for JoinStore {
    fn vendor(&self) -> String {
        "isolated-api-join-stores".into()
    }
    fn id(&self) -> String {
        self.vendor()
    }
    fn build(
        &self,
        service: &str,
        user: &str,
        modifiers: Option<&HashMap<&str, &str>>,
    ) -> keyring_core::Result<keyring_core::Entry> {
        let store = if service == "local.unified-service-credentials" {
            &self.relationships
        } else {
            &self.nodes
        };
        keyring_core::api::CredentialStoreApi::build(&**store, service, user, modifiers)
    }
    fn search(&self, spec: &HashMap<&str, &str>) -> keyring_core::Result<Vec<keyring_core::Entry>> {
        let mut entries = self.nodes.search(spec)?;
        entries.extend(self.relationships.search(spec)?);
        Ok(entries)
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub(super) async fn run(
    fixture: &mut Fixture,
    tls: LocalTls,
    source: HttpsHost,
    source_guard: Option<LocalProvisioner>,
    http: &Client,
    approving: &str,
    storage: &str,
) {
    let case = Case::selected();
    let expected_issuer = fixture.config.idp.oauth2.issuer.clone();
    let address = fixture.config.listen_addr;
    let base = format!("https://{address}");
    let mut config = crate::api_create_tests::fixture_config(&fixture.root, address);
    config.idp.key_namespace = fixture.config.idp.key_namespace.clone();
    config.idp.oauth2.issuer = expected_issuer.clone();
    config.idp.service_audience = Some(config.idp.api_public_uri.clone());

    // The subprocess's native roots already contain this isolated neutral CA.
    let tls_directory = LocalTls::directory(&config);
    let staged_tls = fixture.root.join("parent-tls");
    fs::rename(&tls_directory, &staged_tls).expect("stage isolated parent TLS");
    let mut host = BootstrapConfig::create_explicit(config.clone()).expect("neutral API host");
    assert!(host.config().idp.oauth2.issuer.is_empty());
    host.begin_setup()
        .expect("retain neutral prerequisite production history");
    let admission_root = host.directory().join("setup-admission");
    drop(api::SetupAdmission::open(&admission_root).expect("host-generated protected capability"));
    fs::rename(staged_tls, &tls_directory).expect("retain exact neutral CA");
    let setup = SetupHost::open(host).expect("existing-only neutral API startup");
    assert_eq!(setup.tls().ca_path(), tls.ca_path());
    let ca_before = fs::read(tls.ca_path()).expect("neutral CA bytes");
    let admission_path = setup.admission_path();
    let initial = fs::read_to_string(&admission_path).expect("protected host management");
    let code = initial.lines().nth(1).expect("host code");
    let mut cancellation = CancellationToken::new();
    let mut server = tokio::spawn(setup.serve_verified(http.clone(), cancellation.clone(), false));
    wait_state(http, &base, "awaiting_choice").await;

    let body = json!({
        "operation_nonce": "0123456789abcdef0123456789abcdef",
        "node": "API joining Node", "peer": source.endpoint.to_string(),
    });
    for supplied in [None, Some("invalid"), Some(approving)] {
        assert_eq!(
            post(http, &base, "join", supplied, Some(&body))
                .await
                .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    assert!(!config.credential_bootstrap_directory().exists());
    assert!(!config.local_provisioning_directory().exists());
    assert!(
        BootstrapConfig::open_explicit(config.clone()).is_err(),
        "serving host owns lifecycle guard"
    );
    assert_eq!(
        http.post(format!("{base}/idp/setup/join"))
            .header("Authorization", format!("Setup {code}"))
            .header("Origin", "https://untrusted.example")
            .json(&body)
            .send()
            .await
            .expect("origin refusal")
            .status(),
        StatusCode::FORBIDDEN
    );
    let mut forged = body.clone();
    forged["issuer"] = json!("https://caller-chosen.invalid");
    assert_eq!(
        post(http, &base, "join", Some(code), Some(&forged))
            .await
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let mut malformed = body.clone();
    malformed["peer"] = json!("not-an-authenticated-peer");
    assert_eq!(
        post(http, &base, "join", Some(code), Some(&malformed))
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        post(http, &base, "intent", Some(code), None).await.status(),
        StatusCode::NO_CONTENT
    );

    let accepted = post(http, &base, "join", Some(code), Some(&body)).await;
    assert_eq!(accepted.status(), StatusCode::ACCEPTED);
    assert_eq!(
        accepted.json::<Value>().await.expect("accepted Join")["ready"],
        false
    );
    let bound = fs::read(&admission_path).expect("durable admission before 202");
    let recovered = post(http, &base, "intent", Some(code), None)
        .await
        .json::<Value>()
        .await
        .expect("protected exact Join recovery");
    assert_eq!(
        recovered,
        json!({
            "operation_nonce": body["operation_nonce"], "node": body["node"],
            "peer": body["peer"], "issuer": expected_issuer,
        })
    );
    assert_eq!(
        post(http, &base, "intent", None, None).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        post(http, &base, "join", None, Some(&body)).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        post(http, &base, "join", Some(code), Some(&body))
            .await
            .status(),
        StatusCode::ACCEPTED
    );
    let mut competing = body.clone();
    competing["peer"] = json!(iroh::SecretKey::generate().public().to_string());
    assert_eq!(
        post(http, &base, "join", Some(code), Some(&competing))
            .await
            .status(),
        StatusCode::CONFLICT
    );
    let create = json!({"operation_nonce": body["operation_nonce"], "administrator": "operator", "node": body["node"], "password": "fixture-initial-password"});
    assert_eq!(
        post(http, &base, "create", Some(code), Some(&create))
            .await
            .status(),
        StatusCode::CONFLICT
    );

    let first = pending_node(http, &source, approving).await;
    assert_eq!(first["request"]["installation"], expected_issuer);
    let pending = if matches!(case, Case::Interrupted) {
        assert_eq!(
            post(http, &base, "cancel", None, Some(&recovered))
                .await
                .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            post(http, &base, "cancel", Some(code), Some(&recovered))
                .await
                .status(),
            StatusCode::NO_CONTENT
        );
        wait_state(http, &base, "unfinished").await;
        source.assert_offers_released().await;
        let stale = first["offer_id"].as_str().expect("first offer ID");
        assert_eq!(
            http.post(format!(
                "{}/management/node-joins/{stale}/approve",
                source.base
            ))
            .bearer_auth(approving)
            .send()
            .await
            .expect("cancelled source offer")
            .status(),
            StatusCode::NOT_FOUND
        );
        cancellation.cancel();
        server
            .await
            .expect("cancelled host task")
            .expect("cancelled host cleanup");

        let host = BootstrapConfig::open_explicit(config.clone()).expect("retained Join host");
        assert!(matches!(host.purpose(), crate::BootstrapPurpose::Join));
        assert_eq!(host.config().idp.oauth2.issuer, expected_issuer);
        assert_eq!(host.trusted_peer(), body["peer"].as_str());
        let setup = SetupHost::open(host).expect("incomplete Join API reopen");
        cancellation = CancellationToken::new();
        server = tokio::spawn(setup.serve_verified(http.clone(), cancellation.clone(), false));
        wait_state(http, &base, "unfinished").await;
        assert_eq!(
            post(http, &base, "intent", Some(code), None)
                .await
                .json::<Value>()
                .await
                .expect("retained Join after restart"),
            recovered
        );
        assert_eq!(
            post(http, &base, "join", Some(code), Some(&body))
                .await
                .status(),
            StatusCode::ACCEPTED
        );
        let pending = pending_node(http, &source, approving).await;
        assert_eq!(
            pending["request"], first["request"],
            "exact retry retains Node, signer and endpoint"
        );
        let rejected = pending["offer_id"].as_str().expect("rejected offer ID");
        assert_eq!(
            http.post(format!(
                "{}/management/node-joins/{rejected}/reject",
                source.base
            ))
            .bearer_auth(approving)
            .send()
            .await
            .expect("ordinary source rejection")
            .status(),
            StatusCode::NO_CONTENT
        );
        let refused = wait_state(http, &base, "unfinished").await;
        assert_eq!(
            refused["ready"], false,
            "rejected membership never completes target"
        );
        source.assert_offers_released().await;
        assert_eq!(
            post(http, &base, "join", Some(code), Some(&body))
                .await
                .status(),
            StatusCode::ACCEPTED
        );
        let pending = pending_node(http, &source, approving).await;
        assert_eq!(
            pending["request"], first["request"],
            "rejection does not replace retained identity"
        );
        pending
    } else {
        first
    };
    let offer = pending["offer_id"].as_str().expect("new exact offer ID");
    let approval = format!("{}/management/node-joins/{offer}/approve", source.base);
    assert_eq!(
        http.post(&approval)
            .header("Authorization", format!("Setup {code}"))
            .send()
            .await
            .expect("target capability is not source User authority")
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        http.post(&approval)
            .bearer_auth("forged-bearer")
            .send()
            .await
            .expect("forged source approval")
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        http.post(&approval)
            .bearer_auth(approving)
            .send()
            .await
            .expect("ordinary authorized source approval")
            .status(),
        StatusCode::OK
    );
    let ready = wait_state(http, &base, "ready").await;
    assert_eq!(ready["ready"], true);
    let login = ready["login_client"].clone();
    assert_eq!(
        fs::read(tls.ca_path()).expect("retained CA after Join"),
        ca_before
    );
    authenticated_storage(http, &base, storage).await;
    for operation in ["intent", "join", "cancel"] {
        let payload = match operation {
            "join" => Some(&body),
            "cancel" => Some(&recovered),
            _ => None,
        };
        assert_eq!(
            post(http, &base, operation, Some(code), payload)
                .await
                .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    let endpoint_before = fs::read(config.endpoint_data_dir.join("endpoint.key"))
        .expect("retained local endpoint identity");
    source.stop().await;
    drop(source_guard);
    cancellation.cancel();
    server
        .await
        .expect("completed host task")
        .expect("completed host cleanup");

    if matches!(case, Case::Interrupted) {
        return;
    }

    if matches!(case, Case::Finalization) {
        // Simulate a crash after owner completion but before admission retirement.
        fs::write(&admission_path, &bound)
            .expect("restore isolated pre-retirement crash checkpoint");
        let setup = SetupHost::open(
            BootstrapConfig::open_explicit(config.clone()).expect("complete unretired host"),
        )
        .expect("explicit finalization startup");
        cancellation = CancellationToken::new();
        server = tokio::spawn(setup.serve_verified(http.clone(), cancellation.clone(), false));
        let unfinished = wait_state(http, &base, "unfinished").await;
        assert_eq!(unfinished["ready"], false);
        assert_eq!(
            post(http, &base, "intent", Some(code), None)
                .await
                .json::<Value>()
                .await
                .expect("exact offline finalization intent"),
            recovered
        );
        assert_eq!(
            post(http, &base, "join", None, Some(&body)).await.status(),
            StatusCode::UNAUTHORIZED
        );
        let mut conflict = body.clone();
        conflict["node"] = json!("Different Node");
        assert_eq!(
            post(http, &base, "join", Some(code), Some(&conflict))
                .await
                .status(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            post(http, &base, "join", Some(code), Some(&body))
                .await
                .status(),
            StatusCode::ACCEPTED
        );
        assert_eq!(
            wait_state(http, &base, "ready").await["login_client"],
            login
        );
        authenticated_storage(http, &base, storage).await;
        cancellation.cancel();
        server
            .await
            .expect("finalized host task")
            .expect("finalized host cleanup");
        assert_eq!(
            fs::read(config.endpoint_data_dir.join("endpoint.key")).expect("finalized endpoint"),
            endpoint_before
        );
        return;
    }

    let setup = SetupHost::open(
        BootstrapConfig::open_explicit(config.clone()).expect("completed offline host"),
    )
    .expect("offline retained TLS and admission");
    cancellation = CancellationToken::new();
    server = tokio::spawn(setup.serve_verified(http.clone(), cancellation.clone(), false));
    assert_eq!(
        wait_state(http, &base, "ready").await["login_client"],
        login
    );
    authenticated_storage(http, &base, storage).await;
    assert_eq!(
        fs::read(config.endpoint_data_dir.join("endpoint.key")).expect("offline endpoint"),
        endpoint_before
    );
    assert_eq!(
        post(http, &base, "join", Some(code), Some(&body))
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    cancellation.cancel();
    server
        .await
        .expect("offline host task")
        .expect("offline coordinated cleanup");
}

async fn post(
    http: &Client,
    base: &str,
    operation: &str,
    code: Option<&str>,
    body: Option<&Value>,
) -> reqwest::Response {
    let mut request = http.post(format!("{base}/idp/setup/{operation}"));
    if let Some(code) = code {
        request = request.header("Authorization", format!("Setup {code}"));
    }
    if let Some(body) = body {
        request = request.json(body);
    }
    request
        .send()
        .await
        .expect("production setup HTTPS request")
}

async fn wait_state(http: &Client, base: &str, expected: &str) -> Value {
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if let Ok(response) = http.get(format!("{base}/idp/setup/status")).send().await {
                let status = response.json::<Value>().await.expect("setup status JSON");
                if status["state"] == expected {
                    return status;
                }
                assert_ne!(
                    status["state"], "unavailable",
                    "retained state must remain usable"
                );
                if expected == "ready" {
                    assert_ne!(
                        status["state"], "unfinished",
                        "host-owned Join stopped unfinished while awaiting ready; inspect bootstrap stage diagnostics"
                    );
                }
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("bounded host-owned setup observation")
}

async fn pending_node(http: &Client, source: &HttpsHost, approving: &str) -> Value {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let offers = pending_http(http, &source.base, approving).await;
            if let Some(offer) = offers
                .as_array()
                .expect("source offers")
                .iter()
                .find(|offer| offer["request"]["name"] == "API joining Node")
            {
                return offer.clone();
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("real Iroh offer reaches authorized source API")
}

async fn authenticated_storage(http: &Client, base: &str, bearer: &str) {
    assert_eq!(
        http.get(format!("{base}/storage/filesystems"))
            .send()
            .await
            .expect("unauthenticated Storage")
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        http.get(format!("{base}/storage/filesystems"))
            .bearer_auth(bearer)
            .send()
            .await
            .expect("ordinary authenticated Storage")
            .status(),
        StatusCode::OK
    );
}
