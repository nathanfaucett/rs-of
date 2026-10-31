use std::fs;

#[tokio::test]
async fn reset_uses_the_stopped_owner_reset_boundaries() {
    let data = std::env::temp_dir().join(format!("native-owner-reset-{}", uuid::Uuid::new_v4()));
    let host = crate::native_host::prepare(&data.join("node")).expect("owner prerequisites");
    let ca_path = host.tls().ca_path();
    let fingerprint = |path: &std::path::Path| {
        let der = fs::read(path.with_file_name("ca.der")).expect("CA certificate");
        rustls::crypto::ring::cipher_suite::TLS13_AES_128_GCM_SHA256
            .tls13()
            .expect("SHA-256 cipher suite")
            .common
            .hash_provider
            .hash(&der)
            .as_ref()
            .to_vec()
    };
    let original_fingerprint = fingerprint(&ca_path);
    let tls_files = ["ca.key", "ca.pem", "ca.der", "server.key", "server.der"]
        .map(|name| fs::read(ca_path.with_file_name(name)).expect("original TLS material"));
    let ca = fs::read(&ca_path).expect("shared CA");
    let address = host.local_addr();
    drop(host);
    super::run(&data).await.expect("reset each fresh owner");
    assert_eq!(fs::read(&ca_path).expect("preserved shared CA"), ca);
    let fresh = crate::native_host::prepare(&data.join("node")).expect("fresh owners after reset");
    assert_eq!(fresh.local_addr(), address);
    assert_eq!(fingerprint(&fresh.tls().ca_path()), original_fingerprint);
    assert_eq!(
        ["ca.key", "ca.pem", "ca.der", "server.key", "server.der"]
            .map(|name| fs::read(ca_path.with_file_name(name)).expect("retained TLS material")),
        tls_files
    );
    drop(fresh);
    assert!(!super::pending(&data).expect("completed reset"));
    fs::remove_dir_all(data).expect("cleanup");
}

#[tokio::test]
async fn interrupted_owner_completion_resumes_before_and_after_desktop_acknowledgment() {
    for interrupted in ["idp", "device", "storage"] {
        for acknowledged in [false, true] {
            let data = std::env::temp_dir().join(format!(
                "native-reset-interruption-{}",
                uuid::Uuid::new_v4()
            ));
            drop(crate::native_host::prepare(&data.join("node")).expect("owner prerequisites"));
            let config = unified_server::UnifiedConfig::local_dynamic(&data.join("node"))
                .expect("retained config");
            let mut completed = Vec::<String>::new();
            super::publish(&data, "reset-state.json", &serde_json::json!(completed))
                .expect("begin desktop reset");
            for owner in ["idp", "device", "storage"] {
                let acknowledge = || {
                    if owner != interrupted || acknowledged {
                        completed.push(owner.into());
                        super::publish(&data, "reset-state.json", &serde_json::json!(completed))?;
                    }
                    if owner == interrupted {
                        return Err(std::io::Error::other("interrupted acknowledgment"));
                    }
                    Ok(())
                };
                let result = match owner {
                    "idp" => idp_server::reset_independent(&config.idp, true, acknowledge).await,
                    "device" => {
                        device_server::reset_independent(&config.device, true, acknowledge).await
                    }
                    "storage" => {
                        storage_server::reset_independent(&config.storage, true, acknowledge).await
                    }
                    _ => unreachable!(),
                };
                if owner == interrupted {
                    assert!(result.is_err());
                    break;
                }
                result.expect("preceding owner acknowledged");
            }
            assert!(super::pending(&data).expect("interrupted reset pending"));
            assert!(crate::native_host::prepare(&data.join("node")).is_err());
            super::run(&data)
                .await
                .expect("resume exact reset after interruption");
            assert!(!super::pending(&data).expect("reset complete"));
            drop(
                crate::native_host::prepare(&data.join("node"))
                    .expect("fresh setup after exact reset"),
            );
            fs::remove_dir_all(data).expect("cleanup");
        }
    }
}

#[test]
fn legacy_reference_recovery_refuses_without_reprovisioning() {
    let data = std::env::temp_dir().join(format!("native-legacy-reset-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(data.join("node")).expect("legacy root");
    fs::write(data.join("node/host.redb"), b"retained legacy evidence").expect("old state");
    assert!(super::recover(&data).is_err());
    assert_eq!(
        fs::read(data.join("node/host.redb")).expect("preserve old state"),
        b"retained legacy evidence"
    );
    fs::remove_dir_all(data).expect("cleanup");
}
