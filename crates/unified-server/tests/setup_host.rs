#![cfg(unix)]

use std::{fs, path::PathBuf};

use unified_server::{BootstrapConfig, BootstrapPurpose, SetupHost};

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
    assert!(!unified_server::LocalTls::directory(&config).exists());
    fs::remove_dir_all(root).expect("isolated cleanup");
}
