#![cfg(feature = "cli")]

use std::{
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use serde_json::{Value, json};

struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[tokio::test]
async fn fresh_http_server_authenticates_all_clients_and_restarts() {
    smoke(false).await;
}

#[tokio::test]
async fn fresh_https_server_authenticates_all_clients_and_reuses_tls() {
    smoke(true).await;
}

async fn smoke(tls: bool) {
    let root = tempfile::tempdir().expect("temporary data directory");
    let seed = tempfile::NamedTempFile::new().expect("optional seed file");
    std::fs::write(
        seed.path(),
        br#"{"users":[{"username":"test-user","password":"test-password"}]}"#,
    )
    .expect("seed JSON");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("available port");
    let port = listener.local_addr().expect("address").port();
    drop(listener);
    let base = format!("{}://127.0.0.1:{port}", if tls { "https" } else { "http" });
    let mut retained_tls = None;
    for _ in 0..2 {
        let start = Instant::now();
        let mut command = Command::new(env!("CARGO_BIN_EXE_unified-server"));
        command
            .args(["local", "--data-dir"])
            .arg(root.path())
            .args([
                "--port",
                &port.to_string(),
                "--origin",
                "http://localhost:5173",
            ]);
        if !tls {
            command.arg("--no-tls").arg("--seed").arg(seed.path());
        }
        let mut server = Server(
            command
                .stdout(Stdio::null())
                .spawn()
                .expect("start local server"),
        );
        let client = loop {
            assert!(
                server.0.try_wait().expect("child status").is_none(),
                "server exited"
            );
            assert!(
                start.elapsed() < Duration::from_secs(3),
                "startup exceeded three seconds"
            );
            let mut builder = reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(10));
            if tls {
                let Ok(certificate) = std::fs::read(root.path().join("tls/cert.pem")) else {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    continue;
                };
                builder = builder.add_root_certificate(
                    reqwest::Certificate::from_pem(&certificate).expect("self-signed certificate"),
                );
            }
            let client = builder.build().expect("HTTP client");
            if client
                .get(format!("{base}/idp/health"))
                .timeout(Duration::from_millis(200))
                .send()
                .await
                .is_ok_and(|r| r.status().is_success())
            {
                assert!(
                    start.elapsed() < Duration::from_secs(3),
                    "startup exceeded three seconds"
                );
                break client;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        };
        for owner in ["idp", "device", "management", "storage"] {
            assert_eq!(
                client
                    .get(format!("{base}/{owner}/health"))
                    .send()
                    .await
                    .expect("health")
                    .status(),
                200,
                "{owner}"
            );
            for path in [
                format!("/{owner}/{owner}/health"),
                format!("/{owner}/setup/owner/status"),
            ] {
                assert_eq!(
                    client
                        .get(format!("{base}{path}"))
                        .send()
                        .await
                        .expect("absent route")
                        .status(),
                    404,
                    "{path}"
                );
            }
        }
        let metadata: Value = client
            .get(format!("{base}/idp/.well-known/openid-configuration"))
            .send()
            .await
            .expect("discovery")
            .json()
            .await
            .expect("metadata");
        assert_eq!(metadata["issuer"], format!("{base}/idp"));
        assert_eq!(
            metadata["token_endpoint"],
            format!("{base}/idp/oauth2/token")
        );
        assert_eq!(
            metadata["authorization_endpoint"],
            format!("{base}/idp/oauth2/auth")
        );
        for origin in ["http://localhost:5173", "http://localhost:5174"] {
            let response = client
                .request(reqwest::Method::OPTIONS, format!("{base}/idp/oauth2/token"))
                .header("origin", origin)
                .header("access-control-request-method", "POST")
                .send()
                .await
                .expect("CORS preflight");
            assert_eq!(response.headers()["access-control-allow-origin"], origin);
        }
        authenticate(&client, &base).await;
        if !tls {
            let token = client
                .post(format!("{base}/idp/oauth2/token"))
                .form(&[
                    ("grant_type", "password"),
                    ("client_id", "local-login"),
                    ("username", "test-user"),
                    ("password", "test-password"),
                    ("scope", "openid"),
                ])
                .send()
                .await
                .expect("seed user sign-in");
            assert_eq!(token.status(), 200);
        } else {
            let material = (
                std::fs::read(root.path().join("tls/cert.pem")).expect("certificate"),
                std::fs::read(root.path().join("tls/key.pem")).expect("private key"),
            );
            if let Some(retained) = &retained_tls {
                assert_eq!(&material, retained);
            }
            retained_tls = Some(material);
            assert_eq!(
                std::fs::read_dir(root.path().join("tls"))
                    .expect("TLS files")
                    .count(),
                2
            );
        }
        drop(server);
    }
    assert!(root.path().join("unified.redb").is_file());
    assert_eq!(root.path().join("tls").exists(), tls);
    for owner in ["idp", "device", "management", "storage"] {
        assert!(
            !root
                .path()
                .join(owner)
                .join(format!("{owner}.redb"))
                .exists()
        );
        assert!(!root.path().join(format!("{owner}.setup-host")).exists());
    }
}

async fn authenticate(client: &reqwest::Client, base: &str) {
    let response = client
        .post(format!("{base}/idp/oauth2/token"))
        .form(&[
            ("grant_type", "password"),
            ("client_id", "local-login"),
            ("username", "admin@local.test"),
            ("password", "password"),
            ("scope", "openid profile email"),
        ])
        .send()
        .await
        .expect("administrator sign-in");
    assert_eq!(response.status(), 200);
    let login: Value = response.json().await.expect("token JSON");
    assert_eq!(login["iss"], format!("{base}/idp"));
    let bearer = login["access_token"].as_str().expect("access token");
    let user: Value = client
        .get(format!("{base}/idp/userinfo"))
        .bearer_auth(bearer)
        .send()
        .await
        .expect("userinfo")
        .json()
        .await
        .expect("user JSON");
    assert_eq!(user["email"], "admin@local.test");
    for (id, redirect) in [
        ("app", "http://localhost:5173/callback"),
        ("password-manager", "http://localhost:5174/callback"),
    ] {
        let consent = client
            .post(format!("{base}/idp/oauth2/auth/approve-for-user"))
            .bearer_auth(bearer)
            .json(&json!({"client_id":id,"redirect_uri":redirect,"scope":"openid"}))
            .send()
            .await
            .expect("approve frontend");
        assert_eq!(consent.status(), 200, "{id}");
        let response = client.post(format!("{base}/idp/oauth2/auth")).bearer_auth(bearer).json(&json!({
            "response_type":"code", "client_id":id,"redirect_uri":redirect,"scope":"openid","state":"test-state",
            "code_challenge":"E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM", "code_challenge_method":"S256"
        })).send().await.expect("authorize frontend");
        assert_eq!(response.status(), 200, "{id}");
        let authorization: Value = response.json().await.expect("authorization JSON");
        assert_eq!(authorization["state"], "test-state");
        let response = client
            .post(format!("{base}/idp/oauth2/token"))
            .form(&[
                ("grant_type", "authorization_code"),
                ("client_id", id),
                ("redirect_uri", redirect),
                (
                    "code",
                    authorization["code"].as_str().expect("authorization code"),
                ),
                (
                    "code_verifier",
                    "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
                ),
            ])
            .send()
            .await
            .expect("PKCE token exchange");
        assert_eq!(response.status(), 200, "{id}");
        let token: Value = response.json().await.expect("frontend token JSON");
        assert_eq!(token["iss"], format!("{base}/idp"));
        assert!(token["access_token"].is_string());
        assert!(token["id_token"].is_string());
    }
}
