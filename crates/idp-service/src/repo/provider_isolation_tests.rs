use std::{collections::HashMap, sync::Arc, sync::mpsc, thread};

use key::{DerivationPath, DerivedKey, XPrv};

use super::{PrivateKeyKeyringRepo, PrivateKeyRepo, RawKeyringRepo};

struct Store(Arc<keyring_core::mock::Store>);

impl keyring_core::api::CredentialStoreApi for Store {
    fn vendor(&self) -> String {
        self.0.vendor()
    }
    fn id(&self) -> String {
        self.0.id()
    }
    fn build(
        &self,
        service: &str,
        user: &str,
        _: Option<&HashMap<&str, &str>>,
    ) -> keyring_core::Result<keyring_core::Entry> {
        self.0.build(service, user, None)
    }
    fn search(&self, spec: &HashMap<&str, &str>) -> keyring_core::Result<Vec<keyring_core::Entry>> {
        self.0.search(spec)
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[test]
fn repositories_pin_the_startup_default_and_injection_does_not_replace_it() {
    let first_store: Arc<keyring_core::CredentialStore> = Arc::new(Store(
        keyring_core::mock::Store::new().expect("startup mock store"),
    ));
    let second_store: Arc<keyring_core::CredentialStore> = Arc::new(Store(
        keyring_core::mock::Store::new().expect("injected mock store"),
    ));
    keyring_core::set_default_store(first_store.clone());
    let private = PrivateKeyKeyringRepo::new("startup-service").expect("mock startup provider");
    let raw = RawKeyringRepo::new("startup-service").expect("mock startup raw provider");
    let signer = DerivedKey::new(
        XPrv::new([3; 32]).expect("startup signer"),
        DerivationPath::default(),
    );
    private
        .store("startup-node", &signer)
        .expect("store startup signer");
    raw.store("user", "client", "secret", b"startup-secret")
        .expect("store startup raw secret");
    let injected = PrivateKeyKeyringRepo::new_with_store("startup-service", second_store.clone());
    assert!(Arc::ptr_eq(
        &keyring_core::get_default_store().expect("startup default retained"),
        &first_store
    ));
    assert!(
        injected
            .load("startup-node", signer.derivation_path())
            .expect("separate injected backend")
            .is_none()
    );
    keyring_core::set_default_store(second_store);
    assert_eq!(
        private
            .load("startup-node", signer.derivation_path())
            .expect("pinned startup backend")
            .expect("startup signer retained")
            .to_xprv_string(),
        signer.to_xprv_string()
    );
    assert_eq!(
        raw.load("user", "client", "secret")
            .expect("pinned startup raw backend"),
        Some(b"startup-secret".to_vec())
    );
    raw.delete("user", "client", "secret")
        .expect("delete pinned startup secret");
    assert!(
        raw.load("user", "client", "secret")
            .expect("deleted startup secret")
            .is_none()
    );
}

#[test]
fn overlapping_private_repositories_retain_signers_through_reopen() {
    let first_store: Arc<keyring_core::CredentialStore> = Arc::new(Store(
        keyring_core::mock::Store::new().expect("first mock store"),
    ));
    let second_store: Arc<keyring_core::CredentialStore> = Arc::new(Store(
        keyring_core::mock::Store::new().expect("second mock store"),
    ));
    let (stored_tx, stored_rx) = mpsc::channel();
    let (replaced_tx, replaced_rx) = mpsc::channel();
    let (checked_tx, checked_rx) = mpsc::channel();
    thread::scope(|scope| {
        let first = scope.spawn(move || {
            let repo = PrivateKeyKeyringRepo::new_with_store("same-service", first_store.clone());
            let signer = DerivedKey::new(
                XPrv::new([1; 32]).expect("first signer"),
                DerivationPath::default(),
            );
            repo.store("same-node", &signer)
                .expect("store first signer");
            stored_tx.send(()).expect("first signer stored");
            replaced_rx.recv().expect("second fixture is live");
            let retained = repo
                .load("same-node", signer.derivation_path())
                .expect("load first signer")
                .expect("first signer retained");
            assert_eq!(
                retained.to_xprv_string(),
                signer.to_xprv_string(),
                "another fixture cannot replace the first signer"
            );
            drop(repo);
            let reopened = PrivateKeyKeyringRepo::new_with_store("same-service", first_store);
            assert_eq!(
                reopened
                    .load("same-node", signer.derivation_path())
                    .expect("reopen first signer")
                    .expect("retained first signer")
                    .to_xprv_string(),
                signer.to_xprv_string()
            );
            reopened
                .delete("same-node", signer.derivation_path())
                .expect("delete first signer only");
            assert!(
                reopened
                    .load("same-node", signer.derivation_path())
                    .expect("deleted first signer")
                    .is_none()
            );
            checked_tx.send(()).expect("first fixture checked");
        });
        let second = scope.spawn(move || {
            stored_rx.recv().expect("first fixture is live");
            let repo = PrivateKeyKeyringRepo::new_with_store("same-service", second_store.clone());
            let signer = DerivedKey::new(
                XPrv::new([2; 32]).expect("second signer"),
                DerivationPath::default(),
            );
            repo.store("same-node", &signer)
                .expect("store second signer");
            replaced_tx.send(()).expect("second signer stored");
            checked_rx
                .recv()
                .expect("first fixture reopened and deleted");
            assert_eq!(
                repo.load("same-node", signer.derivation_path())
                    .expect("load second signer")
                    .expect("second signer retained")
                    .to_xprv_string(),
                signer.to_xprv_string()
            );
            drop(repo);
            let reopened = PrivateKeyKeyringRepo::new_with_store("same-service", second_store);
            assert_eq!(
                reopened
                    .load("same-node", signer.derivation_path())
                    .expect("reopen second signer")
                    .expect("retained second signer")
                    .to_xprv_string(),
                signer.to_xprv_string()
            );
        });
        first.join().expect("first fixture thread");
        second.join().expect("second fixture thread");
    });
}
