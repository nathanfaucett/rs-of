use alloc::{
    format,
    string::{String, ToString},
    sync::Arc,
};

use key::{DerivationPath, DerivedKey, create_secret_entry, init_credential_store};

use crate::repo::{PrivateKeyRepo, RepoError, RepoResult};

pub struct PrivateKeyKeyringRepo {
    service_name: String,
    store: Arc<keyring_core::CredentialStore>,
}

impl PrivateKeyKeyringRepo {
    pub fn new(service_name: impl Into<String>) -> RepoResult<Self> {
        init_credential_store()?;
        Ok(Self {
            service_name: service_name.into(),
            store: keyring_core::get_default_store().expect("initialized credential store"),
        })
    }

    /// Retains this backend without changing the process default; reopen with the same store.
    pub fn new_with_store(
        service_name: impl Into<String>,
        store: Arc<keyring_core::CredentialStore>,
    ) -> Self {
        Self {
            service_name: service_name.into(),
            store,
        }
    }
}

impl PrivateKeyRepo for PrivateKeyKeyringRepo {
    fn load(
        &self,
        namespace: &str,
        derivation_path: &DerivationPath,
    ) -> RepoResult<Option<DerivedKey>> {
        let entry_name = derived_key_entry_name(namespace, &derivation_path.to_string());
        let entry = create_secret_entry(&*self.store, &self.service_name, &entry_name, "IdP")?;

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
        let entry_name =
            derived_key_entry_name(namespace, &derived_key.derivation_path().to_string());
        let entry = create_secret_entry(&*self.store, &self.service_name, &entry_name, "IdP")?;
        entry.set_secret(derived_key.to_xprv_string().as_bytes())?;
        Ok(())
    }

    fn delete(&self, namespace: &str, derivation_path: &DerivationPath) -> RepoResult<()> {
        let entry_name = derived_key_entry_name(namespace, &derivation_path.to_string());
        let entry = create_secret_entry(&*self.store, &self.service_name, &entry_name, "IdP")?;
        entry.delete_credential()?;
        Ok(())
    }
}

fn derived_key_entry_name(namespace: &str, derivation_path: &str) -> String {
    format!("{namespace}:{derivation_path}")
}
