#![cfg(unix)]

use unified_server::{BootstrapConfig, BootstrapPurpose};

#[test]
fn ordinary_create_derives_and_retains_all_public_host_settings() {
    let root = std::env::temp_dir().join(format!(
        "unified-default-config-{}",
        idp_model::model::Id::now_v7()
    ));
    let host = BootstrapConfig::create(&root, BootstrapPurpose::Create, None)
        .expect("fresh automatic host");
    let config = host.config();
    assert_ne!(config.listen_addr.port(), 0);
    assert_eq!(
        config.idp.api_public_uri,
        format!("https://{}/idp", config.listen_addr)
    );
    assert_ne!(config.idp.api_public_uri, config.idp.oauth2.issuer);
    assert!(
        config
            .idp
            .oauth2
            .issuer
            .ends_with(".installation.invalid/idp")
    );
    assert!(
        !config
            .idp
            .oauth2
            .issuer
            .contains(&config.listen_addr.to_string())
    );
    assert_ne!(config.idp.data_dir, config.device.data_dir);
    assert_ne!(config.device.data_dir, config.storage.data_dir);
    drop(host);
    let reopened = BootstrapConfig::open(&root)
        .expect("retained host settings")
        .config();
    assert_eq!(reopened.listen_addr, config.listen_addr);
    assert_eq!(reopened.idp.key_namespace, config.idp.key_namespace);
    assert_eq!(reopened.idp.oauth2.issuer, config.idp.oauth2.issuer);
    assert!(BootstrapConfig::create(&root, BootstrapPurpose::Create, None).is_err());
    std::fs::remove_dir_all(root).expect("isolated cleanup");
}

#[test]
fn join_settings_require_peer_evidence_not_an_invented_installation() {
    let root = std::env::temp_dir().join(format!(
        "unified-default-join-config-{}",
        idp_model::model::Id::now_v7()
    ));
    assert!(BootstrapConfig::create(&root, BootstrapPurpose::Join, None).is_err());
    assert!(!root.exists());
    let peer = iroh::SecretKey::generate().public().to_string();
    let host = BootstrapConfig::create(
        &root,
        BootstrapPurpose::Join,
        Some((&peer, "https://trusted-installation.test/idp")),
    )
    .expect("authenticated peer result");
    assert_eq!(
        host.config().idp.oauth2.issuer,
        "https://trusted-installation.test/idp"
    );
    assert_eq!(host.trusted_peer(), Some(peer.as_str()));
    drop(host);
    std::fs::remove_file(root.join("host.json")).expect("simulate lost host identity");
    assert!(BootstrapConfig::open(&root).is_err());
    assert!(
        BootstrapConfig::create(
            &root,
            BootstrapPurpose::Join,
            Some((&peer, "https://trusted-installation.test/idp"))
        )
        .is_err()
    );
    std::fs::remove_dir_all(root).expect("isolated cleanup");
}
