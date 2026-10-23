use std::{io, net::SocketAddr, path::Path};

use api::IndependentSetupHost;

use crate::AppConfig;

pub(crate) fn load_config(path: &Path) -> io::Result<AppConfig> {
    let raw = config::Config::builder()
        .add_source(config::File::from(path).required(false))
        .add_source(config::Environment::with_prefix("LIDP"))
        .build()
        .map_err(|_| io::Error::other("invalid IdP configuration"))?;
    let explicit_data = raw.get_string("data_dir").is_ok();
    let mut config: AppConfig = raw
        .try_deserialize()
        .map_err(|_| io::Error::other("invalid IdP configuration"))?;
    if !explicit_data
        && !Path::new("data").is_symlink()
        && !Path::new("data").try_exists()?
        && !Path::new("data.setup-host").is_symlink()
        && !Path::new("data.setup-host").try_exists()?
    {
        config.data_dir = "idp-data".into();
    }
    Ok(config)
}

pub fn prepare_setup(
    config: &AppConfig,
    address: SocketAddr,
) -> io::Result<Option<IndependentSetupHost>> {
    if let Some(remote) = &config.identity_database {
        remote.validate()?;
        if Path::new(&config.data_dir).join("idp.redb").try_exists()? {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "remote identity consumer cannot also own idp.redb",
            ));
        }
        return Ok(None);
    }
    IndependentSetupHost::prepare(
        "idp",
        Path::new(&config.data_dir),
        address,
        config
            .server
            .setup_ui_origin
            .as_deref()
            .unwrap_or(&config.ui_public_uri),
        config.server.prefix(),
        config.oauth2.local_signer.is_some() || config.management_data_dir.is_some(),
    )
}

#[cfg(all(test, target_os = "linux"))]
#[path = "startup_https_tests.rs"]
mod https_tests;

#[cfg(all(test, unix))]
mod tests {
    #[test]
    fn optional_cli_config_uses_independent_defaults_and_rejects_malformed_config() {
        let root =
            std::env::temp_dir().join(format!("idp-config-{}", idp_model::model::Id::now_v7()));
        std::fs::create_dir(&root).expect("fixture");
        let path = root.join("config.yaml");
        let config = super::load_config(&path).expect("optional absent config");
        assert_eq!(config.data_dir, "idp-data");
        std::fs::write(&path, "server: [not-a-server]").expect("malformed retained config");
        assert!(super::load_config(&path).is_err());
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn fresh_owner_routing_settings_are_not_relationship_credentials() {
        let root =
            std::env::temp_dir().join(format!("idp-routing-{}", idp_model::model::Id::now_v7()));
        let config = crate::AppConfig {
            data_dir: root.join("idp").to_string_lossy().into_owned(),
            control_plane_uri: Some("https://127.0.0.1:3001".into()),
            ..crate::AppConfig::default()
        };
        let host = super::prepare_setup(&config, "127.0.0.1:3000".parse().expect("address"))
            .expect("nonsecret routing is allowed")
            .expect("setup");
        drop(host);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn fresh_owner_startup_does_not_create_domain_state_or_select_issuer() {
        let root = std::env::temp_dir().join(format!(
            "idp-independent-{}",
            idp_model::model::Id::now_v7()
        ));
        let config = crate::AppConfig {
            data_dir: root.join("idp").to_string_lossy().into_owned(),
            ..crate::AppConfig::default()
        };
        let host = super::prepare_setup(&config, "127.0.0.1:3000".parse().expect("address"))
            .expect("fresh startup")
            .expect("setup rather than normal runtime");
        assert!(!std::path::Path::new(&config.data_dir).exists());
        assert!(host.admission_path().is_file());
        drop(host);
        std::fs::remove_dir_all(root).expect("isolated cleanup");
    }
}
