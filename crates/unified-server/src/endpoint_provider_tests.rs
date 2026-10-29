use std::{
    collections::HashMap,
    fs,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use keyring_core::{Entry, Error, api::CredentialStoreApi};
use serde_json::{Value, json};

use super::{Fixture, LocalProvisioner};
use crate::{BootstrapConfig, BootstrapPurpose, CredentialBootstrap, LocalJoin, UnifiedEndpoint};

#[derive(Clone, Copy)]
enum Fault {
    Unavailable,
    Existing,
    Missing,
    Corrupt,
    Mismatch,
    Operation,
}

struct Provider {
    inner: Arc<keyring_core::mock::Store>,
    journal: Metadata,
    calls: AtomicUsize,
    fault: Mutex<Option<(usize, Fault)>>,
}

enum Metadata {
    Create(crate::host_store::HostStore),
    Join(crate::host_store::HostStore),
}

impl Metadata {
    fn read(&self) -> Value {
        match self {
            Self::Create(store) => store
                .get("create")
                .expect("SQL read")
                .expect("durable Create"),
            Self::Join(store) => store.get("join").expect("SQL read").expect("durable Join"),
        }
    }
}

impl Provider {
    fn new(config: &crate::UnifiedConfig, joining: bool) -> Arc<Self> {
        let journal = if joining {
            Metadata::Join(crate::host_store::HostStore::for_config(config).expect("host SQL"))
        } else {
            Metadata::Create(crate::host_store::HostStore::for_config(config).expect("host SQL"))
        };
        let provider = Arc::new(Self {
            inner: keyring_core::mock::Store::new().expect("isolated endpoint provider"),
            journal,
            calls: AtomicUsize::new(0),
            fault: Mutex::new(None),
        });
        keyring_core::set_default_store(provider.clone());
        provider
    }

    fn fail(&self, call: usize, fault: Fault) {
        self.calls.store(0, Ordering::SeqCst);
        *self.fault.lock().expect("fault lock") = Some((call, fault));
    }

    fn entry(&self) -> Entry {
        let journal = self.journal.read();
        self.inner
            .build(
                journal["endpoint_secret"]["service"]
                    .as_str()
                    .expect("exact service"),
                journal["endpoint_secret"]["entry"]
                    .as_str()
                    .expect("exact entry"),
                None,
            )
            .expect("exact provider entry")
    }
}

impl CredentialStoreApi for Provider {
    fn vendor(&self) -> String {
        self.inner.vendor()
    }
    fn id(&self) -> String {
        self.inner.id()
    }
    fn build(
        &self,
        service: &str,
        user: &str,
        modifiers: Option<&HashMap<&str, &str>>,
    ) -> keyring_core::Result<Entry> {
        let entry = self.inner.build(service, user, None)?;
        if service != "of-unified-endpoint" {
            return Ok(entry);
        }
        let journal = self.journal.read();
        assert_eq!(journal["endpoint_attempted"], true);
        assert_eq!(journal["endpoint_secret"]["service"], service);
        assert_eq!(journal["endpoint_secret"]["entry"], user);
        assert_eq!(
            journal["endpoint_secret"]["target"],
            modifiers.expect("retained provider modifiers")["target"]
        );
        journal["endpoint"]
            .as_str()
            .expect("expected public ID durable before creation")
            .parse::<iroh::EndpointId>()
            .expect("canonical public ID");
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        let mut fault = self.fault.lock().expect("fault lock");
        if fault.as_ref().is_some_and(|(at, _)| *at == call) {
            match fault.take().expect("matched fault").1 {
                Fault::Unavailable => {
                    return Err(Error::Invalid("provider".into(), "unavailable".into()));
                }
                Fault::Existing => entry.set_secret(&[9; 32])?,
                Fault::Missing => entry.delete_credential()?,
                Fault::Corrupt => entry.set_secret(&[1, 2])?,
                Fault::Mismatch => entry.set_secret(&iroh::SecretKey::generate().to_bytes())?,
                Fault::Operation => entry
                    .as_any()
                    .downcast_ref::<keyring_core::mock::Cred>()
                    .expect("mock operation")
                    .set_error(Error::Invalid(
                        "provider".into(),
                        "injected operation failure".into(),
                    )),
            }
        }
        Ok(entry)
    }
    fn search(&self, spec: &HashMap<&str, &str>) -> keyring_core::Result<Vec<Entry>> {
        self.inner.search(spec)
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[tokio::test]
async fn create_attempt_is_durable_before_provider_work_and_uncertainty_never_retries_creation() {
    for (call, fault) in [
        (1, Fault::Unavailable),
        (1, Fault::Existing),
        (2, Fault::Operation),
        (3, Fault::Missing),
        (3, Fault::Corrupt),
        (3, Fault::Mismatch),
    ] {
        let fixture = Fixture::new();
        let local = LocalProvisioner::create_with_password(
            fixture.config.clone(),
            "operator",
            "Node",
            "fixture-initial-password",
        )
        .expect("fresh intent");
        let provider = Provider::new(&fixture.config, false);
        provider.fail(call, fault);
        assert!(local.prepare_endpoint().await.is_err());
        let retained = fixture.journal();
        assert_eq!(retained["endpoint_attempted"], true);
        assert!(!retained["endpoint_secret"].is_null());
        assert!(
            local.prepare_endpoint().await.is_err(),
            "uncertain creation cannot mint or overwrite material"
        );
        assert_eq!(fixture.journal(), retained);
        if matches!(fault, Fault::Existing) {
            assert!(
                provider
                    .entry()
                    .get_secret()
                    .expect("preexisting secret untouched")
                    == vec![9; 32],
                "preexisting secret changed"
            );
        }
        assert!(
            !fixture
                .config
                .endpoint_data_dir
                .join("endpoint.key")
                .exists()
        );
    }
}

#[tokio::test]
async fn failed_creation_readback_can_only_reopen_the_exact_written_identity() {
    let fixture = Fixture::new();
    let local = LocalProvisioner::create_with_password(
        fixture.config.clone(),
        "operator",
        "Node",
        "fixture-initial-password",
    )
    .expect("fresh intent");
    let provider = Provider::new(&fixture.config, false);
    provider.fail(3, Fault::Unavailable);
    assert!(local.prepare_endpoint().await.is_err());
    let retained = fixture.journal();
    let bytes = provider
        .entry()
        .get_secret()
        .expect("uncertain write actually reached provider");
    let endpoint = local
        .prepare_endpoint()
        .await
        .expect("existing-only exact continuation");
    assert_eq!(endpoint.endpoint_id().to_string(), retained["endpoint"]);
    assert!(
        provider
            .entry()
            .get_secret()
            .expect("unchanged private material")
            == bytes,
        "private material changed"
    );
    assert_eq!(fixture.journal(), retained);
    endpoint.close().await;
}

#[tokio::test]
async fn ordinary_reopen_refuses_missing_corrupt_mismatched_and_unavailable_material_unchanged() {
    for fault in [
        Fault::Missing,
        Fault::Corrupt,
        Fault::Mismatch,
        Fault::Unavailable,
    ] {
        let fixture = Fixture::new();
        let local = LocalProvisioner::create_with_password(
            fixture.config.clone(),
            "operator",
            "Node",
            "fixture-initial-password",
        )
        .expect("fresh intent");
        let provider = Provider::new(&fixture.config, false);
        let endpoint = local.prepare_endpoint().await.expect("fresh endpoint");
        let expected = endpoint.endpoint_id();
        endpoint.close().await;
        let retained = fixture.journal();
        provider.fail(1, fault);
        assert!(
            UnifiedEndpoint::open(&fixture.config.endpoint_data_dir, &fixture.config.idp)
                .await
                .is_err()
        );
        assert_eq!(fixture.journal(), retained);
        if matches!(fault, Fault::Unavailable) {
            let endpoint = local
                .prepare_endpoint()
                .await
                .expect("provider recovery reuses existing identity");
            assert_eq!(endpoint.endpoint_id(), expected);
            endpoint.close().await;
        } else {
            assert!(local.prepare_endpoint().await.is_err());
        }
        assert!(
            !fixture
                .config
                .endpoint_data_dir
                .join("endpoint.key")
                .exists()
        );
    }
}

#[tokio::test]
async fn join_endpoint_is_stable_and_missing_attempted_material_cannot_be_replaced() {
    let fixture = Fixture::new();
    let peer = iroh::SecretKey::generate().public().to_string();
    let mut local =
        LocalJoin::create(fixture.config.clone(), &peer, "Joining Node").expect("fresh Join");
    let store = crate::host_store::HostStore::for_config(&fixture.config).expect("host SQL");
    let provider = Provider::new(&fixture.config, true);
    let endpoint = local
        .prepare_endpoint()
        .await
        .expect("provider-owned Join endpoint");
    let expected = endpoint.endpoint_id();
    endpoint.close().await;
    drop(local);
    let mut local = LocalJoin::open(fixture.config.clone()).expect("retained Join");
    let endpoint = local.prepare_endpoint().await.expect("exact reopen");
    assert_eq!(endpoint.endpoint_id(), expected);
    endpoint.close().await;
    let retained: Value = store.get("join").expect("SQL read").expect("Join");
    provider
        .entry()
        .delete_credential()
        .expect("simulate provider loss");
    assert!(local.prepare_endpoint().await.is_err());
    assert_eq!(
        store.get::<Value>("join").expect("SQL read").expect("Join"),
        retained
    );
    assert!(
        !fixture
            .config
            .endpoint_data_dir
            .join("endpoint.key")
            .exists()
    );
}

#[tokio::test]
async fn reset_retains_exact_endpoint_reference_through_deletion_failure_and_continuation() {
    for joining in [false, true] {
        let fixture = Fixture::new();
        let peer = iroh::SecretKey::generate().public().to_string();
        let trusted = joining.then_some((peer.as_str(), "https://source.test/idp"));
        let host = BootstrapConfig::create_http(
            &fixture.root.join("node"),
            if joining {
                BootstrapPurpose::Join
            } else {
                BootstrapPurpose::Create
            },
            trusted,
            "127.0.0.1:3000".parse().expect("listener"),
            "http://127.0.0.1:3000".into(),
        )
        .expect("private host");
        let config = host.config();
        let credentials =
            CredentialBootstrap::create(config.clone()).expect("host credential references");
        drop(credentials);
        if joining {
            let mut local =
                LocalJoin::create(config.clone(), &peer, "Joining Node").expect("fresh Join");
            let provider = Provider::new(&config, joining);
            let endpoint = local
                .prepare_endpoint()
                .await
                .expect("Join endpoint before owner production");
            endpoint.close().await;
            drop(local);
            drop(provider);
        } else {
            let local = LocalProvisioner::create_with_password(
                config.clone(),
                "operator",
                "Node",
                "fixture-initial-password",
            )
            .expect("fresh intent");
            let provider = Provider::new(&config, joining);
            let endpoint = local
                .prepare_endpoint()
                .await
                .expect("endpoint before owner production");
            endpoint.close().await;
            drop(local);
            drop(provider);
        }
        let provider = keyring_core::get_default_store().expect("injected reset provider");
        let provider = provider
            .as_any()
            .downcast_ref::<Provider>()
            .expect("same captured provider");
        let unrelated = provider
            .inner
            .build("unrelated-service", "unrelated-entry", None)
            .expect("foreign secret");
        unrelated
            .set_secret(b"do not delete")
            .expect("foreign material");
        let references = host
            .local_reset_references()
            .expect("capture exact references");
        assert!(!references["keys"]["endpoint_secret"].is_null());
        provider.fail(2, Fault::Operation);
        assert!(host.delete_local_setup_secrets(&references).await.is_err());
        assert_eq!(
            host.local_reset_references().expect("retained references"),
            references
        );
        assert!(provider.entry().get_secret().is_ok());
        host.delete_local_setup_secrets(&references)
            .await
            .expect("explicit exact reset continuation");
        assert!(matches!(provider.entry().get_secret(), Err(Error::NoEntry)));
        assert_eq!(
            host.local_reset_references()
                .expect("evidence survives until caller removes local data"),
            references
        );
        host.delete_local_setup_secrets(&references)
            .await
            .expect("idempotent deletion after lost acknowledgement");
        assert_eq!(
            unrelated.get_secret().expect("unrelated entry survives"),
            b"do not delete"
        );
        assert!(config.local_provisioning_directory().exists());
    }
}

#[tokio::test]
async fn unsupported_raw_endpoint_layout_is_refused_without_provider_work_or_replacement() {
    let fixture = Fixture::new();
    fs::create_dir(&fixture.config.endpoint_data_dir).expect("legacy directory");
    let raw = fixture.config.endpoint_data_dir.join("endpoint.key");
    fs::write(&raw, b"retained legacy private material").expect("legacy fixture");
    let before = fixture.secrets();
    assert!(
        LocalProvisioner::create_with_password(
            fixture.config.clone(),
            "operator",
            "Node",
            "fixture-initial-password"
        )
        .is_err()
    );
    assert!(
        UnifiedEndpoint::open(&fixture.config.endpoint_data_dir, &fixture.config.idp)
            .await
            .is_err()
    );
    assert_eq!(
        fs::read(raw).expect("legacy state retained"),
        b"retained legacy private material"
    );
    assert!(fixture.secrets() == before, "provider inventory changed");
    assert!(!fixture.config.local_provisioning_directory().exists());
}

#[tokio::test]
async fn interrupted_attempt_before_provider_creation_refuses_and_preserves_reference() {
    let fixture = Fixture::new();
    let local = LocalProvisioner::create_with_password(
        fixture.config.clone(),
        "operator",
        "Node",
        "fixture-initial-password",
    )
    .expect("fresh intent");
    let provider = Provider::new(&fixture.config, false);
    provider.fail(1, Fault::Unavailable);
    assert!(local.prepare_endpoint().await.is_err());
    drop(local);
    fs::remove_dir(&fixture.config.endpoint_data_dir)
        .expect("simulate interruption before endpoint directory publication");
    let local = LocalProvisioner::open(fixture.config.clone()).expect("reopen uncertain attempt");
    let retained = fixture.journal();
    assert!(local.prepare_endpoint().await.is_err());
    assert_eq!(fixture.journal(), retained);
    assert!(matches!(provider.entry().get_secret(), Err(Error::NoEntry)));
    assert_eq!(retained["inventory_attempted"], json!(false));
    assert!(!fixture.config.endpoint_data_dir.exists());
}

use iroh::{
    Endpoint,
    endpoint::presets,
    protocol::{AcceptError, ProtocolHandler},
};
use iroh_chain::DATA_ALPN;
#[derive(Debug)]
struct TestProtocol(tokio::sync::mpsc::UnboundedSender<()>);

impl ProtocolHandler for TestProtocol {
    async fn accept(&self, _connection: iroh::endpoint::Connection) -> Result<(), AcceptError> {
        self.0.send(()).expect("test receiver remains open");
        Ok(())
    }
}

#[tokio::test]
async fn iroh_rejects_connections_after_peer_revocation() {
    let fixture = Fixture::new();
    let local = LocalProvisioner::create_with_password(
        fixture.config.clone(),
        "operator",
        "Node",
        "fixture-initial-password",
    )
    .expect("fresh host intent");
    let unified = local
        .prepare_endpoint()
        .await
        .expect("open unified endpoint");
    let peer = Endpoint::builder(presets::Minimal)
        .bind()
        .await
        .expect("bind test peer endpoint");
    assert_eq!(
        unified
            .identity()
            .expect("endpoint Device identity")
            .endpoint_id(),
        unified.endpoint_id()
    );
    let peer_id = peer.id();
    unified.refresh_admission([peer_id]);
    let (accepted_tx, mut accepted_rx) = tokio::sync::mpsc::unbounded_channel();
    let _router = unified.server().router(TestProtocol(accepted_tx));
    let address = unified.server().endpoint().addr();

    let connection = peer
        .connect(address.clone(), DATA_ALPN)
        .await
        .expect("approved peer connects");
    tokio::time::timeout(std::time::Duration::from_secs(5), accepted_rx.recv())
        .await
        .expect("approved peer reaches protocol handler")
        .expect("protocol acceptance event arrives");
    connection.close(0u32.into(), b"done");
    connection.closed().await;
    unified.refresh_admission([]);
    let _connection = peer
        .connect(address, DATA_ALPN)
        .await
        .expect("Iroh handshake can complete before remote admission rejection");
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), accepted_rx.recv())
            .await
            .is_err(),
        "revoked peer must not reach the protocol handler"
    );

    peer.close().await;
    unified.close().await;
}

#[tokio::test]
async fn operation_without_endpoint_reference_field_is_refused_intact_without_provider_work() {
    let fixture = Fixture::new();
    let local = LocalProvisioner::create_with_password(
        fixture.config.clone(),
        "operator",
        "Node",
        "fixture-initial-password",
    )
    .expect("fresh metadata");
    drop(local);
    let mut old = fixture.journal();
    old.as_object_mut()
        .expect("object")
        .remove("endpoint_secret");
    fixture.write_journal(&old);
    let before = fixture.secrets();
    assert!(LocalProvisioner::open(fixture.config.clone()).is_err());
    assert_eq!(fixture.journal(), old);
    assert!(fixture.secrets() == before, "provider inventory changed");
    assert!(!fixture.config.endpoint_data_dir.exists());
}
