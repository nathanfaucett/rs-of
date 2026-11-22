#![cfg(feature = "cli")]

use std::{
    fs,
    net::TcpListener,
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use reqwest::{Method, redirect::Policy};
use serde_json::{Value, json};
use tokio::time::sleep;

struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[tokio::test]
async fn fresh_server_authenticates_all_clients_and_restarts() {
    smoke().await;
}

#[tokio::test]
async fn portless_server_restarts_with_retained_state() {
    let root = tempfile::tempdir().expect("fresh performance directory");
    let listener = TcpListener::bind("127.0.0.1:0").expect("available port");
    let port = listener.local_addr().expect("address").port();
    drop(listener);
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_millis(100))
        .build()
        .expect("HTTP client");
    for restart in 0..2 {
        let started = Instant::now();
        let mut server = Server(
            Command::new(env!("CARGO_BIN_EXE_unified-server"))
                .args(["local", "--data-dir"])
                .arg(root.path())
                .env("PORT", port.to_string())
                .env("PORTLESS_URL", format!("http://127.0.0.1:{port}"))
                .stdout(Stdio::null())
                .spawn()
                .expect("start local server"),
        );
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
                eprintln!("server restart {restart} ready in {:?}", started.elapsed());
                break;
            }
            sleep(Duration::from_millis(20)).await;
        }
        for origin in [
            "https://of.localhost:1355",
            "https://password-manager.localhost:1355",
        ] {
            let response = client
                .request(
                    Method::OPTIONS,
                    format!("http://127.0.0.1:{port}/idp/oauth2/token"),
                )
                .header("origin", origin)
                .header("access-control-request-method", "POST")
                .send()
                .await
                .expect("default frontend CORS preflight");
            assert_eq!(response.headers()["access-control-allow-origin"], origin);
        }
    }
}

async fn smoke() {
    let root = tempfile::tempdir().expect("temporary data directory");
    let seed = tempfile::NamedTempFile::new().expect("optional seed file");
    fs::write(
        seed.path(),
        br#"{"users":[{"username":"test-user","password":"test-password"}]}"#,
    )
    .expect("seed JSON");
    let listener = TcpListener::bind("127.0.0.1:0").expect("available port");
    let port = listener.local_addr().expect("address").port();
    drop(listener);
    let base = format!("http://127.0.0.1:{port}");
    let mut retained_jwks = None;
    let mut retained_resource = None;
    let mut retained_filesystem = None;
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
                "--password-manager-origin",
                "http://localhost:5174",
            ]);
        command.arg("--seed").arg(seed.path());
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
            let client = reqwest::Client::builder()
                .no_proxy()
                .redirect(Policy::none())
                .timeout(Duration::from_secs(10))
                .build()
                .expect("HTTP client");
            if client
                .get(format!("{base}/idp/health"))
                .timeout(Duration::from_millis(200))
                .send()
                .await
                .is_ok_and(|r| r.status().is_success())
            {
                let elapsed = start.elapsed();
                assert!(
                    elapsed < Duration::from_secs(3),
                    "startup exceeded three seconds: {elapsed:?}"
                );
                startup_times.push(elapsed);
                break client;
            }
            sleep(Duration::from_millis(20)).await;
        };
        assert_local_surface(&client, &base, root.path(), &mut retained_jwks).await;
        authenticate(&client, &base, restart == 0).await;
        exercise_retained_resources(
            &client,
            &base,
            root.path(),
            &mut retained_filesystem,
            &mut retained_resource,
        )
        .await;
        drop(server);
    }
    eprintln!(
        "four-owner health, sign-in, PKCE and restart passed; startup times: {startup_times:?}"
    );
    assert!(root.path().join("unified.redb").is_file());
    assert!(!root.path().join("tls").exists());
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

async fn assert_local_surface(
    client: &reqwest::Client,
    base: &str,
    root: &Path,
    retained_jwks: &mut Option<Value>,
) {
    let mut entries = fs::read_dir(root)
        .expect("local data directory")
        .map(|entry| entry.expect("directory entry").file_name())
        .collect::<Vec<_>>();
    entries.sort();
    assert_eq!(
        entries,
        vec!["unified.redb"],
        "local restart must retain the single-file layout after resource use"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(root.join("unified.redb"))
                .expect("database metadata")
                .permissions()
                .mode()
                & 0o077,
            0
        );
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
            .request(Method::OPTIONS, format!("{base}/idp/oauth2/token"))
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
    let keys = jwks["keys"].as_array().expect("JWKS keys");
    let _first_key = keys.first().expect("JWKS key");
    if let Some(previous) = &retained_jwks {
        assert_eq!(&jwks, previous, "restart must retain the signing key");
    }
    *retained_jwks = Some(jwks);
}

async fn exercise_retained_resources(
    client: &reqwest::Client,
    base: &str,
    root: &Path,
    retained_filesystem: &mut Option<String>,
    retained_resource: &mut Option<String>,
) {
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
    let bearer = token["access_token"].as_str().expect("seed User bearer");
    *retained_filesystem =
        Some(filesystem_restart(client, base, bearer, retained_filesystem.as_deref()).await);
    if let Some(id) = &retained_resource {
        let read = exchange_storage(
            client,
            base,
            "local-login",
            bearer,
            "storage:databases",
            "read",
        )
        .await;
        assert_saved_resource(client, &format!("{base}/storage/databases/{id}/sql"), &read).await;
    } else {
        *retained_resource = Some(storage_roundtrip(client, base, "local-login", bearer).await);
    }
    assert_eq!(
        fs::read_dir(root)
            .expect("layout after resource use")
            .count(),
        1,
        "resource use must not create sidecars"
    );
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
    exercise_device(client, base, bearer, first_start).await;
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
    exercise_frontends(client, base, bearer).await;
}

async fn exercise_device(client: &reqwest::Client, base: &str, bearer: &str, first_start: bool) {
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
}

async fn exercise_frontends(client: &reqwest::Client, base: &str, bearer: &str) {
    for (id, redirect) in [
        ("app", "http://localhost:5173/callback"),
        ("password-manager", "http://localhost:5174/callback"),
    ] {
        let token = frontend_token(client, base, bearer, id, redirect).await;
        storage_roundtrip(
            client,
            base,
            id,
            token["access_token"].as_str().expect("frontend bearer"),
        )
        .await;
    }
}

async fn frontend_token(
    client: &reqwest::Client,
    base: &str,
    bearer: &str,
    id: &str,
    redirect: &str,
) -> Value {
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
    token
}

async fn exchange_storage(
    client: &reqwest::Client,
    base: &str,
    client_id: &str,
    bearer: &str,
    resource: &str,
    action: &str,
) -> String {
    let details = json!([{"type":"storage","actions":[action]}]).to_string();
    exchange_token(client, base, client_id, bearer, resource, Some(&details)).await
}

async fn exchange_token(
    client: &reqwest::Client,
    base: &str,
    client_id: &str,
    bearer: &str,
    resource: &str,
    details: Option<&str>,
) -> String {
    let mut form = vec![
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
    ];
    if let Some(details) = details {
        form.push(("authorization_details", details));
    }
    let response = client
        .post(format!("{base}/idp/oauth2/token"))
        .form(&form)
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

async fn storage_roundtrip(
    client: &reqwest::Client,
    base: &str,
    client_id: &str,
    bearer: &str,
) -> String {
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
    assert_saved_resource(client, &url, &read).await;
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
    id.to_owned()
}

async fn filesystem_restart(
    client: &reqwest::Client,
    base: &str,
    bearer: &str,
    retained: Option<&str>,
) -> String {
    let id = if let Some(id) = retained {
        id.to_owned()
    } else {
        let write = exchange_storage(
            client,
            base,
            "local-login",
            bearer,
            "storage:filesystems",
            "write",
        )
        .await;
        let response = client
            .post(format!("{base}/storage/filesystems"))
            .bearer_auth(&write)
            .json(&json!({"name":"Retained filesystem"}))
            .send()
            .await
            .expect("create filesystem");
        assert_eq!(response.status(), 201);
        let resource: Value = response.json().await.expect("filesystem resource");
        let id = resource["id"].as_str().expect("filesystem ID").to_owned();
        let response = client
            .put(format!("{base}/storage/filesystems/{id}/residency"))
            .bearer_auth(write)
            .json(&json!({"mode":"passthrough"}))
            .send()
            .await
            .expect("persist filesystem residency");
        assert_eq!(response.status(), 204);
        id
    };
    let read = exchange_storage(
        client,
        base,
        "local-login",
        bearer,
        "storage:filesystems",
        "read",
    )
    .await;
    let response = client
        .get(format!("{base}/storage/filesystems/{id}"))
        .bearer_auth(&read)
        .send()
        .await
        .expect("retained filesystem");
    assert_eq!(response.status(), 200);
    let resource: Value = response.json().await.expect("filesystem detail");
    assert_eq!(resource["name"], "Retained filesystem");
    let response = client
        .get(format!("{base}/storage/filesystems/{id}/residency"))
        .bearer_auth(read)
        .send()
        .await
        .expect("retained residency");
    assert_eq!(response.status(), 200);
    let residency: Value = response.json().await.expect("filesystem residency");
    assert_eq!(residency["mode"], "passthrough");
    id
}

async fn assert_saved_resource(client: &reqwest::Client, url: &str, read: &str) {
    let response = client
        .post(url)
        .bearer_auth(read)
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
        .post(url)
        .bearer_auth(read)
        .json(&json!({"statements":[
            {"sql":"SELECT secret FROM local_private_keys","params":[]}
        ]}))
        .send()
        .await
        .expect("private key isolation check");
    assert_eq!(
        response.status(),
        400,
        "resource SQL must not expose owner private keys"
    );
}

#[tokio::test]
async fn device_selection_is_scoped_and_deselection_preserves_the_resource() {
    use std::sync::Arc;

    use management_service::{
        ManagementService,
        ofdb::{DbPermissionRepo, DbRoleRepo},
    };
    use of_model::id::Id;

    let root = tempfile::tempdir().expect("fresh owner data");
    let shared = db::EmbeddedDatabase::open(root.path())
        .await
        .expect("fresh database");
    let database = Arc::new(shared.database.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind owner routes");
    let address = listener.local_addr().expect("listener address");
    let base = format!("http://{address}");
    let idp_base = format!("{base}/idp");
    let device_base = format!("{base}/device");
    let storage_base = format!("{base}/storage");
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(20))
        .build()
        .expect("owner HTTP client");
    let idp = idp_server::build_local_router(
        root.path(),
        &idp_base,
        "http://localhost:5173",
        Some("http://localhost:5174"),
        Some(database.clone()),
        Some(shared.storage()),
        &[("other-user".into(), "other-password".into())],
    )
    .await
    .expect("IdP owner");
    let device = device_server::build_local_router(
        root.path(),
        address.port(),
        &idp_base,
        Some(database.clone()),
        Some(client.clone()),
    )
    .await
    .expect("Device owner");
    let storage = storage_server::build_local_runtime(
        root.path(),
        address.port(),
        &idp_base,
        &device_base,
        Some(database),
        Some(shared.storage()),
        Some(client.clone()),
    )
    .await
    .expect("Storage owner");
    let router = unified_server::compose_router(idp, axum::Router::new(), device, storage.router());
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.expect("serve owners");
    });

    let login = |username: &'static str, password: &'static str| {
        let url = format!("{idp_base}/oauth2/token");
        let client = client.clone();
        async move {
            let response = client
                .post(url)
                .form(&[
                    ("grant_type", "password"),
                    ("client_id", "local-login"),
                    ("username", username),
                    ("password", password),
                    ("scope", "openid"),
                ])
                .send()
                .await
                .expect("password request");
            assert_eq!(response.status(), 200);
            response.json::<Value>().await.expect("login token")["access_token"]
                .as_str()
                .expect("login bearer")
                .to_owned()
        }
    };
    let bearer = login("admin@local.test", "password").await;
    let other_user = login("other-user", "other-password").await;
    exercise_device(&client, &base, &bearer, true).await;

    let device_token =
        exchange_token(&client, &base, "local-login", &bearer, &device_base, None).await;
    let listed: Value = client
        .get(format!("{device_base}/devices"))
        .bearer_auth(&device_token)
        .send()
        .await
        .expect("list Devices")
        .error_for_status()
        .expect("approved Device list")
        .json()
        .await
        .expect("Device list body");
    let device_id = listed[0]["id"].as_str().expect("approved Device ID");
    let selection = format!("{device_base}/devices/{device_id}/selection");
    let management = ManagementService::new(
        DbPermissionRepo::new(shared.client()),
        DbRoleRepo::new(shared.client()),
    );
    let subject = |token: String| {
        let client = client.clone();
        let url = format!("{idp_base}/userinfo");
        async move {
            let info: Value = client
                .get(url)
                .bearer_auth(token)
                .send()
                .await
                .expect("userinfo")
                .error_for_status()
                .expect("valid User")
                .json()
                .await
                .expect("User response");
            info["sub"]
                .as_str()
                .expect("subject")
                .parse::<Id>()
                .expect("User ID")
        }
    };
    let other_token = exchange_token(
        &client,
        &base,
        "local-login",
        &other_user,
        &device_base,
        None,
    )
    .await;
    let other_app = frontend_token(
        &client,
        &base,
        &bearer,
        "app",
        "http://localhost:5173/callback",
    )
    .await;
    let app_bearer = other_app["access_token"]
        .as_str()
        .expect("second Application token");
    let app_token = exchange_token(&client, &base, "app", app_bearer, &device_base, None).await;
    let owner = subject(bearer.clone()).await;
    let stranger = subject(other_user).await;
    let mut selected = Vec::new();
    for (id, login, token) in [
        ("local-login", bearer.as_str(), &device_token),
        ("app", app_bearer, &app_token),
    ] {
        let resource = storage_roundtrip(&client, &base, id, login).await;
        let read = exchange_storage(&client, &base, id, login, "storage:databases", "read").await;
        let url = format!("{storage_base}/databases/{resource}");
        let details: Value = client
            .get(&url)
            .bearer_auth(&read)
            .send()
            .await
            .expect("read database")
            .error_for_status()
            .expect("own database")
            .json()
            .await
            .expect("database details");
        let app: Id = details["applicationId"]
            .as_str()
            .expect("Application")
            .parse()
            .expect("Application ID");
        let role = management
            .create_role(app, "selector", None)
            .await
            .expect("test role");
        let permission = management
            .create_permission(app, "devices.select", None)
            .await
            .expect("Selection permission");
        management
            .add_permission_to_role(app, role.id, permission.id)
            .await
            .expect("grant permission");
        for subject in [owner]
            .into_iter()
            .chain((id == "local-login").then_some(stranger))
        {
            management
                .add_role_to_user(app, subject, role.id)
                .await
                .expect("grant role");
        }
        let body = json!({"applicationId": app, "kind": "database", "id": resource, "storageAccessToken": read});
        let response = client
            .put(&selection)
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .expect("select own database");
        let status = response.status();
        assert_eq!(
            status,
            204,
            "{}",
            response.text().await.expect("Selection response")
        );
        selected.push((
            body,
            format!("{selection}/{app}/database/{resource}"),
            url,
            read,
        ));
    }

    let (body, deselect, resource_url, read) = &selected[0];
    let (app_body, app_deselect, _, _) = &selected[1];
    for (token, body, url) in [
        (&other_token, body, deselect),
        (&device_token, app_body, app_deselect),
    ] {
        assert_eq!(
            client
                .put(&selection)
                .bearer_auth(token)
                .json(body)
                .send()
                .await
                .expect("cross-scope PUT")
                .status(),
            403
        );
        assert_eq!(
            client
                .delete(url)
                .bearer_auth(token)
                .send()
                .await
                .expect("cross-scope DELETE")
                .status(),
            403
        );
    }
    for (field, value) in [("kind", json!("filesystem")), ("id", json!(Id::now_v7()))] {
        let mut invalid = body.clone();
        invalid[field] = value;
        assert_eq!(
            client
                .put(&selection)
                .bearer_auth(&device_token)
                .json(&invalid)
                .send()
                .await
                .expect("wrong Resource binding")
                .status(),
            403
        );
    }
    assert_eq!(
        client
            .get(&selection)
            .bearer_auth(&device_token)
            .send()
            .await
            .expect("no unscoped User list")
            .status(),
        405
    );
    let wrong_device = format!("{device_base}/devices/{}/selection", Id::now_v7());
    assert_eq!(
        client
            .put(wrong_device)
            .bearer_auth(&device_token)
            .json(body)
            .send()
            .await
            .expect("wrong Device")
            .status(),
        403
    );
    let endpoint = listed[0]["publicKey"]
        .as_str()
        .expect("Device endpoint key");
    let listing = client
        .get(format!(
            "{device_base}/replication/devices/{endpoint}/selections"
        ))
        .bearer_auth(&device_token)
        .send()
        .await
        .expect("Storage-only listing");
    assert_eq!(listing.status(), 403);
    for (url, token) in [(deselect, &device_token), (app_deselect, &app_token)] {
        assert_eq!(
            client
                .delete(url)
                .bearer_auth(token)
                .send()
                .await
                .expect("exact scoped DELETE")
                .status(),
            204
        );
    }
    assert_eq!(
        client
            .get(resource_url)
            .bearer_auth(read)
            .send()
            .await
            .expect("retained Resource")
            .status(),
        200
    );
    assert_eq!(
        client
            .post(format!("{storage_base}/sync/connect"))
            .send()
            .await
            .expect("retired Storage caller")
            .status(),
        404
    );
    server.abort();
}
