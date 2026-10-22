#[cfg(all(test, not(unix)))]
mod unsupported_tests {
    #[test]
    fn unsupported_platform_rejects_all_bootstrap_constructors_before_creating_state() {
        let root = std::env::temp_dir().join(format!(
            "unified-unsupported-bootstrap-{}",
            idp_model::model::Id::now_v7()
        ));
        let config = crate::config::test_config(&root);
        for error in [
            crate::LocalTls::create(&config)
                .err()
                .expect("unsupported TLS"),
            crate::CredentialBootstrap::create(config.clone())
                .err()
                .expect("unsupported credentials"),
            crate::LocalJoin::create(config.clone(), "peer", "Node")
                .err()
                .expect("unsupported join"),
            crate::LocalProvisioner::create(config, "operator", "Node", &root.join("password"))
                .err()
                .expect("unsupported create"),
        ] {
            assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);
        }
        assert!(
            !root.exists(),
            "no retained directory or material on unsupported platforms"
        );
    }
}
