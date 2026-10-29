use std::{fs, io, os::unix::fs::PermissionsExt, path::PathBuf};

use serde_json::Value;

fn fixture() -> PathBuf {
    let data = std::env::temp_dir().join(format!("native-reset-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&data).expect("isolated native profile");
    fs::set_permissions(&data, fs::Permissions::from_mode(0o700)).expect("private profile");
    drop(crate::native_host::prepare(&data.join("node")).expect("neutral shared host"));
    data
}

#[tokio::test]
async fn failed_secret_cleanup_preserves_manifests_and_reset_is_explicitly_retryable() {
    let data = fixture();
    let original = crate::native_host::tests::retained_identity(&data.join("node"));
    let result = super::run_with(&data, |_, _| {
        Box::pin(async { Err(io::Error::other("isolated provider refusal")) })
    })
    .await;
    assert!(result.is_err());
    assert!(super::pending(&data).expect("pending reset"));
    assert_eq!(
        crate::native_host::tests::retained_identity(&data.join("node")),
        original
    );
    assert!(data.join("setup-reference-backup.json").is_file());
    super::run(&data).await.expect("explicit exact reset retry");
    assert!(!data.join("node").exists());
    assert!(!super::pending(&data).expect("completed reset"));
    assert!(!data.join("setup-reference-backup.json").exists());
    fs::remove_dir_all(data).expect("isolated cleanup");
}

#[tokio::test]
async fn lost_host_identity_requires_explicit_exact_reference_recovery() {
    let data = fixture();
    let _ = super::run_with(&data, |_, _| {
        Box::pin(async { Err(io::Error::other("stop before data deletion")) })
    })
    .await;
    let host = data.join("node/host.redb");
    let original = fs::read(&host).expect("exact host identity");
    fs::remove_file(&host).expect("inject lost reference");
    assert!(super::run(&data).await.is_err());
    assert!(!host.exists());
    assert!(
        super::recover(&data).is_err(),
        "reference recovery cannot reconstruct a lost host database"
    );
    assert!(!host.exists());
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&host)
        .expect("operator restores exact database backup")
        .write_all(&original)
        .expect("restore database");
    super::recover(&data).expect("exact database restored; validate references");
    super::run(&data).await.expect("reset original host only");
    fs::remove_dir_all(data).expect("isolated cleanup");
}

#[tokio::test]
async fn damaged_provisioning_without_authoritative_references_is_never_discarded() {
    let data = fixture();
    fs::create_dir(data.join("node/idp")).expect("inject unexplained owner state");
    assert!(super::run(&data).await.is_err());
    assert!(data.join("node/idp").exists());
    assert!(!data.join("reset-state.json").exists());
    fs::remove_dir_all(data).expect("isolated cleanup");
}

#[tokio::test]
async fn interrupted_data_cleanup_resumes_without_touching_a_provider_again() {
    let data = fixture();
    let _ = super::run_with(&data, |_, _| {
        Box::pin(async { Err(io::Error::other("retain original references")) })
    })
    .await;
    let bytes = super::read_optional(&data.join("reset-state.json"))
        .expect("journal access")
        .expect("journal");
    let mut record: Value = serde_json::from_slice(&bytes).expect("retained record");
    record["phase"] = serde_json::json!("data-pending");
    super::publish(&data, "reset-state.json", &record)
        .expect("simulate durable successful key deletion");
    fs::remove_file(data.join("node/host.redb")).expect("simulate interrupted directory deletion");
    super::run_with(&data, |_, _| {
        Box::pin(async { panic!("must never recreate owners after key deletion") })
    })
    .await
    .expect("finish data-only reset");
    assert!(!data.join("node").exists());
    fs::remove_dir_all(data).expect("isolated cleanup");
}

#[test]
fn reset_reference_recovery_rejects_symlink_and_conflicting_evidence() {
    let data = fixture();
    let outside = data.join("outside");
    fs::write(&outside, b"original").expect("unrelated data");
    std::os::unix::fs::symlink(&outside, data.join("reset-state.json")).expect("inject symlink");
    assert!(super::pending(&data).is_err());
    assert_eq!(
        fs::read(outside).expect("unrelated data intact"),
        b"original"
    );
    fs::remove_dir_all(data).expect("isolated cleanup");
}

#[tokio::test]
async fn retained_join_reset_preserves_exact_owner_ids_and_never_starts_an_endpoint_or_provider() {
    use unified_server::{BootstrapConfig, BootstrapPurpose, CredentialBootstrap, LocalJoin};

    let data = std::env::temp_dir().join(format!("native-join-reset-{}", uuid::Uuid::new_v4()));
    let root = data.join("node");
    let peer = iroh::SecretKey::generate().public().to_string();
    let host = BootstrapConfig::create(
        &root,
        BootstrapPurpose::Join,
        Some((&peer, "https://installation.test/idp")),
    )
    .expect("authenticated peer/issuer fixture");
    let credentials = CredentialBootstrap::create(host.config())
        .expect("durable public credential references only");
    let joining = LocalJoin::create(
        credentials.load().expect("same issuer/config"),
        &peer,
        "Joining Node",
    )
    .expect("retained Join before owner production");
    drop(joining);
    drop(credentials);
    let references = host
        .local_reset_references()
        .expect("exact shared Join references");
    assert_eq!(references["issuer"], "https://installation.test/idp");
    drop(host);
    let journal = root.join("endpoint.provisioning/join.json");
    let original = fs::read(&journal).expect("original Join identity and production history");
    let result = super::run_with(&data, |_, _| {
        Box::pin(async { Err(io::Error::other("injected cleanup refusal")) })
    })
    .await;
    assert!(result.is_err());
    assert_eq!(
        fs::read(&journal).expect("Join retained after refusal"),
        original
    );
    fs::remove_file(&journal).expect("inject lost Join reference");
    assert!(super::run(&data).await.is_err());
    super::recover(&data).expect("explicit exact Join reference recovery");
    assert_eq!(
        fs::read(&journal).expect("original Join restored"),
        original
    );
    let host = BootstrapConfig::open(&root).expect("exact retained Join host");
    assert_eq!(
        host.local_reset_references().expect("same Join IDs"),
        references
    );
    drop(host);
    assert!(!root.join("endpoint").exists());
    assert!(!root.join("idp").exists());
    super::run(&data)
        .await
        .expect("reset never-started Join without provider access");
    assert!(!root.exists());
    fs::remove_dir_all(data).expect("isolated cleanup");
}
