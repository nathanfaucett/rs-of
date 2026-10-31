#![cfg(unix)]

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

struct MockProvider(Arc<keyring_core::mock::Store>);
impl keyring_core::api::CredentialStoreApi for MockProvider {
    fn vendor(&self) -> String {
        self.0.vendor()
    }
    fn id(&self) -> String {
        self.0.id()
    }
    fn build(
        &self,
        service: &str,
        user: &str,
        _: Option<&std::collections::HashMap<&str, &str>>,
    ) -> keyring_core::Result<keyring_core::Entry> {
        self.0.build(service, user, None)
    }
    fn search(
        &self,
        spec: &std::collections::HashMap<&str, &str>,
    ) -> keyring_core::Result<Vec<keyring_core::Entry>> {
        self.0.search(spec)
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;
use unified_server::{SetupHost, UnifiedConfig};

async fn ready(http: &reqwest::Client, base: &str) -> Value {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let status: Value = http
            .get(format!("{base}/setup/owner/status"))
            .send()
            .await
            .expect("status transport")
            .json()
            .await
            .expect("owner status");
        if status["ready"] == true {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "owner readiness timeout {base}: {status}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn composed_owners_reconcile_create_join_and_offline_reopen() {
    if std::env::var_os("OF_OWNER_LIVE_ROOT").is_none() {
        let root = std::env::temp_dir().join(format!(
            "unified-owner-live-{}",
            idp_model::model::Id::now_v7()
        ));
        let mut roots = Vec::new();
        for name in ["source", "target"] {
            let config =
                UnifiedConfig::local_dynamic(&root.join(name)).expect("isolated configuration");
            let host = SetupHost::prepare(config).expect("local owner prerequisites");
            roots.extend(std::fs::read(host.tls().ca_path()).expect("one CA per local server"));
        }
        let bundle = root.join("roots.pem");
        std::fs::write(&bundle, roots).expect("test trust bundle");
        std::fs::create_dir(root.join("empty-certs")).expect("isolated native root directory");
        let result = std::process::Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "composed_owners_reconcile_create_join_and_offline_reopen",
                "--nocapture",
            ])
            .env("OF_OWNER_LIVE_ROOT", &root)
            .env("SSL_CERT_FILE", &bundle)
            .env("SSL_CERT_DIR", root.join("empty-certs"))
            .output()
            .expect("isolated trust child");
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        std::fs::remove_dir_all(root).expect("isolated cleanup");
        return;
    }
    #[cfg(feature = "cli")]
    let _ = env_logger::Builder::from_env(
        env_logger::Env::default()
            .default_filter_or("warn,idp_server=debug,device_server=debug,storage_server=debug"),
    )
    .try_init();
    let root = std::path::PathBuf::from(
        std::env::var_os("OF_OWNER_LIVE_ROOT").expect("isolated child root"),
    )
    .join("source");
    let mut config = UnifiedConfig::local_dynamic(&root).expect("local configuration");
    keyring_core::set_default_store(Arc::new(MockProvider(
        keyring_core::mock::Store::new().expect("isolated mock provider"),
    )));
    config.idp.password.memory_mib = 1;
    config.idp.password.iterations = 1;
    let host = SetupHost::prepare(config.clone()).expect("owner composition");
    let http = host.tls().client().expect("pin one local CA");
    let base = format!("https://{}", host.local_addr());
    let bases = [
        format!("{base}/idp"),
        format!("{base}/device"),
        format!("{base}/storage"),
    ];
    let codes = ["idp", "device", "storage"].map(|owner| {
        api::SetupAdmission::protected_code(
            host.admission_path(owner)
                .expect("owner path")
                .parent()
                .expect("admission root"),
        )
        .expect("protected code")
        .expect("live capability")
    });
    let stop = CancellationToken::new();
    let task = tokio::spawn(host.serve(stop.clone()));
    for _ in 0..100 {
        if http
            .get(format!("{}/setup/owner/status", bases[0]))
            .send()
            .await
            .is_ok()
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    for (i, owner) in bases.iter().enumerate() {
        for (j, code) in codes.iter().enumerate() {
            let status = http
                .post(format!("{owner}/setup/owner/admission"))
                .header("Authorization", format!("Setup {code}"))
                .send()
                .await
                .expect("admission")
                .status();
            assert_eq!(status.as_u16(), if i == j { 204 } else { 401 });
        }
    }
    let operation = json!({"purpose":"create", "operation_nonce":"0123456789abcdef0123456789abcdef", "administrator":"operator", "node":"Composed Node", "idp":bases[0], "device":bases[1], "storage":bases[2], "issuer":bases[0], "peer":null});
    for (i, owner) in bases.iter().enumerate() {
        let mut body = operation.clone();
        if i == 0 {
            body["password"] = json!("composition-password");
        }
        for _ in 0..2 {
            let response = http
                .post(format!("{owner}/setup/owner/create"))
                .header("Authorization", format!("Setup {}", codes[i]))
                .json(&body)
                .send()
                .await
                .expect("Create");
            assert_eq!(
                response.status().as_u16(),
                202,
                "{}",
                response.text().await.expect("Create response")
            );
        }
        let mut conflict = body;
        conflict["node"] = json!("Conflicting Node");
        assert_eq!(
            http.post(format!("{owner}/setup/owner/create"))
                .header("Authorization", format!("Setup {}", codes[i]))
                .json(&conflict)
                .send()
                .await
                .expect("conflict")
                .status()
                .as_u16(),
            409
        );
    }
    for owner in &bases {
        ready(&http, owner).await;
    }
    let public: Value = http
        .get(format!("{}/setup/owner/public", bases[0]))
        .send()
        .await
        .expect("public facts")
        .json()
        .await
        .expect("public document");
    assert_eq!(public["operation"], operation);
    let endpoint = public["facts"]["endpoint"].clone();
    let login = public["facts"]["login_client"]
        .as_str()
        .expect("public login Client");
    let token = http
        .post(format!("{}/oauth2/token", bases[0]))
        .form(&[
            ("grant_type", "password"),
            ("client_id", login),
            ("username", "operator"),
            ("password", "composition-password"),
            ("scope", "openid"),
        ])
        .send()
        .await
        .expect("operator login");
    assert_eq!(token.status().as_u16(), 200);
    let session: Value = token.json().await.expect("login response");
    assert_eq!(
        http.get(format!("{base}/management/health"))
            .send()
            .await
            .expect("configured Management participates")
            .status()
            .as_u16(),
        200
    );
    for (i, owner) in bases.iter().enumerate() {
        assert_eq!(
            http.post(format!("{owner}/setup/owner/admission"))
                .header("Authorization", format!("Setup {}", codes[i]))
                .send()
                .await
                .expect("retired admission")
                .status()
                .as_u16(),
            401
        );
    }
    stop.cancel();
    task.await.expect("host task").expect("coordinated close");
    let reopened = SetupHost::prepare(config).expect("existing-only owner reopen");
    let stop = CancellationToken::new();
    let task = tokio::spawn(reopened.serve(stop.clone()));
    for owner in &bases {
        ready(&http, owner).await;
    }
    let retained: Value = http
        .get(format!("{}/setup/owner/public", bases[0]))
        .send()
        .await
        .expect("retained public")
        .json()
        .await
        .expect("retained facts");
    assert_eq!(retained["operation"], operation);
    assert_eq!(retained["facts"]["endpoint"], endpoint);
    let authority: Value = http
        .post(format!("{}/oauth2/token", bases[0]))
        .form(&[
            (
                "grant_type",
                "urn:ietf:params:oauth:grant-type:token-exchange",
            ),
            ("client_id", login),
            (
                "subject_token",
                session["access_token"].as_str().expect("operator token"),
            ),
            (
                "subject_token_type",
                "urn:ietf:params:oauth:token-type:access_token",
            ),
            ("resource", "idp-management"),
        ])
        .send()
        .await
        .expect("source authority exchange")
        .json()
        .await
        .expect("authority token");
    let bearer = authority["access_token"]
        .as_str()
        .expect("current source authority");
    assert_eq!(
        http.put(format!("{}/independent/node-joins/acceptance", bases[1]))
            .bearer_auth(bearer)
            .json(&json!({"accepting":true}))
            .send()
            .await
            .expect("source acceptance")
            .status()
            .as_u16(),
        204
    );
    let target_root = root.parent().expect("fixture root").join("target");
    let mut target_config = UnifiedConfig::local_dynamic(&target_root).expect("target config");
    target_config.idp.password.memory_mib = 1;
    target_config.idp.password.iterations = 1;
    let target = SetupHost::prepare(target_config.clone()).expect("target owners");
    let target_base = format!("https://{}", target.local_addr());
    let target_codes = ["idp", "device", "storage"].map(|owner| {
        api::SetupAdmission::protected_code(
            target
                .admission_path(owner)
                .expect("target owner")
                .parent()
                .expect("admission root"),
        )
        .expect("protected target code")
        .expect("target code")
    });
    let target_stop = CancellationToken::new();
    let target_task = tokio::spawn(target.serve(target_stop.clone()));
    let join = json!({"purpose":"join", "operation_nonce":"fedcba9876543210fedcba9876543210", "administrator":"", "node":"Joined Node", "idp":format!("{target_base}/idp"), "device":format!("{target_base}/device"), "storage":format!("{target_base}/storage"), "issuer":bases[0], "peer":format!("{}#{}",bases[1], endpoint.as_str().expect("source endpoint"))});
    for (i, owner) in ["idp", "device", "storage"].iter().enumerate() {
        let response = http
            .post(format!("{target_base}/{owner}/setup/owner/join"))
            .header("Authorization", format!("Setup {}", target_codes[i]))
            .json(&join)
            .send()
            .await
            .expect("owner Join");
        assert_eq!(
            response.status().as_u16(),
            202,
            "{}",
            response.text().await.expect("Join response")
        );
    }
    let deadline = Instant::now() + Duration::from_secs(60);
    let offer = loop {
        let offers: Vec<Value> = http
            .get(format!("{}/independent/node-joins/pending", bases[1]))
            .bearer_auth(bearer)
            .send()
            .await
            .expect("source pending offers")
            .json()
            .await
            .expect("pending offers");
        if let Some(offer) = offers.into_iter().next() {
            break offer;
        }
        assert!(Instant::now() < deadline, "Join offer timeout");
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    assert_eq!(offer["request"]["operation"], join);
    let offer_id = offer["offer_id"].as_str().expect("exact offer ID");
    assert_eq!(
        http.post(format!(
            "{}/independent/node-joins/{offer_id}/approve",
            bases[1]
        ))
        .bearer_auth(bearer)
        .send()
        .await
        .expect("exact source approval")
        .status()
        .as_u16(),
        202
    );
    for owner in ["idp", "device", "storage"] {
        ready(&http, &format!("{target_base}/{owner}")).await;
    }
    stop.cancel();
    task.await.expect("source task").expect("source close");
    target_stop.cancel();
    target_task
        .await
        .expect("target task")
        .expect("target close");
    let target =
        SetupHost::prepare(target_config).expect("source-offline existing-only Join reopen");
    let target_stop = CancellationToken::new();
    let target_task = tokio::spawn(target.serve(target_stop.clone()));
    for owner in ["idp", "device", "storage"] {
        ready(&http, &format!("{target_base}/{owner}")).await;
        let public: Value = http
            .get(format!("{target_base}/{owner}/setup/owner/public"))
            .send()
            .await
            .expect("offline public facts")
            .json()
            .await
            .expect("offline retained operation");
        assert_eq!(public["operation"], join);
    }
    let discovery: Value = http
        .get(format!(
            "{target_base}/idp/.well-known/openid-configuration"
        ))
        .send()
        .await
        .expect("offline discovery")
        .json()
        .await
        .expect("offline metadata");
    assert_eq!(discovery["issuer"], bases[0]);
    let permission = json!({
        "requestId":"01234567-89ab-4cde-8fab-0123456789ab",
        "action":"devices_read",
        "subject":{"type":"user", "id":public["facts"]["user"]},
        "target":{"scope":"installation", "resource":{"type":"device", "id":null}}
    });
    let response = http
        .post(format!("{target_base}/management/permissions/evaluate"))
        .bearer_auth(bearer)
        .json(&permission)
        .send()
        .await
        .expect("offline Management authorization transport");
    assert_eq!(response.status().as_u16(), 200);
    let decision: Value = response.json().await.expect("offline permission decision");
    assert_eq!(decision["allowed"], true);
    assert_eq!(decision["request"], permission);
    target_stop.cancel();
    target_task
        .await
        .expect("offline target task")
        .expect("offline close");
}
