use std::{io, path::Path};

use crate::AppConfig;

pub async fn reset_independent(config: &AppConfig, _remove_os_trust: bool) -> io::Result<()> {
    let remote = config.identity_database.as_ref().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "standalone Management requires identity_database",
        )
    })?;
    remote.validate()?;
    let domain = Path::new(&config.data_dir);
    for path in [
        domain.to_path_buf(),
        domain.with_extension("setup-host"),
        domain.with_extension("setup-reset"),
        domain.with_extension("setup-reset.json"),
    ] {
        if path.is_symlink() || path.try_exists()? {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "policy-only Management has no local owner state; retained data requires exact owner recovery",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn stateless_consumer_reset_never_removes_retained_owner_data() {
        let root = std::env::temp_dir().join(format!(
            "management-reset-{}",
            idp_model::model::Id::now_v7()
        ));
        let config = crate::AppConfig {
            data_dir: root.to_string_lossy().into_owned(),
            identity_database: Some(db::RemoteDatabaseConfig {
                endpoint: "http://127.0.0.1:50051".into(),
                ca_certificate: None,
                client_certificate: None,
                client_key: None,
                deadline_ms: 100,
            }),
            ..Default::default()
        };
        super::reset_independent(&config, false)
            .await
            .expect("stateless reset needs no SQL or OS trust operation");
        assert!(!root.exists());
        std::fs::create_dir(&root).expect("retained owner root");
        let evidence = root.join("identity.redb");
        std::fs::write(&evidence, "retained").expect("retained owner evidence");
        assert!(super::reset_independent(&config, true).await.is_err());
        assert_eq!(
            std::fs::read_to_string(&evidence).expect("owner untouched"),
            "retained"
        );
        std::fs::remove_dir_all(root).expect("fixture cleanup");
    }
}
