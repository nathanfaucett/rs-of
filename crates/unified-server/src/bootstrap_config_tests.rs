#[cfg(all(test, unix))]
mod tests {
    use crate::{BootstrapConfig, BootstrapPurpose};
    #[test]
    fn independent_installations_on_the_same_local_port_have_distinct_issuers() {
        let root = std::env::temp_dir().join(format!(
            "unified-distinct-installations-{}",
            idp_model::model::Id::now_v7()
        ));
        let a = BootstrapConfig::create(&root.join("a"), BootstrapPurpose::Create, None)
            .expect("first Installation");
        let b = BootstrapConfig::create(&root.join("b"), BootstrapPurpose::Create, None)
            .expect("second Installation");
        let first = a.config();
        drop(b);
        let path = root.join("b/host.json");
        let mut settings: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).expect("second host settings"))
                .expect("public settings");
        settings["listen"] = serde_json::json!(first.listen_addr.to_string());
        std::fs::write(
            &path,
            serde_json::to_vec(&settings).expect("same-port host settings"),
        )
        .expect("simulate same port on another machine");
        let b = BootstrapConfig::open(&root.join("b"))
            .expect("retained separate Installation identity");
        let second = b.config();
        assert_eq!(first.listen_addr, second.listen_addr);
        assert_eq!(first.idp.api_public_uri, second.idp.api_public_uri);
        assert_ne!(first.idp.oauth2.issuer, second.idp.oauth2.issuer);
        assert_ne!(first.idp.key_namespace, second.idp.key_namespace);
        drop((a, b));
        std::fs::remove_dir_all(root).expect("isolated cleanup");
    }
}
