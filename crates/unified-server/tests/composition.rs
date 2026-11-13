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

#[tokio::test]
async fn fresh_server_is_ready_within_three_seconds() {
    let root = tempfile::tempdir().expect("fresh performance directory");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("available port");
    let port = listener.local_addr().expect("address").port();
    drop(listener);
    let started = Instant::now();
    let mut server = Server(
        Command::new(env!("CARGO_BIN_EXE_unified-server"))
            .args(["local", "--data-dir"])
            .arg(root.path())
            .args(["--port", &port.to_string(), "--no-tls"])
            .stdout(Stdio::null())
            .spawn()
            .expect("start local server"),
    );
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_millis(100))
        .build()
        .expect("HTTP client");
    loop {
        assert!(
            server.0.try_wait().expect("child status").is_none(),
            "server exited"
        );
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "startup exceeded three seconds"
        );
        if client
            .get(format!("http://127.0.0.1:{port}/idp/health"))
            .send()
            .await
            .is_ok_and(|response| response.status().is_success())
        {
            assert!(
                started.elapsed() < Duration::from_secs(3),
                "startup exceeded three seconds"
            );
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
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
    let mut startup_times = Vec::new();
    for restart in 0..2 {
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
                start.elapsed() < Duration::from_secs(60),
                "startup exceeded readiness deadline"
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
                startup_times.push(start.elapsed());
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
        let response = client
            .get(format!("{base}/idp/.well-known/jwks.json"))
            .send()
            .await
            .expect("local JWKS over HTTP");
        assert_eq!(response.status(), 200);
        let jwks: Value = response.json().await.expect("JWKS JSON");
        assert!(!jwks["keys"].as_array().expect("JWKS keys").is_empty());
        authenticate(&client, &base, restart == 0).await;
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
            let token: Value = token.json().await.expect("seed User token");
            storage_roundtrip(
                &client,
                &base,
                "local-login",
                token["access_token"].as_str().expect("seed User bearer"),
            )
            .await;
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
    eprintln!(
        "four-owner health, sign-in, PKCE and restart passed; startup times: {startup_times:?}"
    );
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

async fn authenticate(client: &reqwest::Client, base: &str, first_start: bool) {
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
    for id in ["local-device", "local-storage", "local-management"] {
        let response = client
            .post(format!("{base}/idp/oauth2/token"))
            .form(&[
                ("grant_type", "client_credentials"),
                ("client_id", id),
                ("client_secret", "local-password"),
                ("scope", "idp.token.validate idp.user.lookup"),
                ("audience", &format!("{base}/idp")),
            ])
            .send()
            .await
            .expect("local owner service token");
        let status = response.status();
        let body = response.text().await.expect("service token response");
        assert_eq!(status, 200, "{id}: {body}");
    }
    let response = client
        .post(format!("{base}/idp/oauth2/token"))
        .form(&[
            (
                "grant_type",
                "urn:ietf:params:oauth:grant-type:token-exchange",
            ),
            ("subject_token", bearer),
            (
                "subject_token_type",
                "urn:ietf:params:oauth:token-type:access_token",
            ),
            ("client_id", "local-login"),
            ("resource", &format!("{base}/device")),
        ])
        .send()
        .await
        .expect("Device token exchange");
    assert_eq!(response.status(), 200);
    let token: Value = response.json().await.expect("Device token");
    let response = client
        .get(format!("{base}/device/devices"))
        .bearer_auth(token["access_token"].as_str().expect("Device bearer"))
        .send()
        .await
        .expect("Device list");
    let status = response.status();
    let body = response.text().await.expect("Device list response");
    assert_eq!(status, 200, "Device list: {body}");
    assert_eq!(
        client
            .get(format!("{base}/device/devices"))
            .send()
            .await
            .expect("unauthenticated Device list")
            .status(),
        401,
    );
    let device_bearer = token["access_token"].as_str().expect("Device bearer");
    let listed: Value = serde_json::from_str(&body).expect("Device list JSON");
    if first_start {
        assert_eq!(listed, json!([]));
        let enrollment = client
            .post(format!("{base}/device/devices/enrollments"))
            .bearer_auth(device_bearer)
            .json(&json!({
                "name": "Local HTTP device",
                "publicKey": "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
                "address": "127.0.0.1:23456",
            }))
            .send()
            .await
            .expect("Device enrollment");
        let status = enrollment.status();
        let body = enrollment.text().await.expect("Device enrollment response");
        assert_eq!(status, 200, "Device enrollment: {body}");
        let enrollment: Value = serde_json::from_str(&body).expect("enrollment JSON");
        let device_id = enrollment["id"].as_str().expect("enrollment ID");
        assert_eq!(enrollment["state"], "approved");
        let devices: Value = client
            .get(format!("{base}/device/devices"))
            .bearer_auth(device_bearer)
            .send()
            .await
            .expect("approved Device list")
            .json()
            .await
            .expect("Device list JSON");
        assert!(
            devices
                .as_array()
                .expect("Device array")
                .iter()
                .any(|device| device["id"] == device_id)
        );
    } else {
        assert_eq!(listed.as_array().expect("retained Device array").len(), 1);
        assert_eq!(listed[0]["name"], "Local HTTP device");
    }
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
        storage_roundtrip(
            client,
            base,
            id,
            token["access_token"].as_str().expect("frontend bearer"),
        )
        .await;
    }
}

async fn exchange_storage(
    client: &reqwest::Client,
    base: &str,
    client_id: &str,
    bearer: &str,
    resource: &str,
    action: &str,
) -> String {
    let response = client
        .post(format!("{base}/idp/oauth2/token"))
        .form(&[
            (
                "grant_type",
                "urn:ietf:params:oauth:grant-type:token-exchange",
            ),
            ("subject_token", bearer),
            (
                "subject_token_type",
                "urn:ietf:params:oauth:token-type:access_token",
            ),
            ("client_id", client_id),
            ("resource", resource),
            (
                "authorization_details",
                &json!([{"type":"storage","actions":[action]}]).to_string(),
            ),
        ])
        .send()
        .await
        .expect("Storage token exchange");
    let status = response.status();
    let body = response.text().await.expect("Storage token response");
    assert_eq!(status, 200, "{client_id} Storage exchange: {body}");
    let token: Value = serde_json::from_str(&body).expect("Storage token JSON");
    token["access_token"]
        .as_str()
        .expect("Storage bearer")
        .to_owned()
}

async fn storage_roundtrip(client: &reqwest::Client, base: &str, client_id: &str, bearer: &str) {
    let write = exchange_storage(
        client,
        base,
        client_id,
        bearer,
        "storage:databases",
        "write",
    )
    .await;
    let response = client
        .post(format!("{base}/storage/databases"))
        .bearer_auth(&write)
        .json(&json!({"name":"Local HTTP roundtrip"}))
        .send()
        .await
        .expect("Storage create");
    let status = response.status();
    let body = response.text().await.expect("Storage create response");
    assert_eq!(status, 201, "{client_id} Storage create: {body}");
    let resource: Value = serde_json::from_str(&body).expect("Storage resource JSON");
    let id = resource["id"].as_str().expect("Storage resource ID");

    let url = format!("{base}/storage/databases/{id}/sql");
    let response = client.post(&url).bearer_auth(&write).json(&json!({"statements":[
        {"sql":"CREATE TABLE notes (id TEXT PRIMARY KEY, value TEXT NOT NULL)","params":[]},
        {"sql":"INSERT INTO notes (id, value) VALUES (?, ?)","params":[{"type":"text","value":"note"},{"type":"text","value":"saved"}]}
    ]})).send().await.expect("Storage SQL write");
    let status = response.status();
    let body = response.text().await.expect("Storage write response");
    assert_eq!(status, 200, "Storage write: {body}");
    let read = exchange_storage(client, base, client_id, bearer, "storage:databases", "read").await;
    let response = client
        .post(&url)
        .bearer_auth(&read)
        .json(&json!({"statements":[
            {"sql":"SELECT value FROM notes WHERE id = ?","params":[{"type":"text","value":"note"}]}
        ]}))
        .send()
        .await
        .expect("Storage SQL read");
    let status = response.status();
    let body = response.text().await.expect("Storage read response");
    assert_eq!(status, 200, "Storage read: {body}");
    let result: Value = serde_json::from_str(&body).expect("Storage read JSON");
    assert_eq!(
        result["results"][0]["rows"],
        json!([[{"type":"text","value":"saved"}]])
    );
    let response = client
        .post(&url)
        .bearer_auth(&read)
        .json(&json!({"statements":[
            {"sql":"DELETE FROM notes","params":[]}
        ]}))
        .send()
        .await
        .expect("read-only write denial");
    assert_eq!(response.status(), 403);
}
