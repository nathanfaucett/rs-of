use std::future::Future;

use key::{DerivationPath, DerivedKey};
use of_model::idp::model::Id;
use ofdb_kv::{Client, Database, SharedDatabase, Value};

use crate::repo::{RepoError, RepoResult};

pub struct LocalPrivateKeys(Client);

impl LocalPrivateKeys {
    pub fn new(storage: &SharedDatabase) -> RepoResult<Self> {
        Ok(Self(
            Database::open_shared(storage, "idp-private-keys")
                .map_err(error)?
                .client(),
        ))
    }

    pub fn load(
        &self,
        namespace: &str,
        derivation: &DerivationPath,
    ) -> RepoResult<Option<DerivedKey>> {
        run(async {
            match self
                .0
                .get(&key_id(namespace, derivation))
                .await
                .map_err(error)?
            {
                Some(Value::Text(secret)) => {
                    Ok(Some(DerivedKey::from_xprv(secret, derivation.clone())?))
                }
                None => Ok(None),
                _ => Err(RepoError::InvalidInput("invalid local private key".into())),
            }
        })
    }

    pub fn store(&self, namespace: &str, key: &DerivedKey) -> RepoResult<()> {
        run(async {
            self.0
                .set(
                    &key_id(namespace, key.derivation_path()),
                    Value::Text(key.to_xprv_string().to_string()),
                    None,
                )
                .await
                .map_err(error)
        })
    }

    pub fn delete(&self, namespace: &str, derivation: &DerivationPath) -> RepoResult<()> {
        run(async {
            self.0
                .delete(&key_id(namespace, derivation))
                .await
                .map_err(error)
        })
    }
}

fn key_id(namespace: &str, derivation: &DerivationPath) -> String {
    let namespace = Id::new_v5(&Id::NAMESPACE_URL, namespace.as_bytes());
    Id::new_v5(&namespace, derivation.to_string().as_bytes()).to_string()
}

fn run<T: Send>(future: impl Future<Output = RepoResult<T>> + Send) -> RepoResult<T> {
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                tokio::runtime::Builder::new_current_thread()
                    .build()
                    .map_err(error)?
                    .block_on(future)
            })
            .join()
            .map_err(|_| RepoError::InvalidInput("local private key worker panicked".into()))?
    })
}

fn error(error: impl std::fmt::Display) -> RepoError {
    RepoError::InvalidInput(error.to_string())
}
