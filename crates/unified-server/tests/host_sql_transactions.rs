#![cfg(unix)]

use std::{fs, io, os::unix::fs::PermissionsExt, sync::Arc};
use api::{SetupAdmission, SetupAdmissionHistory};
use axum::http::HeaderMap;
use idp_model::model::Id;
use serde_json::json;
use unified_server::{BootstrapConfig, BootstrapPurpose, SetupHost, UnifiedConfig};

// Transaction fault coverage is below the lifecycle seam: no public test-only API.
#[allow(dead_code, reason = "compile the private store at its transaction fault seam")]
#[path = "../src/host_store.rs"]
mod host_store;

#[test]
fn failed_completion_transaction_preserves_authority_and_retirement_survives_cleanup_failure() {
    let root = std::env::temp_dir().join(format!("host-sql-atomic-{}", Id::now_v7()));
    let host = BootstrapConfig::create(&root, BootstrapPurpose::AwaitingChoice, None).expect("fresh host");
    let setup = SetupHost::create_http(host).expect("no OS trust");
    let path = root.join("setup-admission");
    let code = SetupHost::protected_code(&path).expect("protected retrieval").expect("live code");
    drop(setup);
    let store = host_store::HostStore::open(&root).expect("private store");
    let admission = SetupAdmission::open_with_history(&path, Arc::new(store.clone())).expect("retained admission");
    let mut headers = HeaderMap::new();
    headers.insert("Authorization", format!("Setup {code}").parse().expect("header"));
    let canonical = idp_server::hosted_setup::CreateIntent { operation_nonce: "0123456789abcdef0123456789abcdef".into(), administrator: "operator".into(), node: "Atomic Node".into() }.canonical().expect("canonical operation");
    admission.bind(&headers, &canonical).expect("exact binding");
    store.put("create", &json!({"storage_initialized": false, "complete": false})).expect("owner phase fault");
    assert!(store.retire(&canonical).is_err(), "unacknowledged owners cannot complete");
    assert!(!admission.is_retired().expect("previous live state"));
    assert_eq!(store.get::<serde_json::Value>("create").expect("readback").expect("Create")["complete"], false);
    let failed: io::Result<()> = store.update(|records| {
        records.get_mut("create").expect("Create")["complete"] = json!(true);
        records.get_mut("admission").expect("admission")["retired"] = json!(true);
        Err(io::Error::other("injected failure before commit"))
    });
    assert!(failed.is_err());
    admission.check(&headers).expect("rolled back transaction retains live authority");
    assert_eq!(store.get::<serde_json::Value>("create").expect("rollback readback").expect("Create")["complete"], false);
    store.put("create", &json!({"storage_initialized": true, "complete": false})).expect("owner acknowledgement");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o500)).expect("inject code cleanup permission failure");
    admission.retire(&headers, &canonical).expect("atomic completion and retirement");
    assert_eq!(store.get::<serde_json::Value>("create").expect("committed readback").expect("Create")["complete"], true);
    assert!(path.join("state").exists(), "failed cleanup leaves credential evidence");
    assert!(admission.check(&headers).is_err(), "leftover code grants no authority");
    drop(admission);
    assert_eq!(SetupHost::protected_code(&path).expect("retired protected retrieval"), None);
    let admission = SetupAdmission::open_with_history(&path, Arc::new(store)).expect("retired reopen");
    assert!(admission.is_retired().expect("durable retirement"));
    assert!(admission.check(&headers).is_err());
    drop(admission);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).expect("restore fixture permissions");
    fs::remove_dir_all(root).expect("isolated cleanup");
}
