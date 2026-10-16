use std::{fs, os::unix::fs::PermissionsExt, sync::Arc};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    middleware,
    routing::post,
};
use tower::ServiceExt;

use crate::{SetupAdmission, SetupAdmissionError, setup_admission_middleware};

fn fixture() -> (
    tempfile::TempDir,
    std::path::PathBuf,
    SetupAdmission,
    axum::http::HeaderMap,
    String,
) {
    let parent = tempfile::tempdir().expect("test parent");
    fs::set_permissions(parent.path(), fs::Permissions::from_mode(0o700)).expect("private parent");
    let root = parent.path().join("admission");
    let admission = SetupAdmission::open(&root).expect("fresh admission");
    let text = fs::read_to_string(root.join("state")).expect("protected management retrieval");
    let code = text.lines().nth(1).expect("code").to_owned();
    assert_eq!(code.len(), 64);
    assert!(
        code.bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        "authorization",
        format!("Setup {code}").parse().expect("header"),
    );
    (parent, root, admission, headers, code)
}

#[test]
fn health_checks_private_state_without_authorizing_and_failure_is_sticky() {
    for fault in ["missing", "corrupt", "permissions", "pending", "lock"] {
        let (_parent, root, admission, headers, _code) = fixture();
        admission.health().expect("valid private retained state");
        assert_eq!(
            admission.check(&axum::http::HeaderMap::new()),
            Err(SetupAdmissionError::Denied)
        );
        let state = root.join("state");
        let original = fs::read(&state).expect("retain exact fixture state");
        match fault {
            "missing" => fs::rename(&state, root.join("held-state")).expect("simulate loss"),
            "corrupt" => fs::write(&state, "corrupt").expect("simulate corruption"),
            "permissions" => fs::set_permissions(&state, fs::Permissions::from_mode(0o644))
                .expect("unsafe state permissions"),
            "pending" => fs::write(root.join("pending"), "incomplete").expect("incomplete update"),
            _ => {
                fs::rename(root.join("lock"), root.join("held-lock")).expect("move held lock");
                fs::copy(root.join("held-lock"), root.join("lock")).expect("replace lock inode");
            }
        }
        assert_eq!(admission.health(), Err(SetupAdmissionError::Unavailable));
        match fault {
            "missing" => {
                fs::rename(root.join("held-state"), &state).expect("external fixture restoration")
            }
            "corrupt" => fs::write(&state, original).expect("external fixture restoration"),
            "permissions" => fs::set_permissions(&state, fs::Permissions::from_mode(0o600))
                .expect("external fixture restoration"),
            "pending" => {
                fs::remove_file(root.join("pending")).expect("external fixture restoration")
            }
            _ => {
                fs::remove_file(root.join("lock")).expect("remove replacement fixture inode");
                fs::rename(root.join("held-lock"), root.join("lock"))
                    .expect("external fixture restoration");
            }
        }
        assert_eq!(admission.health(), Err(SetupAdmissionError::Unavailable));
        assert_eq!(
            admission.is_retired(),
            Err(SetupAdmissionError::Unavailable)
        );
        assert_eq!(
            admission.check(&headers),
            Err(SetupAdmissionError::Unavailable)
        );
    }
}

#[test]
fn healthy_retired_state_is_not_admission_proof() {
    let (_parent, root, admission, headers, _code) = fixture();
    assert!(!admission.is_retired().expect("fresh unretired state"));
    admission
        .bind(&headers, "create:health-test")
        .expect("authorized binding");
    admission
        .retire(&headers, "create:health-test")
        .expect("authorized retirement");
    admission.health().expect("valid retained retirement state");
    assert!(admission.is_retired().expect("durable retirement"));
    assert_eq!(admission.check(&headers), Err(SetupAdmissionError::Denied));
    drop(admission);
    let reopened = SetupAdmission::open(&root).expect("reopen retired state");
    assert!(reopened.is_retired().expect("retirement survives restart"));
    assert_eq!(reopened.check(&headers), Err(SetupAdmissionError::Denied));
}

#[test]
fn bound_intent_is_authorized_read_only_and_survives_reopen() {
    let (_parent, root, admission, headers, _code) = fixture();
    let initial = fs::read(root.join("state")).expect("initial state");
    assert_eq!(admission.bound_intent(&headers), Ok(None));
    assert_eq!(
        fs::read(root.join("state")).expect("unchanged state"),
        initial
    );
    assert_eq!(
        admission.bound_intent(&axum::http::HeaderMap::new()),
        Err(SetupAdmissionError::Denied)
    );
    let mut wrong = headers.clone();
    wrong.insert(
        "authorization",
        format!("Setup {}", "0".repeat(64))
            .parse()
            .expect("wrong code header"),
    );
    assert_eq!(
        admission.bound_intent(&wrong),
        Err(SetupAdmissionError::Denied)
    );
    let mut duplicate = headers.clone();
    duplicate.append("authorization", headers["authorization"].clone());
    assert_eq!(
        admission.bound_intent(&duplicate),
        Err(SetupAdmissionError::Denied)
    );
    admission
        .bind(&headers, "create:retained-operation")
        .expect("durable binding");
    let bound = fs::read(root.join("state")).expect("bound state");
    drop(admission);
    let admission = SetupAdmission::open(&root).expect("durable reopen");
    assert_eq!(
        admission.bound_intent(&headers),
        Ok(Some("create:retained-operation".into()))
    );
    assert_eq!(
        fs::read(root.join("state")).expect("unchanged bound state"),
        bound
    );
    admission
        .retire(&headers, "create:retained-operation")
        .expect("retirement");
    assert_eq!(
        admission.bound_intent(&headers),
        Err(SetupAdmissionError::Denied)
    );
    drop(admission);
    let admission = SetupAdmission::open(&root).expect("retired reopen");
    assert_eq!(
        admission.bound_intent(&headers),
        Err(SetupAdmissionError::Denied)
    );
}

#[test]
fn bound_intent_denies_corrupt_state_and_failure_remains_sticky() {
    let (_parent, root, admission, headers, _code) = fixture();
    let original = fs::read(root.join("state")).expect("initial state");
    fs::write(root.join("state"), "corrupt").expect("corrupt retained state");
    assert_eq!(
        admission.bound_intent(&headers),
        Err(SetupAdmissionError::Unavailable)
    );
    assert_eq!(
        fs::read(root.join("state")).expect("no regeneration"),
        b"corrupt"
    );
    fs::write(root.join("state"), original).expect("external restoration");
    assert_eq!(
        admission.bound_intent(&headers),
        Err(SetupAdmissionError::Unavailable)
    );
}

#[test]
fn binding_retries_reopen_and_explicit_retirement_are_durable() {
    let (_parent, root, admission, headers, code) = fixture();
    assert_eq!(
        admission.retire(&headers, "create:operation-1"),
        Err(SetupAdmissionError::Conflict)
    );
    for invalid in ["", "create\npassword", "\0"] {
        assert_eq!(
            admission.bind(&headers, invalid),
            Err(SetupAdmissionError::InvalidIntent)
        );
    }
    assert_eq!(
        admission.bind(&headers, &"a".repeat(1025)),
        Err(SetupAdmissionError::InvalidIntent)
    );
    admission
        .bind(&headers, "create:operation-1")
        .expect("first bind");
    admission
        .bind(&headers, "create:operation-1")
        .expect("exact retry");
    assert_eq!(
        admission.bind(&headers, "join:operation-1"),
        Err(SetupAdmissionError::Conflict)
    );
    drop(admission);
    let admission = SetupAdmission::open(&root).expect("offline reopen");
    admission.check(&headers).expect("same retained code");
    admission
        .bind(&headers, "create:operation-1")
        .expect("same bound intent");
    assert_eq!(
        admission.bind(&headers, "create:operation-2"),
        Err(SetupAdmissionError::Conflict)
    );
    assert_eq!(
        admission.retire(&headers, "join:operation-1"),
        Err(SetupAdmissionError::Conflict)
    );
    admission
        .retire(&headers, "create:operation-1")
        .expect("explicit completion");
    assert_eq!(admission.check(&headers), Err(SetupAdmissionError::Denied));
    assert!(
        !fs::read_to_string(root.join("state"))
            .expect("retired state")
            .contains(&code)
    );
    drop(admission);
    let admission = SetupAdmission::open(&root).expect("retired reopen");
    assert_eq!(admission.check(&headers), Err(SetupAdmissionError::Denied));
    assert_eq!(
        admission.bind(&headers, "create:operation-1"),
        Err(SetupAdmissionError::Denied)
    );
    assert!(!format!("{admission:?}").contains(&code));
    for error in [
        SetupAdmissionError::Denied,
        SetupAdmissionError::Conflict,
        SetupAdmissionError::InvalidIntent,
        SetupAdmissionError::Unavailable,
    ] {
        assert!(!format!("{error:?} {error}").contains(&code));
    }
}

#[tokio::test]
async fn setup_handler_binds_before_work_without_changing_other_routes() {
    use axum::extract::State;
    async fn bind_operation(
        State(admission): State<Arc<SetupAdmission>>,
        headers: axum::http::HeaderMap,
        intent: String,
    ) -> Result<StatusCode, SetupAdmissionError> {
        tokio::task::spawn_blocking(move || admission.bind(&headers, &intent))
            .await
            .expect("binding task")?;
        Ok(StatusCode::NO_CONTENT)
    }
    let (_parent, root, admission, headers, code) = fixture();
    let admission = Arc::new(admission);
    let app = Router::new()
        .route("/setup", post(bind_operation))
        .route_layer(middleware::from_fn_with_state(
            Arc::clone(&admission),
            setup_admission_middleware,
        ))
        .route("/ordinary", post(|| async { StatusCode::FORBIDDEN }))
        .with_state(admission);
    for (intent, expected) in [
        ("create:http-1", StatusCode::NO_CONTENT),
        ("create:http-1", StatusCode::NO_CONTENT),
        ("join:http-1", StatusCode::CONFLICT),
        ("", StatusCode::BAD_REQUEST),
    ] {
        let mut request = Request::builder()
            .method("POST")
            .uri("/setup")
            .body(Body::from(intent))
            .expect("request");
        *request.headers_mut() = headers.clone();
        let response = app.clone().oneshot(request).await.expect("response");
        assert_eq!(response.status(), expected);
        assert!(
            axum::body::to_bytes(response.into_body(), 1024)
                .await
                .expect("body")
                .is_empty()
        );
    }
    let request = Request::builder()
        .method("POST")
        .uri("/ordinary")
        .header("authorization", format!("Setup {code}"))
        .body(Body::empty())
        .expect("ordinary request");
    assert_eq!(
        app.clone()
            .oneshot(request)
            .await
            .expect("ordinary response")
            .status(),
        StatusCode::FORBIDDEN
    );
    drop(app);
    let admission = SetupAdmission::open(&root).expect("HTTP binding persisted");
    admission
        .bind(&headers, "create:http-1")
        .expect("exact HTTP intent retry");
    assert_eq!(
        admission.bind(&headers, "join:http-1"),
        Err(SetupAdmissionError::Conflict)
    );
}

#[test]
fn process_lock_probe() {
    if let Some(root) = std::env::var_os("OF_SETUP_ADMISSION_LOCK_PROBE_PATH") {
        assert_eq!(
            SetupAdmission::open(std::path::Path::new(&root)).expect_err("other process owns lock"),
            SetupAdmissionError::Unavailable
        );
    }
}

#[test]
fn live_handle_holds_exclusive_lock_until_drop() {
    let (_parent, root, admission, headers, _code) = fixture();
    assert_eq!(
        SetupAdmission::open(&root).expect_err("second handle must deny"),
        SetupAdmissionError::Unavailable
    );
    let child = std::process::Command::new(std::env::current_exe().expect("test executable"))
        .args(["--exact", "setup_admission_tests::process_lock_probe"])
        .env("OF_SETUP_ADMISSION_LOCK_PROBE_PATH", &root)
        .output()
        .expect("child lock probe");
    assert!(child.status.success(), "child lock probe failed");
    admission
        .bind(&headers, "create:locked")
        .expect("first handle retains authority");
    drop(admission);
    SetupAdmission::open(&root).expect("released only on drop");
}

#[test]
fn concurrent_conflicting_operations_bind_only_one_intent() {
    let (_parent, root, admission, headers, _code) = fixture();
    let admission = Arc::new(admission);
    let mut handles = Vec::new();
    for intent in ["create:first", "join:second"] {
        let admission = Arc::clone(&admission);
        let headers = headers.clone();
        handles.push(std::thread::spawn(move || {
            (intent, admission.bind(&headers, intent))
        }));
    }
    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().expect("bind thread"))
        .collect();
    assert_eq!(
        results.iter().filter(|(_, result)| result.is_ok()).count(),
        1
    );
    assert_eq!(
        results
            .iter()
            .filter(|(_, result)| *result == Err(SetupAdmissionError::Conflict))
            .count(),
        1
    );
    let winner = results
        .iter()
        .find(|(_, result)| result.is_ok())
        .expect("one winner")
        .0;
    drop(admission);
    let reopened = SetupAdmission::open(&root).expect("bound state");
    reopened
        .bind(&headers, winner)
        .expect("winning exact retry");
}

#[test]
fn missing_corrupt_and_unsafe_retained_state_is_not_repaired() {
    use std::os::unix::fs::symlink;
    for damage in [
        "missing-state",
        "missing-lock",
        "corrupt",
        "state-permissions",
        "root-permissions",
        "symlink",
        "hardlink",
        "directory",
        "pending",
    ] {
        let (parent, root, admission, _headers, code) = fixture();
        drop(admission);
        let state = root.join("state");
        match damage {
            "missing-state" => fs::remove_file(&state).expect("remove state"),
            "missing-lock" => fs::remove_file(root.join("lock")).expect("remove lock"),
            "corrupt" => fs::write(&state, "not retained state").expect("corrupt fixture"),
            "state-permissions" => fs::set_permissions(&state, fs::Permissions::from_mode(0o644))
                .expect("unsafe state"),
            "root-permissions" => {
                fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).expect("unsafe root")
            }
            "symlink" => {
                fs::rename(&state, parent.path().join("saved")).expect("save state");
                symlink(parent.path().join("saved"), &state).expect("unsafe symlink");
            }
            "hardlink" => {
                fs::hard_link(&state, parent.path().join("alias")).expect("unsafe hardlink")
            }
            "directory" => {
                fs::remove_file(&state).expect("remove state");
                fs::create_dir(&state).expect("wrong file type");
            }
            "pending" => {
                fs::write(root.join("pending"), "interrupted update").expect("incomplete write")
            }
            _ => unreachable!(),
        }
        let before = fs::symlink_metadata(&state).ok().map(|metadata| {
            (
                metadata.file_type(),
                metadata.len(),
                metadata.permissions().mode(),
            )
        });
        assert_eq!(
            SetupAdmission::open(&root).expect_err(damage),
            SetupAdmissionError::Unavailable,
            "{damage}"
        );
        assert_eq!(
            before,
            fs::symlink_metadata(&state).ok().map(|metadata| (
                metadata.file_type(),
                metadata.len(),
                metadata.permissions().mode()
            )),
            "unchanged {damage}"
        );
        if damage == "corrupt" {
            assert_eq!(
                fs::read_to_string(&state).expect("preserved corruption"),
                "not retained state"
            );
        }
        if damage == "missing-state" {
            assert!(!state.exists());
        }
        if damage == "missing-lock" {
            assert!(!root.join("lock").exists());
        }
        if damage == "symlink" {
            assert!(
                fs::read_to_string(parent.path().join("saved"))
                    .expect("unchanged target")
                    .contains(&code)
            );
        }
    }
}

#[test]
fn incomplete_existing_root_and_unsafe_parents_never_generate() {
    use std::os::unix::fs::symlink;
    let parent = tempfile::tempdir().expect("test parent");
    fs::set_permissions(parent.path(), fs::Permissions::from_mode(0o700)).expect("private parent");
    let root = parent.path().join("existing");
    fs::create_dir(&root).expect("incomplete root");
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("private root");
    assert!(SetupAdmission::open(&root).is_err());
    assert_eq!(fs::read_dir(&root).expect("unchanged root").count(), 0);
    let alias = parent.path().join("alias");
    symlink(&root, &alias).expect("root symlink");
    assert!(SetupAdmission::open(&alias).is_err());
    assert!(SetupAdmission::open(&alias.join("nested")).is_err());
    fs::set_permissions(parent.path(), fs::Permissions::from_mode(0o755)).expect("unsafe parent");
    let fresh = parent.path().join("fresh");
    assert!(SetupAdmission::open(&fresh).is_err());
    assert!(!fresh.exists());
}

#[test]
fn failed_durable_write_poisoning_does_not_authorize_mutation() {
    // Root bypasses Unix DAC; this fault is meaningful only for an unprivileged host.
    if rustix::process::geteuid().is_root() {
        return;
    }
    let (_parent, root, admission, headers, _code) = fixture();
    let before = fs::read(root.join("state")).expect("unbound state");
    fs::set_permissions(&root, fs::Permissions::from_mode(0o500)).expect("deny update writes");
    admission
        .check(&headers)
        .expect("read admission still works");
    assert_eq!(
        admission.bind(&headers, "create:failed"),
        Err(SetupAdmissionError::Unavailable)
    );
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
        .expect("restore fixture permissions");
    assert!(
        fs::read(root.join("state")).expect("state not changed") == before,
        "failed update changed retained state"
    );
    assert_eq!(
        admission.bind(&headers, "join:other"),
        Err(SetupAdmissionError::Unavailable)
    );
    assert_eq!(
        admission.check(&headers),
        Err(SetupAdmissionError::Unavailable)
    );
    assert_eq!(admission.health(), Err(SetupAdmissionError::Unavailable));
    drop(admission);
    let reopened = SetupAdmission::open(&root).expect("disk stayed unbound");
    reopened
        .bind(&headers, "create:retry")
        .expect("explicit retry after reopen");
}

#[test]
fn invalid_live_state_poisoning_never_relaxes_after_local_repair() {
    let (_parent, root, admission, headers, _code) = fixture();
    let state = root.join("state");
    let original = fs::read(&state).expect("saved state");
    fs::write(&state, "corrupt").expect("damage state");
    assert_eq!(
        admission.bind(&headers, "create:one"),
        Err(SetupAdmissionError::Unavailable)
    );
    fs::write(&state, original).expect("external fixture repair");
    assert_eq!(
        admission.check(&headers),
        Err(SetupAdmissionError::Unavailable)
    );
    assert_eq!(
        admission.retire(&headers, "create:one"),
        Err(SetupAdmissionError::Unavailable)
    );
}

#[tokio::test]
async fn setup_http_requires_explicit_exact_code() {
    let parent = tempfile::tempdir().expect("private test parent");
    fs::set_permissions(parent.path(), fs::Permissions::from_mode(0o700)).expect("private parent");
    let root = parent.path().join("admission");
    let admission = Arc::new(SetupAdmission::open(&root).expect("fresh admission"));
    let code = fs::read_to_string(root.join("state"))
        .expect("protected management retrieval")
        .lines()
        .nth(1)
        .expect("retained code")
        .to_owned();
    let app = Router::new()
        .route("/setup", post(|| async { StatusCode::NO_CONTENT }))
        .route_layer(middleware::from_fn_with_state(
            admission,
            setup_admission_middleware,
        ));
    for headers in [
        vec![],
        vec!["Setup wrong".to_owned()],
        vec![format!("Setup {}", "0".repeat(64))],
        vec![format!("Setup {code} ")],
        vec![format!("Setup {code}, Setup {code}")],
        vec![format!("Bearer {code}")],
        vec![format!("Setup A{}", &code[1..])],
        vec![format!("Setup {code}"), format!("Setup {code}")],
    ] {
        let mut request = Request::builder()
            .method("POST")
            .uri("/setup")
            .body(Body::empty())
            .expect("request");
        for value in headers {
            request
                .headers_mut()
                .append("authorization", value.parse().expect("header"));
        }
        let response = app.clone().oneshot(request).await.expect("response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(
            axum::body::to_bytes(response.into_body(), 1024)
                .await
                .expect("body")
                .is_empty()
        );
    }
    let request = Request::builder()
        .method("POST")
        .uri("/setup")
        .header("authorization", format!("Setup {code}"))
        .body(Body::empty())
        .expect("request");
    assert_eq!(
        app.oneshot(request).await.expect("response").status(),
        StatusCode::NO_CONTENT
    );
}
