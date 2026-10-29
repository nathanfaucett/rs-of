#![cfg(unix)]

use std::{fs, path::Path};

use unified_server::{BootstrapConfig, BootstrapPurpose, UnifiedConfig};

#[allow(
    dead_code,
    reason = "private SQL fault seam for retained configuration"
)]
#[path = "../src/host_store.rs"]
mod host_store;

fn retained_config(root: &Path) -> serde_json::Value {
    host_store::HostStore::open(root)
        .expect("existing host database")
        .get("config")
        .expect("read retained configuration")
        .expect("retained configuration")
}

fn explicit_config(root: &Path) -> UnifiedConfig {
    UnifiedConfig {
        listen_addr: "127.0.0.1:49152".parse().expect("local listener"),
        endpoint_data_dir: root.join("endpoint"),
        idp: idp_server::AppConfig {
            data_dir: root.join("idp").to_string_lossy().into_owned(),
            api_public_uri: "https://public.test/idp".into(),
            ui_public_uri: "https://public.test".into(),
            oauth2: idp_service::oauth2::OAuth2Config {
                issuer: "https://identity.test/custom-issuer".into(),
                ..Default::default()
            },
            ..Default::default()
        },
        management: Default::default(),
        device: device_server::AppConfig {
            data_dir: root.join("device").to_string_lossy().into_owned(),
            ..Default::default()
        },
        storage: storage_server::AppConfig {
            data_dir: root.join("storage").to_string_lossy().into_owned(),
            ..Default::default()
        },
    }
}

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
    assert_eq!(config.idp.oauth2.issuer, config.idp.api_public_uri);
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
fn automatic_neutral_create_selects_public_api_issuer() {
    let root = std::env::temp_dir().join(format!(
        "unified-neutral-create-{}",
        idp_model::model::Id::now_v7()
    ));
    let mut host = BootstrapConfig::create(&root, BootstrapPurpose::AwaitingChoice, None)
        .expect("neutral automatic host");
    assert!(host.config().idp.oauth2.issuer.is_empty());
    assert!(host.select_create().is_err());
    let public_uri = host.config().idp.api_public_uri;
    host.begin_setup().expect("retain setup production");
    host.select_create().expect("select automatic Create");
    assert_eq!(host.config().idp.oauth2.issuer, public_uri);
    drop(host);
    let host = BootstrapConfig::open(&root).expect("reopen automatic Create");
    assert_eq!(host.config().idp.oauth2.issuer, public_uri);
    drop(host);
    fs::remove_dir_all(root).expect("isolated cleanup");
}

#[test]
fn proxy_create_uses_public_api_issuer_not_internal_listener() {
    for base in ["https://proxy.test/", "http://localhost:49153/"] {
        for purpose in [BootstrapPurpose::Create, BootstrapPurpose::AwaitingChoice] {
            let root = std::env::temp_dir().join(format!(
                "unified-proxy-create-{}",
                idp_model::model::Id::now_v7()
            ));
            let mut host = BootstrapConfig::create_http(
                &root,
                purpose,
                None,
                "127.0.0.1:49152".parse().expect("internal listener"),
                base.into(),
            )
            .expect("proxy-facing host");
            if purpose == BootstrapPurpose::AwaitingChoice {
                assert!(host.config().idp.oauth2.issuer.is_empty());
                host.begin_setup().expect("retain setup production");
                host.select_create().expect("select proxy Create");
            }
            let expected = format!("{}/idp", base.trim_end_matches('/'));
            assert_eq!(host.config().idp.oauth2.issuer, expected);
            assert_eq!(host.config().idp.api_public_uri, expected);
            drop(host);
            let host = BootstrapConfig::open(&root).expect("reopen proxy identity");
            assert_eq!(host.config().idp.oauth2.issuer, expected);
            drop(host);
            fs::remove_dir_all(root).expect("isolated cleanup");
        }
    }
}

#[test]
fn explicit_custom_issuer_survives_neutral_setup_selection_and_reopen() {
    for https in [true, false] {
        let root = std::env::temp_dir().join(format!(
            "unified-explicit-issuer-{}",
            idp_model::model::Id::now_v7()
        ));
        let config = explicit_config(&root);
        let mut host = if https {
            BootstrapConfig::create_explicit(config.clone())
        } else {
            BootstrapConfig::create_explicit_http(config.clone())
        }
        .expect("explicit neutral host");
        assert!(host.config().idp.oauth2.issuer.is_empty());
        assert_eq!(host.config().idp.api_public_uri, config.idp.api_public_uri);
        host.begin_setup().expect("retain setup production");
        assert!(host.config().idp.oauth2.issuer.is_empty());
        drop(host);
        let mut host =
            BootstrapConfig::open_explicit(config.clone()).expect("reopen attempted neutral host");
        host.set_runtime_config(host.config());
        host.select_create().expect("select configured issuer");
        assert_eq!(host.config().idp.oauth2.issuer, config.idp.oauth2.issuer);
        assert_ne!(host.config().idp.oauth2.issuer, config.idp.api_public_uri);
        host.select_create().expect("idempotent selection");
        drop(host);
        let host = BootstrapConfig::open_explicit(config.clone()).expect("reopen explicit Create");
        assert_eq!(host.config().idp.oauth2.issuer, config.idp.oauth2.issuer);
        assert!(matches!(host.purpose(), BootstrapPurpose::Create));
        drop(host);
        fs::remove_dir_all(root).expect("isolated cleanup");
    }
}

#[test]
fn empty_explicit_issuer_retains_public_api_fallback_and_accepts_equivalent_config() {
    for https in [true, false] {
        let root = std::env::temp_dir().join(format!(
            "unified-empty-explicit-issuer-{}",
            idp_model::model::Id::now_v7()
        ));
        let mut config = explicit_config(&root);
        config.idp.oauth2.issuer.clear();
        let mut resolved = config.clone();
        resolved.idp.oauth2.issuer = config.idp.api_public_uri.clone();
        let mut host = if https {
            BootstrapConfig::create_explicit(config.clone())
        } else {
            BootstrapConfig::create_explicit_http(config.clone())
        }
        .expect("empty issuer uses public API URI");
        assert!(host.config().idp.oauth2.issuer.is_empty());
        let host_root = host.directory().to_path_buf();
        assert_eq!(host.config().idp.api_public_uri, config.idp.api_public_uri);
        host.begin_setup().expect("retain setup production");
        drop(host);
        for equivalent in [&config, &resolved] {
            let host = BootstrapConfig::open_explicit(equivalent.clone())
                .expect("equivalent neutral configuration");
            assert!(host.config().idp.oauth2.issuer.is_empty());
            drop(host);
        }
        let mut host = BootstrapConfig::open_explicit(config.clone())
            .expect("reopen empty-issuer configuration");
        host.set_runtime_config(host.config());
        host.select_create()
            .expect("select retained resolved issuer");
        assert_eq!(host.config().idp.oauth2.issuer, config.idp.api_public_uri);
        drop(host);
        let retained = retained_config(&host_root);
        for equivalent in [&config, &resolved] {
            let host = BootstrapConfig::open_explicit(equivalent.clone())
                .expect("equivalent selected configuration");
            assert_eq!(host.config().idp.oauth2.issuer, config.idp.api_public_uri);
            drop(host);
            assert!(
                retained_config(&host_root) == retained,
                "persisted host configuration changed"
            );
        }
        fs::remove_dir_all(root).expect("isolated cleanup");
    }
}

#[test]
fn whitespace_explicit_issuer_is_rejected_without_writes() {
    for issuer in [
        " ",
        "\t\n",
        " https://identity.test/idp",
        "https://identity.test/idp ",
    ] {
        let root = std::env::temp_dir().join(format!(
            "unified-whitespace-issuer-{}",
            idp_model::model::Id::now_v7()
        ));
        let mut config = explicit_config(&root);
        config.idp.oauth2.issuer = issuer.into();
        assert!(BootstrapConfig::create_explicit(config.clone()).is_err());
        assert!(BootstrapConfig::create_explicit_http(config).is_err());
        assert!(
            !root.exists(),
            "invalid issuer must not create host or data roots"
        );
    }
}

#[test]
fn explicit_issuer_conflicts_are_refused_without_changing_identity_or_database() {
    let root = std::env::temp_dir().join(format!(
        "unified-issuer-conflict-{}",
        idp_model::model::Id::now_v7()
    ));
    let config = explicit_config(&root);
    let mut host = BootstrapConfig::create_explicit_http(config.clone()).expect("neutral host");
    let host_root = host.directory().to_path_buf();
    host.begin_setup().expect("retain setup production");
    drop(host);
    let mut conflicting = config.clone();
    conflicting.idp.oauth2.issuer = "https://different.test/idp".into();
    for selected in [false, true] {
        let mut host = BootstrapConfig::open_explicit(config.clone()).expect("exact identity");
        if selected {
            host.select_create().expect("select Create");
        }
        drop(host);
        fs::create_dir_all(&config.idp.data_dir).expect("isolated database root");
        let database = Path::new(&config.idp.data_dir).join("existing.db");
        fs::write(&database, b"existing installation data").expect("isolated database");
        let retained = retained_config(&host_root);
        let error = BootstrapConfig::open_explicit(conflicting.clone())
            .err()
            .expect("issuer mismatch rejected");
        assert!(
            error
                .to_string()
                .contains("conflicts with retained host identity")
        );
        assert!(
            retained_config(&host_root) == retained,
            "persisted host configuration changed"
        );
        assert!(
            fs::read(&database).expect("database preserved") == b"existing installation data",
            "rejected configuration changed existing owner database bytes"
        );
        assert!(BootstrapConfig::create_explicit_http(conflicting.clone()).is_err());
    }
    fs::remove_dir_all(root).expect("isolated cleanup");
}

#[test]
fn explicit_join_preserves_authenticated_peer_issuer_not_configured_create_issuer() {
    let root = std::env::temp_dir().join(format!(
        "unified-explicit-join-{}",
        idp_model::model::Id::now_v7()
    ));
    let config = explicit_config(&root);
    let peer = iroh::SecretKey::generate().public().to_string();
    let issuer = "https://authenticated-peer.test/installation";
    let mut host = BootstrapConfig::create_explicit_http(config.clone()).expect("neutral host");
    assert!(host.select_join(&peer, issuer).is_err());
    host.begin_setup().expect("retain setup production");
    assert!(
        host.select_join(&peer, "http://untrusted.test/idp")
            .is_err()
    );
    host.select_join(&peer, issuer).expect("authenticated Join");
    assert_eq!(host.config().idp.oauth2.issuer, issuer);
    host.select_join(&peer, issuer)
        .expect("idempotent exact Join");
    assert!(
        host.select_join(&peer, "https://different.test/idp")
            .is_err()
    );
    assert!(host.select_create().is_err());
    drop(host);
    let host = BootstrapConfig::open_explicit(config.clone()).expect("reopen authenticated Join");
    assert_eq!(host.config().idp.oauth2.issuer, issuer);
    assert_eq!(host.trusted_peer(), Some(peer.as_str()));
    drop(host);
    fs::remove_dir_all(root).expect("isolated cleanup");
}

#[test]
fn legacy_fabricated_issuer_is_rejected_without_rewriting_identity_or_database() {
    let root = std::env::temp_dir().join(format!(
        "unified-legacy-issuer-{}",
        idp_model::model::Id::now_v7()
    ));
    let host = BootstrapConfig::create(&root, BootstrapPurpose::Create, None)
        .expect("isolated automatic host");
    let database_root = host.config().idp.data_dir;
    drop(host);
    fs::create_dir(&database_root).expect("isolated database root");
    let database = Path::new(&database_root).join("existing.db");
    fs::write(&database, b"existing installation data").expect("isolated database");
    let store = host_store::HostStore::open(&root).expect("SQL fault seam");
    let mut retained: serde_json::Value =
        store.get("config").expect("read config").expect("config");
    retained["issuer"] = serde_json::json!(format!(
        "https://{}.installation.invalid/idp",
        retained["key_namespace"]
            .as_str()
            .expect("retained namespace")
    ));
    store
        .put("config", &retained)
        .expect("inject unsupported retained issuer");
    let retained = retained_config(&root);
    let error = BootstrapConfig::open(&root)
        .err()
        .expect("legacy issuer rejected");
    assert!(error.to_string().contains("installation.invalid"));
    assert!(error.to_string().contains("backup"));
    assert!(
        retained_config(&root) == retained,
        "rejected issuer changed persisted host configuration"
    );
    assert!(
        fs::read(&database).expect("database preserved") == b"existing installation data",
        "rejected issuer changed existing owner database bytes"
    );
    assert!(BootstrapConfig::create(&root, BootstrapPurpose::Create, None).is_err());
    fs::remove_dir_all(root).expect("isolated cleanup");
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
    let host = BootstrapConfig::open(&root).expect("retained authenticated Join identity");
    assert_eq!(
        host.config().idp.oauth2.issuer,
        "https://trusted-installation.test/idp"
    );
    assert_eq!(host.trusted_peer(), Some(peer.as_str()));
    drop(host);
    std::fs::remove_file(root.join("host.redb")).expect("simulate lost host identity");
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
