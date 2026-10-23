use std::{io, net::SocketAddr, path::Path};

use api::IndependentSetupHost;

use crate::AppConfig;

pub(crate) fn load_config(path: &Path) -> io::Result<AppConfig> {
    let raw = config::Config::builder()
        .add_source(config::File::from(path).required(false))
        .add_source(config::Environment::with_prefix("SERVER"))
        .build()
        .map_err(|_| io::Error::other("invalid Management configuration"))?;
    let explicit_data = raw.get_string("data_dir").is_ok();
    let explicit_port = raw.get_int("server.port").is_ok() || std::env::var_os("PORT").is_some();
    let mut config: AppConfig = raw
        .try_deserialize()
        .map_err(|_| io::Error::other("invalid Management configuration"))?;
    if !explicit_data
        && !Path::new("data").is_symlink()
        && !Path::new("data").try_exists()?
        && !Path::new("data.setup-host").is_symlink()
        && !Path::new("data.setup-host").try_exists()?
    {
        config.data_dir = "device-data".into();
    }
    let domain = Path::new(&config.data_dir);
    if !explicit_port
        && (!domain.try_exists()? || domain.with_extension("setup-host").try_exists()?)
    {
        config.server.port = 3001;
    }
    Ok(config)
}

pub fn prepare_setup(
    config: &AppConfig,
    address: SocketAddr,
) -> io::Result<Option<IndependentSetupHost>> {
    IndependentSetupHost::prepare(
        "device",
        Path::new(&config.data_dir),
        address,
        config
            .server
            .setup_ui_origin
            .as_deref()
            .unwrap_or("https://lidp.localhost:1355"),
        config.server.prefix(),
        config.idp_permission_evaluator_client_id.is_some()
            || [
                "SERVER_IDP_OAUTH_CLIENT_ID",
                "SERVER_IDP_OAUTH_CLIENT_SECRET",
            ]
            .iter()
            .any(|name| std::env::var_os(name).is_some()),
    )
}

#[cfg(all(test, target_os = "linux"))]
#[path = "startup_https_tests.rs"]
mod https_tests;

#[cfg(test)]
mod tests {
    #[test]
    fn existing_owner_keeps_its_listener_and_does_not_gain_fresh_admission() {
        let root = std::env::temp_dir().join(format!(
            "device-existing-{}",
            idp_model::model::Id::now_v7()
        ));
        std::fs::create_dir(&root).expect("existing domain root");
        let path = root.join("config.yaml");
        std::fs::write(
            &path,
            format!(
                "data_dir: {}\nexpected_issuer: https://issuer.invalid\n",
                root.display()
            ),
        )
        .expect("configured existing owner");
        let config = super::load_config(&path).expect("existing configuration");
        assert_eq!(config.server.port, 3000);
        assert!(
            super::prepare_setup(&config, "127.0.0.1:3000".parse().expect("address"))
                .expect("existing selection")
                .is_none()
        );
        assert!(!root.with_extension("setup-host").exists());
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn retained_setup_keeps_the_default_listener_after_domain_creation() {
        let root = std::env::temp_dir().join(format!(
            "device-retained-port-{}",
            idp_model::model::Id::now_v7()
        ));
        std::fs::create_dir(&root).expect("fixture root");
        let domain = root.join("device");
        let path = root.join("config.yaml");
        std::fs::write(&path, format!("data_dir: {}\n", domain.display()))
            .expect("routing configuration without a port");
        let config = super::load_config(&path).expect("fresh ordinary configuration");
        assert_eq!(config.server.port, 3001);
        let host = super::prepare_setup(
            &config,
            format!("127.0.0.1:{}", config.server.port)
                .parse()
                .expect("address"),
        )
        .expect("prepare actual setup host")
        .expect("fresh host");
        drop(host);
        assert_eq!(
            super::load_config(&path)
                .expect("pre-domain interrupted retry")
                .server
                .port,
            3001
        );
        std::fs::create_dir(&domain).expect("worker has attempted its owner store");
        assert_eq!(
            super::load_config(&path)
                .expect("post-domain interrupted retry or completed reopen")
                .server
                .port,
            3001
        );
        std::fs::write(
            &path,
            format!("data_dir: {}\nserver:\n  port: 4011\n", domain.display()),
        )
        .expect("explicit listener choice");
        assert_eq!(
            super::load_config(&path)
                .expect("explicit port remains authoritative")
                .server
                .port,
            4011
        );
        std::fs::remove_dir_all(root).expect("fixture cleanup");
    }

    #[test]
    fn optional_cli_config_uses_an_independent_root_and_port() {
        let path = std::env::temp_dir().join(format!(
            "absent-device-{}.yaml",
            idp_model::model::Id::now_v7()
        ));
        let config = super::load_config(&path).expect("optional configuration");
        assert_eq!(config.data_dir, "device-data");
        assert_eq!(config.server.port, 3001);
    }

    #[test]
    fn fresh_owner_routing_settings_are_not_relationship_credentials() {
        let root =
            std::env::temp_dir().join(format!("device-routing-{}", idp_model::model::Id::now_v7()));
        let config = crate::AppConfig {
            data_dir: root.join("device").to_string_lossy().into_owned(),
            idp_api_base: "https://127.0.0.1:3000".into(),
            storage_api_base: "https://127.0.0.1:3002".into(),
            expected_issuer: "https://127.0.0.1:3000/idp".into(),
            storage_audience: "storage:databases".into(),
            ..crate::AppConfig::default()
        };
        let host = super::prepare_setup(&config, "127.0.0.1:3001".parse().expect("address"))
            .expect("nonsecret routing is allowed")
            .expect("fresh setup");
        drop(host);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn fresh_owner_startup_requires_no_relationship_credentials() {
        let root = std::env::temp_dir().join(format!(
            "device-independent-{}",
            idp_model::model::Id::now_v7()
        ));
        let config = crate::AppConfig {
            data_dir: root.join("device").to_string_lossy().into_owned(),
            ..crate::AppConfig::default()
        };
        let host = super::prepare_setup(&config, "127.0.0.1:3001".parse().expect("address"))
            .expect("fresh startup")
            .expect("setup host");
        assert!(!std::path::Path::new(&config.data_dir).exists());
        assert!(host.admission_path().is_file());
        drop(host);
        std::fs::remove_dir_all(root).expect("isolated cleanup");
    }
}
