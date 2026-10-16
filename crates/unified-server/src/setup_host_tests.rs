use std::{fs, path::PathBuf};

use crate::{BootstrapConfig, BootstrapPurpose, SetupHost};

fn root() -> PathBuf {
    std::env::temp_dir().join(format!(
        "unified-setup-host-{}",
        idp_model::model::Id::now_v7()
    ))
}

#[test]
fn fresh_host_awaits_choice_without_installation_or_credentials_and_reopens_exactly() {
    let root = root();
    let host = BootstrapConfig::create(&root, BootstrapPurpose::AwaitingChoice, None)
        .expect("fresh neutral host");
    let config = host.config();
    assert!(config.idp.oauth2.issuer.is_empty());
    let setup = SetupHost::create(host).expect("fresh setup prerequisites");
    assert!(
        BootstrapConfig::open(&root).is_err(),
        "startup guard excludes competing host operations"
    );
    let state_path = setup.admission_path();
    let state = fs::read(&state_path).expect("protected management retrieval");
    let ca = fs::read(setup.tls().ca_path()).expect("retained CA");
    assert!(!config.credential_bootstrap_directory().exists());
    assert!(!config.local_provisioning_directory().exists());
    assert!(!std::path::Path::new(&config.idp.data_dir).exists());
    drop(setup);
    let setup = SetupHost::open(BootstrapConfig::open(&root).expect("retained host"))
        .expect("existing-only setup reopening");
    assert!(fs::read(setup.admission_path()).expect("same admission") == state);
    assert_eq!(fs::read(setup.tls().ca_path()).expect("same CA"), ca);
    drop(setup);
    fs::remove_file(state_path).expect("simulate admission loss");
    assert!(SetupHost::open(BootstrapConfig::open(&root).expect("retained host")).is_err());
    assert!(!root.join("setup-admission/state").exists());
    assert!(SetupHost::create(BootstrapConfig::open(&root).expect("retained host")).is_err());
    assert!(!root.join("setup-admission/state").exists());
    fs::remove_dir_all(root).expect("isolated cleanup");
}

#[test]
fn interrupted_host_settings_are_not_fresh_generation_permission() {
    let root = root();
    let host = BootstrapConfig::create(&root, BootstrapPurpose::AwaitingChoice, None)
        .expect("fresh host settings");
    let config = host.config();
    drop(host);
    let retained = BootstrapConfig::open(&root).expect("unattempted retained host");
    assert!(
        SetupHost::create(retained).is_err(),
        "reopened metadata cannot authorize fresh production"
    );
    assert!(!root.join("setup-admission").exists());
    assert!(!crate::LocalTls::directory(&config).exists());
    fs::remove_dir_all(root).expect("isolated cleanup");
}

#[test]
fn neutral_production_history_is_required_without_changing_active_host_metadata() {
    let root = root();
    let active = BootstrapConfig::create(&root.join("active"), BootstrapPurpose::Create, None)
        .expect("active Create host");
    let active_bytes = fs::read(root.join("active/host.json")).expect("public host metadata");
    let active_json: serde_json::Value =
        serde_json::from_slice(&active_bytes).expect("metadata JSON");
    assert!(
        active_json.get("setup_attempted").is_none(),
        "neutral-only history must not alter active metadata"
    );
    drop(active);
    BootstrapConfig::open(&root.join("active")).expect("existing active schema");
    let neutral = BootstrapConfig::create(
        &root.join("neutral"),
        BootstrapPurpose::AwaitingChoice,
        None,
    )
    .expect("neutral host");
    drop(neutral);
    let path = root.join("neutral/host.json");
    let bytes = fs::read(&path).expect("neutral metadata");
    let mut json: serde_json::Value = serde_json::from_slice(&bytes).expect("neutral JSON");
    json.as_object_mut()
        .expect("object")
        .remove("setup_attempted");
    fs::write(
        &path,
        serde_json::to_vec(&json).expect("simulate missing required production history"),
    )
    .expect("retain damaged metadata");
    assert!(BootstrapConfig::open(&root.join("neutral")).is_err());
    fs::remove_dir_all(root).expect("isolated cleanup");
}

#[test]
fn explicit_configuration_classifies_absence_and_never_selects_an_issuer() {
    let root = root();
    let mut config = crate::config::test_config(&root);
    config.idp.ui_public_uri = "https://app.example".into();
    config.idp.oauth2.issuer = "https://caller-chosen.invalid/idp".into();
    let host = BootstrapConfig::create_explicit(config.clone()).expect("absent configured roots");
    assert!(host.config().idp.oauth2.issuer.is_empty());
    let setup = SetupHost::create(host).expect("explicit neutral setup");
    let ca = fs::read(setup.tls().ca_path()).expect("retained CA");
    drop(setup);
    let reopened = BootstrapConfig::open_explicit(config.clone()).expect("exact configured host");
    let setup = SetupHost::open(reopened).expect("exact neutral reopen");
    assert_eq!(fs::read(setup.tls().ca_path()).expect("unchanged CA"), ca);
    drop(setup);
    let mut conflicting = config.clone();
    conflicting.idp.data_dir = root.join("different-idp").to_string_lossy().into_owned();
    assert!(BootstrapConfig::open_explicit(conflicting).is_err());
    fs::remove_file(crate::LocalTls::directory(&config).join("server.key")).expect("TLS loss");
    assert!(
        SetupHost::open(BootstrapConfig::open_explicit(config.clone()).expect("host")).is_err()
    );
    assert!(
        !crate::LocalTls::directory(&config)
            .join("server.key")
            .exists()
    );
    fs::remove_dir_all(&root).expect("isolated cleanup");
    for field in [
        "idp",
        "management",
        "storage",
        "endpoint",
        "credentials",
        "provisioning",
        "tls",
    ] {
        let root = root.join(field);
        let mut config = crate::config::test_config(&root);
        config.idp.ui_public_uri = "https://app.example".into();
        let retained = match field {
            "idp" => PathBuf::from(&config.idp.data_dir),
            "management" => PathBuf::from(&config.management.data_dir),
            "storage" => PathBuf::from(&config.storage.data_dir),
            "endpoint" => config.endpoint_data_dir.clone(),
            "credentials" => config.credential_bootstrap_directory(),
            "provisioning" => config.local_provisioning_directory(),
            _ => crate::LocalTls::directory(&config),
        };
        fs::create_dir_all(&retained).expect("simulate retained root with lost state");
        fs::write(retained.join("sentinel"), "do not replace").expect("preexisting work");
        assert!(BootstrapConfig::create_explicit(config.clone()).is_err());
        assert!(!BootstrapConfig::explicit_directory(&config).exists());
        assert_eq!(
            fs::read_to_string(retained.join("sentinel")).expect("unchanged retained work"),
            "do not replace"
        );
        fs::remove_dir_all(root).expect("isolated cleanup");
    }
    fs::remove_dir_all(root).expect("isolated parent cleanup");
}

#[cfg(target_os = "linux")]
#[test]
fn setup_https_uses_isolated_native_roots_and_never_installs_real_trust() {
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let root = root();
    let host =
        BootstrapConfig::create(&root, BootstrapPurpose::AwaitingChoice, None).expect("host");
    let setup = SetupHost::create(host).expect("fresh setup");
    let bundle = setup.tls().ca_path();
    let ca = fs::read(&bundle).expect("retained CA");
    assert!(
        setup.tls().test_install_os_ca(|_| false).is_err(),
        "injected OS trust-install failure refuses setup"
    );
    assert_eq!(
        fs::read(&bundle).expect("unchanged CA after installation failure"),
        ca
    );
    drop(setup);
    let empty = root.join("empty-native-roots");
    fs::create_dir(&empty).expect("isolated cert directory");
    let empty_bundle = root.join("empty.pem");
    fs::write(&empty_bundle, "").expect("empty native CA bundle");
    for trusted in [true, false] {
        let mut child = Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "setup_host_tests::setup_https_child",
                "--ignored",
                "--nocapture",
            ])
            .env("OF_SETUP_TEST_ROOT", &root)
            .env("OF_SETUP_TEST_TRUSTED", if trusted { "yes" } else { "no" })
            .env(
                "SSL_CERT_FILE",
                if trusted { &bundle } else { &empty_bundle },
            )
            .env("SSL_CERT_DIR", &empty)
            .stdin(Stdio::null())
            .spawn()
            .expect("isolated native-root child");
        let deadline = Instant::now() + Duration::from_secs(30);
        let status = loop {
            if let Some(status) = child.try_wait().expect("child status") {
                break status;
            }
            if Instant::now() >= deadline {
                child.kill().expect("bound child");
                child.wait().expect("reap child");
                panic!("setup HTTPS child exceeded 30 seconds");
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        assert!(status.success(), "isolated HTTPS child must succeed");
    }
    fs::remove_dir_all(root).expect("isolated cleanup");
}

#[cfg(target_os = "linux")]
#[tokio::test]
#[ignore = "parent supplies isolated native CA roots; never uses real OS trust"]
async fn setup_https_child() {
    use std::time::Duration;

    use reqwest::{
        StatusCode,
        header::{ACCESS_CONTROL_ALLOW_CREDENTIALS, ACCESS_CONTROL_ALLOW_ORIGIN},
    };
    use tokio_util::sync::CancellationToken;
    let root = PathBuf::from(std::env::var_os("OF_SETUP_TEST_ROOT").expect("isolated parent root"));
    let setup = SetupHost::open(BootstrapConfig::open(&root).expect("retained neutral host"))
        .expect("setup reopen");
    let base = format!("https://{}", setup.local_addr());
    let origin = base.clone();
    setup
        .tls()
        .test_install_os_ca(|command| {
            assert_eq!(command.get_program(), "trust");
            true
        })
        .expect("injected trust installation only");
    let client = crate::LocalTls::verification_client().expect("native-root verification client");
    let cancellation = CancellationToken::new();
    if std::env::var("OF_SETUP_TEST_TRUSTED").expect("trust case") == "no" {
        assert!(
            setup
                .serve_verified(client, cancellation, false)
                .await
                .is_err()
        );
        let listener = tokio::net::TcpListener::bind(base.trim_start_matches("https://"))
            .await
            .expect("failed verification released listener");
        drop(listener);
        return;
    }
    let code_file =
        fs::read_to_string(setup.admission_path()).expect("protected management code retrieval");
    let code = code_file.lines().nth(1).expect("private code").to_owned();
    let server = tokio::spawn(setup.serve_verified(client.clone(), cancellation.clone(), false));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let response = loop {
        if let Ok(response) = client.get(format!("{base}/idp/setup/status")).send().await {
            break response;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "bounded listener startup"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    assert_eq!(response.status(), StatusCode::OK);
    let text = response.text().await.expect("status JSON");
    assert!(!text.contains(&code));
    let status: serde_json::Value = serde_json::from_str(&text).expect("JSON status");
    assert_eq!(
        status,
        serde_json::json!({"stage": "installation", "state": "awaiting_choice", "ready": false, "owners": ["idp", "management", "storage"]})
    );
    let admission = format!("{base}/idp/setup/admission");
    for authorized in [false, true] {
        let mut request = client.post(&admission);
        if authorized {
            request = request.header("Authorization", format!("Setup {code}"));
        }
        let response = request.send().await.expect("native HTTPS admission");
        assert_eq!(
            response.status(),
            if authorized {
                StatusCode::NO_CONTENT
            } else {
                StatusCode::UNAUTHORIZED
            }
        );
    }
    let response = client
        .post(&admission)
        .header("Origin", &origin)
        .send()
        .await
        .expect("origin is not admission proof");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let response = client
        .post(&admission)
        .header("Cookie", format!("setup={code}"))
        .send()
        .await
        .expect("cookie alone");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let response = client
        .post(&admission)
        .header("Authorization", format!("Setup {}", "0".repeat(64)))
        .send()
        .await
        .expect("competing caller");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    for other in ["https://evil.example", "null"] {
        let response = client
            .post(&admission)
            .header("Origin", other)
            .header("Authorization", format!("Setup {code}"))
            .send()
            .await
            .expect("wrong browser origin");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(
            response
                .headers()
                .get(ACCESS_CONTROL_ALLOW_ORIGIN)
                .is_none()
        );
    }
    let response = client
        .post(&admission)
        .header("Origin", &origin)
        .header("Authorization", format!("Setup {code}"))
        .send()
        .await
        .expect("allowed browser origin with code");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        response
            .headers()
            .get(ACCESS_CONTROL_ALLOW_ORIGIN)
            .expect("exact CORS origin"),
        origin.as_str()
    );
    assert!(
        response
            .headers()
            .get(ACCESS_CONTROL_ALLOW_CREDENTIALS)
            .is_none()
    );
    let response = client
        .request(reqwest::Method::OPTIONS, &admission)
        .header("Origin", &origin)
        .header("Access-Control-Request-Method", "POST")
        .header("Access-Control-Request-Headers", "authorization")
        .send()
        .await
        .expect("explicit authorization preflight");
    assert!(response.status().is_success());
    assert_eq!(
        response
            .headers()
            .get(ACCESS_CONTROL_ALLOW_ORIGIN)
            .expect("preflight CORS"),
        origin.as_str()
    );
    for path in [
        "/idp/setup/create",
        "/idp/setup/join",
        "/management/permissions/evaluate",
        "/storage",
    ] {
        assert_eq!(
            client
                .post(format!("{base}{path}"))
                .header("Authorization", format!("Setup {code}"))
                .send()
                .await
                .expect("no resource/setup mutation routes")
                .status(),
            StatusCode::NOT_FOUND
        );
    }
    let response = client
        .get(format!("{base}/idp/setup/status"))
        .send()
        .await
        .expect("verification is not completion");
    assert_eq!(
        response
            .json::<serde_json::Value>()
            .await
            .expect("incomplete status")["ready"],
        false
    );
    let admission_path = root.join("setup-admission/state");
    fs::rename(&admission_path, root.join("held-state")).expect("simulate live admission loss");
    for restored in [false, true] {
        if restored {
            fs::rename(root.join("held-state"), &admission_path)
                .expect("restore exact fixture state without clearing live poison");
        }
        let response = client
            .get(format!("{base}/idp/setup/status"))
            .send()
            .await
            .expect("live unavailable status");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let text = response.text().await.expect("generic unavailable JSON");
        assert!(!text.contains(&code));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&text).expect("status JSON"),
            serde_json::json!({"stage": "installation", "state": "unavailable", "ready": false, "owners": ["idp", "management", "storage"]})
        );
        let response = client
            .post(&admission)
            .header("Authorization", format!("Setup {code}"))
            .send()
            .await
            .expect("lost or poisoned admission fails closed");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
    cancellation.cancel();
    tokio::time::timeout(Duration::from_secs(10), server)
        .await
        .expect("bounded shutdown")
        .expect("server task")
        .expect("verified setup serving");
    for abort in [false, true] {
        let setup = SetupHost::open(
            BootstrapConfig::open(&root).expect("host reopened after graceful shutdown"),
        )
        .expect("admission reopened after graceful shutdown");
        let cancellation = CancellationToken::new();
        let mut serving =
            Box::pin(setup.serve_verified(client.clone(), cancellation.clone(), false));
        if abort {
            let task = tokio::spawn(serving);
            await_setup_status(&client, &base).await;
            assert!(
                BootstrapConfig::open(&root).is_err(),
                "serving owns host lock"
            );
            task.abort();
            assert!(task.await.expect_err("aborted parent task").is_cancelled());
        } else {
            tokio::select! {
                result = &mut serving => panic!("host exited before drop test: {result:?}"),
                () = await_setup_status(&client, &base) => (),
            };
            assert!(
                BootstrapConfig::open(&root).is_err(),
                "serving owns host lock"
            );
            drop(serving);
        }
        let released = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Ok(listener) =
                    tokio::net::TcpListener::bind(base.trim_start_matches("https://")).await
                    && let Ok(host) = BootstrapConfig::open(&root)
                    && let Ok(setup) = SetupHost::open(host)
                {
                    drop((setup, listener));
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await;
        let cancelled_on_drop = cancellation.is_cancelled();
        cancellation.cancel();
        assert!(
            released.is_ok(),
            "dropped/aborted serving must release listener AND host/admission locks"
        );
        assert!(
            cancelled_on_drop,
            "dropping serving must signal coordinated cancellation"
        );
    }
}

#[cfg(target_os = "linux")]
async fn await_setup_status(client: &reqwest::Client, base: &str) {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if let Ok(response) = client
            .get(format!("{base}/idp/setup/status"))
            .header("Connection", "close")
            .send()
            .await
        {
            assert_eq!(response.status(), reqwest::StatusCode::OK);
            response
                .bytes()
                .await
                .expect("consume status before stopping host");
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "bounded listener startup"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}
