#![cfg_attr(target_arch = "wasm32", allow(dead_code))] // browser construction lands with the browser backend

use std::fmt::{self, Debug, Display};
#[cfg(not(target_arch = "wasm32"))]
use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(not(target_arch = "wasm32"))]
use crate::{native::remove_root, shared::remove};
#[cfg(not(target_arch = "wasm32"))]
use ofdb_kv::Database;
#[cfg(all(target_arch = "wasm32", feature = "browser"))]
use ofdb_kv::Database;
use ofdb_kv::{Client, Value};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

use crate::Error;

const CATALOG_PREFIX: &str = "catalog:";
const PROJECTED_PREFIX: &str = "projected:";
const CATALOG_TABLE: &str = "filesystem_catalog";

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct FileSystemId(Uuid);

impl FileSystemId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// Parses a `UUIDv7` filesystem identifier.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidResourceId`] for malformed or non-UUIDv7 identifiers.
    pub fn parse(value: &str) -> Result<Self, Error> {
        let id = Uuid::parse_str(value).map_err(|_error| Error::InvalidResourceId)?;
        if id.get_version_num() != 7 {
            return Err(Error::InvalidResourceId);
        }
        Ok(Self(id))
    }

    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }

    fn key(&self) -> String {
        format!("{}{}", CATALOG_PREFIX, self.0)
    }

    fn projected_key(&self) -> String {
        format!("{}{}", PROJECTED_PREFIX, self.0)
    }
}

impl Default for FileSystemId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FileSystemResource {
    pub id: FileSystemId,
    pub name: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CatalogEntry {
    pub resource: FileSystemResource,
    pub deleted: bool,
}

pub struct FileSystemCatalog {
    #[cfg(not(target_arch = "wasm32"))]
    storage: CatalogStorage,
    #[cfg(all(target_arch = "wasm32", feature = "browser"))]
    namespace: String,
    client: Client,
}

#[cfg(not(target_arch = "wasm32"))]
enum CatalogStorage {
    Native(PathBuf),
    Shared(ofdb_kv::SharedDatabase, String),
}

impl fmt::Debug for FileSystemCatalog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileSystemCatalog").finish_non_exhaustive()
    }
}

impl FileSystemCatalog {
    /// Opens a catalog in a shared database host.
    ///
    /// # Errors
    ///
    /// Returns an error if the catalog database cannot be opened.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open_shared(host: &ofdb_kv::SharedDatabase, name: &str) -> Result<Self, Error> {
        let database =
            Database::open_shared(host, &format!("{name}:catalog")).map_err(metadata_error)?;
        Ok(Self {
            storage: CatalogStorage::Shared(host.clone(), name.to_owned()),
            client: database.client(),
        })
    }

    /// Opens a catalog under a native namespace root.
    ///
    /// # Errors
    ///
    /// Returns an error for directory or catalog database failures.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open(namespace_root: impl AsRef<Path>) -> Result<Self, Error> {
        let root = namespace_root.as_ref().to_owned();
        fs::create_dir_all(&root)?;
        let database = Database::open_with_table(root.join("catalog.redb"), CATALOG_TABLE)
            .map_err(metadata_error)?;
        Ok(Self {
            storage: CatalogStorage::Native(root.join("filesystems")),
            client: database.client(),
        })
    }

    #[cfg(all(target_arch = "wasm32", feature = "browser"))]
    pub async fn open_browser(namespace: &str) -> Result<Self, Error> {
        let database = Database::open_indexeddb(&format!("offs:{namespace}:catalog"))
            .await
            .map_err(metadata_error)?;
        Ok(Self {
            namespace: namespace.to_owned(),
            client: database.client(),
        })
    }

    /// Creates and persists a filesystem catalog entry.
    ///
    /// # Errors
    ///
    /// Returns an error for metadata encoding or storage failures.
    pub async fn create(&self, name: Option<String>) -> Result<FileSystemResource, Error> {
        let resource = FileSystemResource {
            id: FileSystemId::new(),
            name,
        };
        self.client
            .set(
                &resource.id.key(),
                Value::Blob(encode(&CatalogEntry {
                    resource: resource.clone(),
                    deleted: false,
                })?),
                None,
            )
            .await
            .map_err(metadata_error)?;
        Ok(resource)
    }

    /// Opens an existing filesystem by identifier.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier is unknown or the filesystem cannot be opened.
    pub async fn open_filesystem<PeerId>(
        &self,
        id: FileSystemId,
        node_id: PeerId,
    ) -> Result<crate::FileSystem<PeerId>, Error>
    where
        PeerId: Clone
            + Debug
            + Ord
            + Serialize
            + DeserializeOwned
            + ofdb_platform::MaybeSend
            + ofdb_platform::MaybeSync
            + 'static,
    {
        if !self.list().await?.iter().any(|resource| resource.id == id) {
            return Err(Error::NotFound);
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            match &self.storage {
                CatalogStorage::Native(root) => {
                    crate::FileSystem::open(root.join(id.0.to_string()), node_id).await
                }
                CatalogStorage::Shared(host, name) => {
                    crate::FileSystem::open_shared(host, &format!("{name}:{}", id.0), node_id).await
                }
            }
        }
        #[cfg(all(target_arch = "wasm32", feature = "browser"))]
        {
            let root = format!("{}:{}", self.namespace, id.0);
            crate::FileSystem::open_browser(root, node_id).await
        }
        #[cfg(all(target_arch = "wasm32", not(feature = "browser")))]
        {
            let _ = node_id;
            Err(Error::PermissionDenied)
        }
    }

    /// Lists active filesystem resources.
    ///
    /// # Errors
    ///
    /// Returns an error for metadata storage or decoding failures.
    pub async fn list(&self) -> Result<Vec<FileSystemResource>, Error> {
        Ok(self
            .snapshot()
            .await?
            .into_iter()
            .filter(|entry| !entry.deleted)
            .map(|entry| entry.resource)
            .collect())
    }

    /// Marks a filesystem resource as deleted.
    ///
    /// # Errors
    ///
    /// Returns an error if the resource is absent or the transaction fails.
    pub async fn delete(&self, id: FileSystemId) -> Result<(), Error> {
        let mut transaction = self.client.transaction().await.map_err(metadata_error)?;
        let mut entry: CatalogEntry = transaction
            .get(&id.key())
            .await
            .map_err(metadata_error)?
            .map(decode_value)
            .transpose()?
            .ok_or(Error::NotFound)?;
        if entry.deleted {
            return Err(Error::NotFound);
        }
        entry.deleted = true;
        transaction
            .set(&id.key(), Value::Blob(encode(&entry)?), None)
            .await
            .map_err(metadata_error)?;
        transaction.commit().await.map_err(metadata_error)
    }

    /// Marks an existing projection as selected.
    ///
    /// # Errors
    ///
    /// Returns an error if the projection transaction fails.
    #[allow(
        clippy::significant_drop_tightening,
        reason = "commit consumes the transaction"
    )]
    pub async fn mark_projected_selected(&self, id: FileSystemId) -> Result<bool, Error> {
        let mut transaction = self.client.transaction().await.map_err(metadata_error)?;
        let key = id.projected_key();
        let exists = transaction
            .get(&key)
            .await
            .map_err(metadata_error)?
            .is_some();
        if exists {
            transaction
                .set(&key, Value::Integer(1), None)
                .await
                .map_err(metadata_error)?;
        }
        transaction.commit().await.map_err(metadata_error)?;
        Ok(exists)
    }

    /// Records a projected filesystem.
    ///
    /// # Errors
    ///
    /// Returns an error if the metadata write fails.
    pub async fn mark_projected(&self, id: FileSystemId) -> Result<(), Error> {
        self.client
            .set(&id.projected_key(), Value::Integer(1), None)
            .await
            .map_err(metadata_error)
    }

    /// Lists selected projected filesystem identifiers.
    ///
    /// # Errors
    ///
    /// Returns an error for metadata storage or identifier decoding failures.
    pub async fn projected_selected(&self) -> Result<Vec<FileSystemId>, Error> {
        self.client
            .scan_prefix(PROJECTED_PREFIX)
            .await
            .map_err(metadata_error)?
            .into_iter()
            .filter(|(_, value)| value == &Value::Integer(1))
            .map(|(key, _)| projected_id(&key))
            .collect()
    }

    /// Permanently retires this device's managed copy without changing catalog identity or
    /// tombstones. Cancellation or failure leaves the durable gate closed; call again to
    /// finish cleanup.
    ///
    /// # Errors
    ///
    /// Returns an error if the resource is absent or managed-copy cleanup fails.
    pub async fn remove_managed_copy(&self, id: FileSystemId) -> Result<(), Error> {
        if !self
            .snapshot()
            .await?
            .iter()
            .any(|entry| entry.resource.id == id)
        {
            return Err(Error::NotFound);
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            match &self.storage {
                CatalogStorage::Native(root) => {
                    remove_root(&root.join(id.0.to_string())).await?;
                }
                CatalogStorage::Shared(host, name) => {
                    remove(host, &format!("{name}:{}", id.0)).await?;
                }
            }
        }
        #[cfg(all(target_arch = "wasm32", feature = "browser"))]
        {
            let root = format!("{}:{}", self.namespace, id.0);
            crate::browser::remove_root(root).await?;
        }
        let mut transaction = self.client.transaction().await.map_err(metadata_error)?;
        let key = id.projected_key();
        if transaction
            .get(&key)
            .await
            .map_err(metadata_error)?
            .is_some()
        {
            transaction
                .set(&key, Value::Integer(0), None)
                .await
                .map_err(metadata_error)?;
        }
        transaction.commit().await.map_err(metadata_error)
    }

    /// Retires a selected projection using the same owner lifecycle as created copies.
    ///
    /// # Errors
    ///
    /// Returns an error if projection lookup or managed-copy cleanup fails.
    pub async fn evict_projected_copy(&self, id: FileSystemId) -> Result<bool, Error> {
        if !self.projected_selected().await?.contains(&id) {
            return Ok(false);
        }
        self.remove_managed_copy(id).await?;
        Ok(true)
    }

    /// Returns all catalog entries, including tombstones.
    ///
    /// # Errors
    ///
    /// Returns an error for metadata storage or decoding failures.
    pub async fn snapshot(&self) -> Result<Vec<CatalogEntry>, Error> {
        self.client
            .scan_prefix(CATALOG_PREFIX)
            .await
            .map_err(metadata_error)?
            .into_iter()
            .map(|(_, value)| decode_value(value))
            .collect()
    }

    /// Applies a tombstone while retaining any known resource name.
    ///
    /// # Errors
    ///
    /// Returns an error for metadata storage or decoding failures.
    pub async fn apply_tombstone(&self, id: FileSystemId) -> Result<(), Error> {
        let existing = self
            .snapshot()
            .await?
            .into_iter()
            .find(|entry| entry.resource.id == id);
        let resource = existing
            .map(|entry| entry.resource)
            .unwrap_or(FileSystemResource { id, name: None });
        self.import_snapshot(&[CatalogEntry {
            resource,
            deleted: true,
        }])
        .await
    }

    /// Merges catalog entries without resurrecting deleted resources.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid identifiers, malformed entries, or transaction failures.
    pub async fn import_snapshot(&self, entries: &[CatalogEntry]) -> Result<(), Error> {
        if entries
            .iter()
            .any(|entry| entry.resource.id.0.get_version_num() != 7)
        {
            return Err(Error::InvalidResourceId);
        }
        let mut transaction = self.client.transaction().await.map_err(metadata_error)?;
        for incoming in entries {
            let key = incoming.resource.id.key();
            let existing = transaction
                .get(&key)
                .await
                .map_err(metadata_error)?
                .map(decode_value)
                .transpose()?;
            let merged = match existing {
                Some(current) if current.deleted => current,
                Some(_) if !incoming.deleted => continue,
                _ => incoming.clone(),
            };
            transaction
                .set(&key, Value::Blob(encode(&merged)?), None)
                .await
                .map_err(metadata_error)?;
        }
        transaction.commit().await.map_err(metadata_error)
    }
}

fn encode(entry: &CatalogEntry) -> Result<Vec<u8>, Error> {
    postcard::to_allocvec(entry).map_err(metadata_error)
}

fn decode_value(value: Value) -> Result<CatalogEntry, Error> {
    let Value::Blob(bytes) = value else {
        return Err(metadata_error("catalog value is not a blob"));
    };
    postcard::from_bytes(&bytes).map_err(metadata_error)
}

fn projected_id(key: &str) -> Result<FileSystemId, Error> {
    let id = key
        .strip_prefix(PROJECTED_PREFIX)
        .and_then(|rest| Uuid::parse_str(rest).ok())
        .ok_or(Error::InvalidResourceId)?;
    Ok(FileSystemId(id))
}

fn metadata_error(error: impl Display) -> Error {
    Error::Metadata(error.to_string())
}
