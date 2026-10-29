use alloc::{borrow::ToOwned, format, string::String, vec::Vec};

use key::SecretStore;

use super::RepoResult;

pub struct RawKeyringRepo(SecretStore);

impl RawKeyringRepo {
    pub fn new(service_name: impl Into<String>) -> RepoResult<Self> {
        Ok(Self(SecretStore::new(service_name, "IdP")?))
    }
    pub fn load(
        &self,
        user_sub: &str,
        client_id: &str,
        key_name: &str,
    ) -> RepoResult<Option<Vec<u8>>> {
        Ok(self.0.load(&entry_name(user_sub, client_id, key_name))?)
    }
    pub fn store(
        &self,
        user_sub: &str,
        client_id: &str,
        key_name: &str,
        bytes: &[u8],
    ) -> RepoResult<()> {
        Ok(self
            .0
            .store(&entry_name(user_sub, client_id, key_name), bytes)?)
    }
    pub fn delete(&self, user_sub: &str, client_id: &str, key_name: &str) -> RepoResult<()> {
        Ok(self.0.delete(&entry_name(user_sub, client_id, key_name))?)
    }
}

fn entry_name(user_sub: &str, client_id: &str, key_name: &str) -> String {
    if user_sub.is_empty() && client_id.is_empty() {
        key_name.to_owned()
    } else {
        format!("{key_name}:{user_sub}:{client_id}")
    }
}
