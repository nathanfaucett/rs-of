use alloc::{
    format,
    string::{String, ToString},
    sync::Arc,
};

use key::{DerivationPath, DerivedKey, create_secret_entry, init_credential_store};

use crate::repo::{PrivateKeyRepo, RepoError, RepoResult};

#[cfg(feature = "replica")]
#[path = "local_private_keys.rs"]
mod local_private_keys;

enum PrivateKeyStorage {
    Keyring(Arc<keyring_core::CredentialStore>),
    #[cfg(feature = "replica")]
    Local(local_private_keys::LocalPrivateKeys),
}

pub struct PrivateKeyKeyringRepo {
    service_name: String,
    storage: PrivateKeyStorage,
}

impl PrivateKeyKeyringRepo {
    pub fn new(service_name: impl Into<String>) -> RepoResult<Self> {
        init_credential_store()?;
        Ok(Self {
            service_name: service_name.into(),
            storage: PrivateKeyStorage::Keyring(
                keyring_core::get_default_store().expect("initialized credential store"),
            ),
        })
    }

    /// Retains this backend without changing the process default; reopen with the same store.
    pub fn new_with_store(
        service_name: impl Into<String>,
        store: Arc<keyring_core::CredentialStore>,
    ) -> Self {
        Self {
            service_name: service_name.into(),
            storage: PrivateKeyStorage::Keyring(store),
        }
    }

    /// Uses a local database, which must be private and must never be replicated.
    #[cfg(feature = "replica")]
    pub fn new_local(client: db::Client) -> RepoResult<Self> {
        Ok(Self {
            service_name: String::new(),
            storage: PrivateKeyStorage::Local(local_private_keys::LocalPrivateKeys::new(client)?),
        })
    }
}

impl PrivateKeyRepo for PrivateKeyKeyringRepo {
    fn load(
        &self,
        namespace: &str,
        derivation_path: &DerivationPath,
    ) -> RepoResult<Option<DerivedKey>> {
        let store = match &self.storage {
            PrivateKeyStorage::Keyring(store) => store,
            #[cfg(feature = "replica")]
            PrivateKeyStorage::Local(keys) => return keys.load(namespace, derivation_path),
        };
        let entry_name = derived_key_entry_name(namespace, &derivation_path.to_string());
        let entry = create_secret_entry(&**store, &self.service_name, &entry_name, "IdP")?;

        match entry.get_secret() {
            Ok(secret_bytes) => {
                let xprv = String::from_utf8(secret_bytes)
                    .map_err(|error| RepoError::InvalidInput(error.to_string()))?;
                Ok(Some(DerivedKey::from_xprv(xprv, derivation_path.clone())?))
            }
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn store(&self, namespace: &str, derived_key: &DerivedKey) -> RepoResult<()> {
        let store = match &self.storage {
            PrivateKeyStorage::Keyring(store) => store,
            #[cfg(feature = "replica")]
            PrivateKeyStorage::Local(keys) => return keys.store(namespace, derived_key),
        };
        let entry_name =
            derived_key_entry_name(namespace, &derived_key.derivation_path().to_string());
        let entry = create_secret_entry(&**store, &self.service_name, &entry_name, "IdP")?;
        entry.set_secret(derived_key.to_xprv_string().as_bytes())?;
        Ok(())
    }

    fn delete(&self, namespace: &str, derivation_path: &DerivationPath) -> RepoResult<()> {
        let store = match &self.storage {
            PrivateKeyStorage::Keyring(store) => store,
            #[cfg(feature = "replica")]
            PrivateKeyStorage::Local(keys) => return keys.delete(namespace, derivation_path),
        };
        let entry_name = derived_key_entry_name(namespace, &derivation_path.to_string());
        let entry = create_secret_entry(&**store, &self.service_name, &entry_name, "IdP")?;
        entry.delete_credential()?;
        Ok(())
    }
}

fn derived_key_entry_name(namespace: &str, derivation_path: &str) -> String {
    format!("{namespace}:{derivation_path}")
}
