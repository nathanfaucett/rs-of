#![cfg(test)]

use std::{
    fs,
    net::SocketAddr,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use api::{IndependentSetupHost, LocalTls};
use axum::http::{
    StatusCode,
    header::{ACCESS_CONTROL_ALLOW_CREDENTIALS, ACCESS_CONTROL_ALLOW_ORIGIN},
};
use tokio_util::sync::CancellationToken;

pub fn parent(
    owner: &str,
    prepare: impl Fn(&Path, &str, SocketAddr) -> std::io::Result<Option<IndependentSetupHost>>,
) {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("{owner}-https-{}-{nonce}", std::process::id()));
    fs::create_dir(&root).expect("fixture root");
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("private fixture");
    let empty = root.join("empty-roots");
    fs::create_dir(&empty).expect("empty native directory");
    let empty_bundle = root.join("empty.pem");
    fs::write(&empty_bundle, "").expect("empty native bundle");
    let commands = root.join("commands");
    fs::create_dir(&commands).expect("isolated command directory");
    let composed = format!("/{owner}");
    for (index, (prefix, trusted, installed)) in [
        ("", true, true),
        (composed.as_str(), true, true),
        ("", false, true),
        ("", true, false),
    ]
    .into_iter()
    .enumerate()
    {
        let case = root.join(index.to_string());
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("reserve port");
        let address = listener.local_addr().expect("address");
        drop(listener);
        let host = prepare(&case, prefix, address)
            .expect("owner startup")
            .expect("fresh setup");
        let ca = host.tls().ca_path();
        drop(host);
        fs::write(
            commands.join("trust"),
            if installed {
                "#!/bin/sh\nexit 0\n"
            } else {
                "#!/bin/sh\nexit 1\n"
            },
        )
        .expect("test-only installation injection");
        fs::set_permissions(commands.join("trust"), fs::Permissions::from_mode(0o700))
            .expect("executable stub");
        let mut child = Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "startup::https_tests::production_owner_https_child",
                "--ignored",
                "--nocapture",
            ])
            .env("OWNER_HTTPS_ROOT", &case)
            .env("OWNER_HTTPS_ADDRESS", address.to_string())
            .env("OWNER_HTTPS_PREFIX", prefix)
            .env(
                "OWNER_HTTPS_OK",
                if trusted && installed { "yes" } else { "no" },
            )
            .env("SSL_CERT_FILE", if trusted { &ca } else { &empty_bundle })
            .env("SSL_CERT_DIR", &empty)
            .env("PATH", &commands)
            .stdin(Stdio::null())
            .spawn()
            .expect("isolated child");
        let deadline = Instant::now() + Duration::from_secs(30);
        let status = loop {
            if let Some(status) = child.try_wait().expect("child status") {
                break status;
            }
            if Instant::now() >= deadline {
                child.kill().expect("bound child");
                child.wait().expect("reap child");
                panic!("HTTPS child exceeded 30 seconds");
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        assert!(status.success(), "isolated production HTTPS case");
        assert!(
            !case.join(owner).exists(),
            "status/admission must not provision domain"
        );
    }
    fs::remove_dir_all(root).expect("isolated cleanup");
}

pub async fn child(
    owner: &str,
    prepare: impl Fn(&Path, &str, SocketAddr) -> std::io::Result<Option<IndependentSetupHost>>,
) {
    let root = PathBuf::from(std::env::var_os("OWNER_HTTPS_ROOT").expect("parent root"));
    let address = std::env::var("OWNER_HTTPS_ADDRESS")
        .expect("address")
        .parse()
        .expect("socket address");
    let prefix = std::env::var("OWNER_HTTPS_PREFIX").expect("prefix");
    let host = prepare(&root, &prefix, address)
        .expect("production startup reopen")
        .expect("setup");
    let code_file =
        fs::read_to_string(host.admission_path()).expect("protected management retrieval");
    let code = code_file.lines().nth(1).expect("code").to_owned();
    let cancellation = CancellationToken::new();
    if std::env::var("OWNER_HTTPS_OK").expect("case") == "no" {
        assert!(
            host.serve(cancellation).await.is_err(),
            "mandatory installation/native verification refusal"
        );
        let listener = tokio::net::TcpListener::bind(address)
            .await
            .expect("failed startup released listener");
        drop(listener);
        prepare(&root, &prefix, address)
            .expect("failed startup releases owner lock")
            .expect("retained setup");
        return;
    }
    let client = LocalTls::verification_client().expect("native verification client");
    let server = tokio::spawn(host.serve(cancellation.clone()));
    let base = format!("https://{address}{prefix}");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let response = loop {
        if let Ok(response) = client.get(format!("{base}/setup/status")).send().await {
            break response;
        }
        assert!(tokio::time::Instant::now() < deadline, "bounded startup");
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.json::<serde_json::Value>().await.expect("status"),
        serde_json::json!({"stage":"installation", "state":"awaiting_choice", "ready":false, "owners":[owner]})
    );
    let admission = format!("{base}/setup/admission");
    let other_owner = if owner == "management" {
        "storage"
    } else {
        "management"
    };
    let other = IndependentSetupHost::prepare(
        other_owner,
        &root.join("other-owner"),
        "127.0.0.1:3009".parse().expect("other address"),
        "https://localhost:5173",
        "",
        false,
    )
    .expect("independent other owner")
    .expect("other setup");
    let other_state =
        fs::read_to_string(other.admission_path()).expect("protected other-owner retrieval");
    let other_code = other_state.lines().nth(1).expect("other code");
    assert_eq!(
        client
            .post(&admission)
            .header("Authorization", format!("Setup {other_code}"))
            .send()
            .await
            .expect("cross-owner denial")
            .status(),
        StatusCode::UNAUTHORIZED
    );
    drop(other);
    for invalid in [
        "",
        "Bearer unrelated",
        "Setup invalid",
        &format!("Setup {}", "0".repeat(64)),
    ] {
        assert_eq!(
            client
                .post(&admission)
                .header("Authorization", invalid)
                .send()
                .await
                .expect("denial")
                .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        client
            .post(&admission)
            .header("Cookie", format!("setup={code}"))
            .send()
            .await
            .expect("cookie is not admission")
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        client
            .post(format!("{admission}?code=not-admission-evidence"))
            .send()
            .await
            .expect("URL is not admission")
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        client
            .post(&admission)
            .header("Origin", "https://localhost:5173")
            .send()
            .await
            .expect("origin is not admission")
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        client
            .post(&admission)
            .header("Authorization", format!("Setup {code}"))
            .header("Origin", "https://evil.invalid")
            .send()
            .await
            .expect("wrong origin")
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        client
            .post(&admission)
            .header("Authorization", format!("Setup {code}"))
            .header("Authorization", format!("Setup {code}"))
            .send()
            .await
            .expect("duplicate authorization")
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let response = client
        .post(&admission)
        .header("Authorization", format!("Setup {code}"))
        .header("Origin", "https://localhost:5173")
        .send()
        .await
        .expect("authorized admission");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        response
            .headers()
            .get(ACCESS_CONTROL_ALLOW_ORIGIN)
            .expect("exact CORS"),
        "https://localhost:5173"
    );
    assert!(
        !response
            .headers()
            .contains_key(ACCESS_CONTROL_ALLOW_CREDENTIALS)
    );
    assert_eq!(
        client
            .get(format!("{base}/{owner}/setup/status"))
            .send()
            .await
            .expect("no doubled prefix")
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        client
            .get(format!("{base}/users"))
            .send()
            .await
            .expect("not a normal protected router")
            .status(),
        StatusCode::NOT_FOUND
    );
    let admission_path = root.join(format!("{owner}.setup-host/setup-admission/state"));
    let held_state = root.join("held-state");
    fs::rename(&admission_path, &held_state).expect("simulate live admission loss");
    for restored in [false, true] {
        if restored {
            fs::rename(&held_state, &admission_path)
                .expect("restore exact state without clearing poison");
        }
        let response = client
            .get(format!("{base}/setup/status"))
            .send()
            .await
            .expect("unavailable scope status");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response
                .json::<serde_json::Value>()
                .await
                .expect("unavailable status"),
            serde_json::json!({"stage":"installation", "state":"unavailable", "ready":false, "owners":[owner]})
        );
        assert_eq!(
            client
                .post(&admission)
                .header("Authorization", format!("Setup {code}"))
                .send()
                .await
                .expect("lost admission denial")
                .status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }
    cancellation.cancel();
    server
        .await
        .expect("server join")
        .expect("verified shutdown");
    let reopened = prepare(&root, &prefix, address)
        .expect("offline setup reopen")
        .expect("setup");
    assert_eq!(
        fs::read_to_string(reopened.admission_path()).expect("retained code"),
        code_file
    );
    drop(reopened);
    fs::remove_dir_all(root.join(format!("{owner}.setup-host/setup-admission")))
        .expect("simulate lost admission");
    assert!(
        prepare(&root, &prefix, address).is_err(),
        "missing admission is not fresh permission"
    );
}
