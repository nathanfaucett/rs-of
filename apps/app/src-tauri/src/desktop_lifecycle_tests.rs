use std::{fs, io, path::PathBuf};

use unified_server::{LocalTls, SetupHost};

use super::NativeHost;

fn directory() -> PathBuf {
    std::env::temp_dir().join(format!("native-shared-host-{}", uuid::Uuid::new_v4()))
}

fn fixture(root: &std::path::Path) -> (SetupHost, reqwest::Client) {
    let setup = super::prepare(root).expect("fresh neutral native host");
    let ca = fs::read(setup.tls().ca_path()).expect("isolated CA");
    let client = reqwest::Client::builder()
        .use_rustls_tls()
        .tls_built_in_root_certs(false)
        .add_root_certificate(reqwest::Certificate::from_pem(&ca).expect("fixture CA"))
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("fixture-only HTTPS verification");
    (setup, client)
}

#[tokio::test]
async fn native_host_keeps_one_listener_and_shared_routes_until_coordinated_close() {
    let root = directory();
    let (setup, client) = fixture(&root);
    let address = setup.local_addr();
    let mut host = NativeHost::start_with(setup, |_| Ok(()), || Ok(client.clone()))
        .await
        .expect("injected trust installer, actual TLS listener");
    assert!(tokio::net::TcpListener::bind(address).await.is_err());
    let status = client
        .get(format!("{}/idp/setup/status", host.base_url()))
        .send()
        .await
        .expect("shared status")
        .json::<serde_json::Value>()
        .await
        .expect("status JSON");
    assert_eq!(status["state"], "awaiting_choice");
    assert_eq!(status["ready"], false);
    assert_eq!(
        status["owners"],
        serde_json::json!(["idp", "management", "storage"])
    );
    for path in ["idp", "management", "storage", "endpoint"] {
        assert!(
            !root.join(path).exists(),
            "startup must not provision {path}"
        );
    }
    let denied = client.post(format!("{}/idp/setup/create", host.base_url()))
        .json(&serde_json::json!({"operation_nonce":"0123456789abcdef0123456789abcdef", "administrator":"admin", "node":"Desktop", "password":"not-authorized"}))
        .send().await.expect("real shared Create route");
    assert_eq!(denied.status(), reqwest::StatusCode::UNAUTHORIZED);
    let denied = client.post(format!("{}/idp/setup/join", host.base_url()))
        .json(&serde_json::json!({"operation_nonce":"0123456789abcdef0123456789abcdef", "node":"Desktop", "peer":iroh::SecretKey::generate().public().to_string()}))
        .send().await.expect("real shared Join route");
    assert_eq!(denied.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert_eq!(
        host.base_url(),
        LocalTls::api_base(address).expect("stable base")
    );
    host.close()
        .await
        .expect("join listener and worker cleanup");
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .expect("listener released");
    drop(listener);
    let reopened = super::prepare(&root).expect("existing-only neutral reopen");
    assert_eq!(reopened.local_addr(), address);
    drop(reopened);
    fs::remove_dir_all(root).expect("isolated cleanup");
}

#[tokio::test]
async fn native_trust_failure_preserves_prerequisites_without_starting_domain_owners() {
    let root = directory();
    let (setup, client) = fixture(&root);
    let address = setup.local_addr();
    let ca = fs::read(setup.tls().ca_path()).expect("CA before refusal");
    assert!(
        NativeHost::start_with(
            setup,
            |_| Err(io::Error::other("consent denied")),
            || Ok(client)
        )
        .await
        .is_err()
    );
    assert_eq!(
        fs::read(root.join("endpoint.tls/ca.pem")).expect("retained CA"),
        ca
    );
    assert!(!root.join("idp").exists());
    assert!(tokio::net::TcpListener::bind(address).await.is_ok());
    drop(super::prepare(&root).expect("exact retry after trust refusal"));
    fs::remove_dir_all(root).expect("isolated cleanup");
}

#[test]
fn native_reopen_refuses_lost_host_and_tls_evidence_without_replacement() {
    for lost in [
        "host.json",
        "setup-admission/state",
        "endpoint.tls/server.key",
    ] {
        let root = directory();
        let setup = super::prepare(&root).expect("fresh prerequisites");
        let host = fs::read(root.join("host.json")).expect("original host identity");
        drop(setup);
        fs::remove_file(root.join(lost)).expect("inject lost evidence");
        assert!(super::prepare(&root).is_err(), "must refuse missing {lost}");
        assert!(!root.join(lost).exists(), "must never regenerate {lost}");
        if lost != "host.json" {
            assert_eq!(
                fs::read(root.join("host.json")).expect("host preserved"),
                host
            );
        }
        fs::remove_dir_all(root).expect("isolated cleanup");
    }
}

#[tokio::test]
async fn desktop_origin_is_explicit_and_does_not_admit_unrelated_browser_origins() {
    let root = directory();
    let (setup, client) = fixture(&root);
    let mut host = NativeHost::start_with(setup, |_| Ok(()), || Ok(client.clone()))
        .await
        .expect("isolated host");
    let url = format!("{}/idp/setup/status", host.base_url());
    assert_eq!(
        client
            .get(&url)
            .header("Origin", super::native_origin())
            .send()
            .await
            .expect("exact native origin")
            .status(),
        reqwest::StatusCode::OK
    );
    assert_eq!(
        client
            .get(&url)
            .header("Origin", "https://unrelated.example")
            .send()
            .await
            .expect("unrelated origin")
            .status(),
        reqwest::StatusCode::FORBIDDEN
    );
    host.close().await.expect("close host and worker");
    fs::remove_dir_all(root).expect("isolated cleanup");
}

#[tokio::test]
async fn native_verification_failure_closes_the_listener_without_replacing_tls_or_owner_state() {
    let root = directory();
    let setup = super::prepare(&root).expect("fresh neutral prerequisites");
    let address = setup.local_addr();
    let ca = fs::read(setup.tls().ca_path()).expect("original CA");
    let untrusted = reqwest::Client::builder()
        .use_rustls_tls()
        .tls_built_in_root_certs(false)
        .no_proxy()
        .build()
        .expect("empty root verifier");
    assert!(
        NativeHost::start_with(setup, |_| Ok(()), || Ok(untrusted))
            .await
            .is_err()
    );
    assert!(
        tokio::net::TcpListener::bind(address).await.is_ok(),
        "failed verification must join the listener"
    );
    assert!(!root.join("idp").exists());
    assert_eq!(
        fs::read(root.join("endpoint.tls/ca.pem")).expect("same CA"),
        ca
    );
    drop(super::prepare(&root).expect("exact retained retry, not CA regeneration"));
    fs::remove_dir_all(root).expect("isolated cleanup");
}
