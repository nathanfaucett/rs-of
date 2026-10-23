use std::path::Path;

use idp_server::{
    AppConfig, BootstrapSettings,
    bootstrap_settings::{ManagementBootstrapSettings, StorageBootstrapSettings},
    local_provisioning::{intent, normalized_config},
};

#[test]
fn minimal_owner_settings_preserve_the_durable_intent_when_normalized() {
    let config = BootstrapSettings {
        listen_addr: "127.0.0.1:3000".parse().expect("loopback address"),
        endpoint_data_dir: "bootstrap-settings/endpoint".into(),
        idp: AppConfig {
            data_dir: "bootstrap-settings/idp".into(),
            api_public_uri: "https://identity.test/idp".into(),
            service_audience: Some("identity-service".into()),
            key_namespace: "local-bootstrap".into(),
            oauth2: idp_service::oauth2::OAuth2Config {
                issuer: "https://installation.test/idp".into(),
                ..Default::default()
            },
            ..Default::default()
        },
        management: ManagementBootstrapSettings {
            data_dir: "bootstrap-settings/management".into(),
            api_public_uri: "https://control.test/management".into(),
            storage_audience: "storage-service".into(),
        },
        storage: StorageBootstrapSettings {
            data_dir: "bootstrap-settings/storage".into(),
            api_public_uri: "https://resources.test/storage".into(),
        },
    };
    let mut retained = serde_json::to_value(intent(&config).expect("valid owner settings"))
        .expect("durable intent");
    assert_eq!(retained["issuer"], "https://installation.test/idp");
    assert_eq!(retained["key_namespace"], "local-bootstrap");
    assert_eq!(
        retained["audiences"],
        serde_json::json!(["identity-service", "storage-service"])
    );
    assert_eq!(
        retained["applications"],
        serde_json::json!([
            "https://identity.test/idp",
            "https://control.test/management",
            "https://resources.test/storage"
        ])
    );
    let directory = std::env::current_dir().expect("current directory");
    retained["roots"] = serde_json::json!([
        directory.join("bootstrap-settings/idp"),
        directory.join("bootstrap-settings/management"),
        directory.join("bootstrap-settings/storage"),
        directory.join("bootstrap-settings/endpoint"),
    ]);
    let normalized = normalized_config(config).expect("normalize minimal settings");
    assert!(normalized.endpoint_data_dir.is_absolute());
    assert!(Path::new(&normalized.management.data_dir).is_absolute());
    assert!(Path::new(&normalized.storage.data_dir).is_absolute());
    assert_eq!(
        serde_json::to_value(intent(&normalized).expect("normalized intent")).expect("intent JSON"),
        retained,
    );
}
