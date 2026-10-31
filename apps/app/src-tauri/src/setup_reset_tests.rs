use std::fs;

#[tokio::test]
async fn reset_uses_the_stopped_owner_reset_boundaries() {
    let data = std::env::temp_dir().join(format!("native-owner-reset-{}", uuid::Uuid::new_v4()));
    drop(crate::native_host::prepare(&data.join("node")).expect("owner prerequisites"));
    super::run(&data).await.expect("reset each fresh owner");
    assert!(!data.join("node").exists());
    assert!(!super::pending(&data).expect("completed reset"));
    fs::remove_dir_all(data).expect("cleanup");
}

#[test]
fn legacy_reference_recovery_refuses_without_reprovisioning() {
    let data = std::env::temp_dir().join(format!("native-legacy-reset-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(data.join("node")).expect("legacy root");
    fs::write(data.join("node/host.redb"), b"retained legacy evidence").expect("old state");
    assert!(super::recover(&data).is_err());
    assert_eq!(
        fs::read(data.join("node/host.redb")).expect("preserve old state"),
        b"retained legacy evidence"
    );
    fs::remove_dir_all(data).expect("cleanup");
}
