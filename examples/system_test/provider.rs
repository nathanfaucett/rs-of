use std::{
    any::Any,
    collections::HashMap,
    fs, io,
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use keyring_core::{
    Entry,
    api::{Credential, CredentialApi, CredentialStoreApi},
};

type Records = Vec<((String, String), Vec<u8>)>;

#[derive(Clone)]
pub struct Store {
    mock: Arc<keyring_core::mock::Store>,
    path: Arc<PathBuf>,
    lock: Arc<Mutex<()>>,
}

impl Store {
    pub fn open(path: PathBuf) -> io::Result<Self> {
        let mock = keyring_core::mock::Store::new().map_err(io::Error::other)?;
        match fs::read(&path) {
            Ok(bytes) => {
                let records: Records = serde_json::from_slice(&bytes)?;
                for ((service, user), secret) in records {
                    mock.build(&service, &user, None)
                        .and_then(|entry| entry.set_secret(&secret))
                        .map_err(io::Error::other)?;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => (),
            Err(error) => return Err(error),
        }
        Ok(Self {
            mock,
            path: Arc::new(path),
            lock: Arc::new(Mutex::new(())),
        })
    }

    fn wrap(&self, entry: Entry) -> Entry {
        Entry::new_with_credential(Arc::new(StoredCredential {
            entry,
            store: self.clone(),
        }))
    }

    fn save(&self) -> keyring_core::Result<()> {
        let records: Records = self
            .mock
            .search(&HashMap::new())?
            .into_iter()
            .filter_map(|entry| match entry.get_secret() {
                Ok(secret) => Some(Ok((
                    entry.get_specifiers().expect("mock entry specifiers"),
                    secret,
                ))),
                Err(keyring_core::Error::NoEntry) => None,
                Err(error) => Some(Err(error)),
            })
            .collect::<keyring_core::Result<_>>()?;
        let save = || -> io::Result<()> {
            use std::io::Write;
            let temporary = self.path.with_extension("new");
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&temporary)?;
            file.write_all(&serde_json::to_vec(&records)?)?;
            file.sync_all()?;
            fs::rename(temporary, &*self.path)?;
            fs::File::open(self.path.parent().expect("private provider parent"))?.sync_all()
        };
        save().map_err(|error| keyring_core::Error::PlatformFailure(Box::new(error)))
    }
}

impl CredentialStoreApi for Store {
    fn vendor(&self) -> String {
        self.mock.vendor()
    }
    fn id(&self) -> String {
        self.path.to_string_lossy().into_owned()
    }
    fn build(
        &self,
        service: &str,
        user: &str,
        _: Option<&HashMap<&str, &str>>,
    ) -> keyring_core::Result<Entry> {
        self.mock
            .build(service, user, None)
            .map(|entry| self.wrap(entry))
    }
    fn search(&self, spec: &HashMap<&str, &str>) -> keyring_core::Result<Vec<Entry>> {
        self.mock
            .search(spec)
            .map(|entries| entries.into_iter().map(|entry| self.wrap(entry)).collect())
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct StoredCredential {
    entry: Entry,
    store: Store,
}
impl CredentialApi for StoredCredential {
    fn set_secret(&self, secret: &[u8]) -> keyring_core::Result<()> {
        let _guard = self.store.lock.lock().expect("provider write lock");
        self.entry.set_secret(secret)?;
        self.store.save()
    }
    fn get_secret(&self) -> keyring_core::Result<Vec<u8>> {
        self.entry.get_secret()
    }
    fn delete_credential(&self) -> keyring_core::Result<()> {
        let _guard = self.store.lock.lock().expect("provider write lock");
        self.entry.delete_credential()?;
        self.store.save()
    }
    fn get_credential(&self) -> keyring_core::Result<Option<Arc<Credential>>> {
        self.entry.get_secret()?;
        Ok(None)
    }
    fn get_specifiers(&self) -> Option<(String, String)> {
        self.entry.get_specifiers()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::Store;
    use keyring_core::api::CredentialStoreApi;

    #[test]
    fn retained_provider_reopens_replaces_and_deletes_secrets() {
        let root = tempfile::tempdir().expect("private provider root");
        let path = root.path().join("provider");
        let store = Store::open(path.clone()).expect("fresh provider");
        let entry = store.build("service", "user", None).expect("entry");
        entry
            .set_secret(b"first local secret")
            .expect("persist secret");
        drop(entry);
        drop(store);
        let store = Store::open(path.clone()).expect("retained provider");
        let entry = store
            .build("service", "user", None)
            .expect("retained entry");
        assert_eq!(
            entry.get_secret().expect("retained secret"),
            b"first local secret"
        );
        entry
            .set_secret(b"replacement local secret")
            .expect("replace secret");
        drop(entry);
        drop(store);
        let store = Store::open(path.clone()).expect("replacement provider");
        let entry = store
            .build("service", "user", None)
            .expect("replacement entry");
        assert_eq!(
            entry.get_secret().expect("replacement secret"),
            b"replacement local secret"
        );
        entry.delete_credential().expect("persist deletion");
        drop(entry);
        drop(store);
        let store = Store::open(path).expect("provider after deletion");
        assert!(matches!(
            store
                .build("service", "user", None)
                .expect("deleted entry")
                .get_secret(),
            Err(keyring_core::Error::NoEntry)
        ));
    }
}
