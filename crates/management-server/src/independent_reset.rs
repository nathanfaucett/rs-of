use std::{io, path::Path};

use crate::AppConfig;

pub async fn reset_independent(config: &AppConfig, _remove_os_trust: bool) -> io::Result<()> {
    config.validate_identity_connection()?;
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
    async fn socket_reset_validates_selector_without_connecting_or_removing_socket() {
        let root = std::env::temp_dir().join(format!(
            "management-socket-reset-{}",
            idp_model::model::Id::now_v7()
        ));
        std::fs::create_dir(&root).expect("test root");
        let socket = root.join("identity.sock");
        let listener = std::os::unix::net::UnixListener::bind(&socket).expect("owner socket");
        let mut config = crate::AppConfig {
            data_dir: root.join("consumer").to_string_lossy().into_owned(),
            identity_socket: Some(socket.clone()),
            ..Default::default()
        };
        super::reset_independent(&config, false)
            .await
            .expect("stateless socket reset needs no connection");
        assert!(socket.exists());
        config.identity_socket_deadline_ms = 0;
        assert!(super::reset_independent(&config, false).await.is_err());
        config.identity_socket_deadline_ms = 1000;
        config.identity_database = Some(db::RemoteDatabaseConfig {
            endpoint: "http://127.0.0.1:50051".into(),
            ca_certificate: None,
            client_certificate: None,
            client_key: None,
            deadline_ms: 100,
        });
        assert!(super::reset_independent(&config, false).await.is_err());
        config.identity_database = None;
        std::fs::create_dir(&config.data_dir).expect("retained evidence");
        assert!(super::reset_independent(&config, true).await.is_err());
        assert!(socket.exists());
        drop(listener);
        std::fs::remove_dir_all(root).expect("test cleanup");
    }

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
