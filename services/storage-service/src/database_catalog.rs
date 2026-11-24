use std::{collections::BTreeMap, fmt, fs, io, path::PathBuf, sync::Mutex};

#[path = "sql_catalog.rs"]
mod sql_catalog;

use of_model::{
    id::Id,
    ofdb::up_storage_catalog,
    storage::{
        namespace::StorageNamespace,
        resource_catalog::{ResourceCatalog, ResourceIdentity, ResourceKind},
    },
};
use ofdb_sql::{Client, Database};

use crate::persistence::{error, run};

pub use of_model::storage::database_resource::{DatabaseId, DatabaseResource};

#[derive(Clone, Debug)]
struct CatalogRecord {
    resource: DatabaseResource,
    deleted: bool,
    initializing: bool,
}

#[derive(Default)]
struct Catalog {
    records: BTreeMap<DatabaseId, CatalogRecord>,
}

pub struct DatabaseCatalog {
    client: Client,
    lock: Mutex<()>,
}

impl fmt::Debug for DatabaseCatalog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DatabaseCatalog").finish_non_exhaustive()
    }
}

impl DatabaseCatalog {
    /// Opens the standalone catalog.
    ///
    /// # Errors
    ///
    /// Returns an error if the catalog cannot be opened or initialized.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "the public constructor accepts an owned catalog root"
    )]
    pub fn new(root: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&root)?;
        let catalog = root.join("catalog.redb");
        if !catalog.try_exists()? && fs::read_dir(&root)?.next().transpose()?.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "database catalog is missing from a nonempty data directory",
            ));
        }
        let client = Database::open(catalog).map_err(error)?.client();
        run(async { up_storage_catalog(&client).await.map_err(error) })?;
        Ok(Self::with_client(client))
    }

    /// Uses the initialized catalog in a shared Storage host.
    #[must_use]
    pub const fn with_client(client: Client) -> Self {
        Self {
            client,
            lock: Mutex::new(()),
        }
    }

    fn with_catalog<R: Send>(
        &self,
        scope: &impl StorageNamespace,
        write: bool,
        edit: impl FnOnce(&mut Catalog) -> io::Result<R> + Send,
    ) -> io::Result<R> {
        validate_subject(scope.user_sub())?;
        if scope.application_id().is_nil() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid storage namespace",
            ));
        }
        let _guard = self
            .lock
            .lock()
            .map_err(|_error| io::Error::other("database catalog lock poisoned"))?;
        sql_catalog::edit(
            &self.client,
            scope.user_sub(),
            scope.application_id(),
            write,
            edit,
        )
    }

    pub(crate) fn owner_resources(&self, owner: &str) -> io::Result<Vec<(Id, DatabaseId)>> {
        validate_subject(owner)?;
        sql_catalog::owner_resources(&self.client, owner)
    }

    /// Creates a database Resource in `scope`.
    ///
    /// # Errors
    ///
    /// Returns an error if the namespace is invalid or the catalog cannot be updated.
    pub fn create<S: StorageNamespace>(
        &self,
        scope: &S,
        name: Option<String>,
    ) -> io::Result<DatabaseResource> {
        self.with_catalog(scope, true, |catalog| {
            let resource = DatabaseResource {
                id: DatabaseId::now_v7(),
                name,
            };
            catalog.records.insert(
                resource.id,
                CatalogRecord {
                    resource: resource.clone(),
                    deleted: false,
                    initializing: false,
                },
            );
            Ok(resource)
        })
    }

    pub(crate) fn reserve<S: StorageNamespace>(
        &self,
        scope: &S,
        key: &str,
        name: Option<String>,
    ) -> io::Result<(DatabaseResource, bool)> {
        if key.is_empty() || key.len() > 128 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid creation key",
            ));
        }
        let identity = serde_json::to_vec(&[
            "of:database:creation:v1",
            scope.user_sub(),
            &scope.application_id().to_string(),
            key,
        ])?;
        let id = DatabaseId::new_v5(&DatabaseId::NAMESPACE_URL, &identity);
        self.with_catalog(scope, true, |catalog| {
            if let Some(record) = catalog.records.get(&id) {
                if record.deleted {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "database deleted",
                    ));
                }
                return Ok((record.resource.clone(), record.initializing));
            }
            let resource = DatabaseResource { id, name };
            catalog.records.insert(
                id,
                CatalogRecord {
                    resource: resource.clone(),
                    deleted: false,
                    initializing: true,
                },
            );
            Ok((resource, true))
        })
    }

    pub(crate) fn finish_creation<S: StorageNamespace>(
        &self,
        scope: &S,
        id: DatabaseId,
    ) -> io::Result<()> {
        self.with_catalog(scope, true, |catalog| {
            let record = catalog
                .records
                .get_mut(&id)
                .filter(|r| !r.deleted)
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "database not found"))?;
            record.initializing = false;
            Ok(())
        })
    }

    /// Lists live database Resources in `scope`.
    ///
    /// # Errors
    ///
    /// Returns an error if the namespace is invalid or the catalog cannot be read.
    pub fn list<S: StorageNamespace>(&self, scope: &S) -> io::Result<Vec<DatabaseResource>> {
        self.with_catalog(scope, false, |catalog| {
            Ok(catalog
                .records
                .values()
                .filter(|&record| !record.deleted && !record.initializing)
                .cloned()
                .map(|record| record.resource)
                .collect())
        })
    }

    /// Registers a selected database Resource without reviving a Tombstone.
    ///
    /// # Errors
    ///
    /// Returns an error if the namespace is invalid or the catalog cannot be updated.
    pub fn register_selected<S: StorageNamespace>(
        &self,
        scope: &S,
        id: DatabaseId,
    ) -> io::Result<bool> {
        self.with_catalog(scope, true, |catalog| {
            if let Some(record) = catalog.records.get(&id) {
                return Ok(!record.deleted && !record.initializing);
            }
            let resource = DatabaseResource { id, name: None };
            catalog.records.insert(
                id,
                CatalogRecord {
                    resource,
                    deleted: false,
                    initializing: false,
                },
            );
            Ok(true)
        })
    }

    pub(crate) fn lifecycle<S: StorageNamespace>(
        &self,
        scope: &S,
        id: DatabaseId,
    ) -> io::Result<Option<bool>> {
        self.with_catalog(scope, false, |catalog| {
            Ok(catalog.records.get(&id).map(|record| record.initializing))
        })
    }

    /// Gets a live database Resource from `scope`.
    ///
    /// # Errors
    ///
    /// Returns an error if the namespace is invalid or the catalog cannot be read.
    pub fn get<S: StorageNamespace>(
        &self,
        scope: &S,
        id: DatabaseId,
    ) -> io::Result<Option<DatabaseResource>> {
        self.with_catalog(scope, false, |catalog| {
            Ok(catalog
                .records
                .get(&id)
                .filter(|record| !record.deleted && !record.initializing)
                .map(|record| record.resource.clone()))
        })
    }

    /// Marks a database Resource as deleted.
    ///
    /// # Errors
    ///
    /// Returns an error if the namespace is invalid or the catalog cannot be updated.
    pub fn delete<S: StorageNamespace>(&self, scope: &S, id: DatabaseId) -> io::Result<bool> {
        self.with_catalog(scope, true, |catalog| {
            let Some(record) = catalog
                .records
                .get_mut(&id)
                .filter(|record| !record.deleted && !record.initializing)
            else {
                return Ok(false);
            };
            record.deleted = true;
            Ok(true)
        })
    }

    /// Applies an absorbing deletion Tombstone.
    ///
    /// # Errors
    ///
    /// Returns an error if the namespace is invalid or the catalog cannot be updated.
    pub fn apply_tombstone<S: StorageNamespace>(
        &self,
        scope: &S,
        id: DatabaseId,
    ) -> io::Result<()> {
        self.with_catalog(scope, true, |catalog| {
            if catalog
                .records
                .get(&id)
                .is_some_and(|record| record.deleted)
            {
                return Ok(());
            }
            let record = catalog.records.entry(id).or_insert_with(|| CatalogRecord {
                resource: DatabaseResource { id, name: None },
                deleted: true,
                initializing: false,
            });
            record.deleted = true;
            Ok(())
        })
    }

    /// Returns whether a database Resource has a deletion Tombstone.
    ///
    /// # Errors
    ///
    /// Returns an error if the namespace is invalid or the catalog cannot be read.
    pub fn is_tombstoned<S: StorageNamespace>(
        &self,
        scope: &S,
        id: DatabaseId,
    ) -> io::Result<bool> {
        self.with_catalog(scope, false, |catalog| {
            Ok(catalog
                .records
                .get(&id)
                .is_some_and(|record| record.deleted))
        })
    }
}

impl ResourceCatalog for DatabaseCatalog {
    #[allow(
        clippy::future_not_send,
        reason = "ResourceCatalog does not require Send futures"
    )]
    #[allow(
        clippy::unused_async_trait_impl,
        reason = "ResourceCatalog defines an asynchronous lookup contract"
    )]
    async fn contains(
        &self,
        namespace: &dyn StorageNamespace,
        resource: &ResourceIdentity,
    ) -> Result<bool, String> {
        if resource.kind != ResourceKind::Database {
            return Ok(false);
        }
        let Ok(id) = resource.id.parse::<DatabaseId>() else {
            return Ok(false);
        };
        self.get(&NamespaceRef(namespace), id)
            .map(|entry| entry.is_some())
            .map_err(|error| error.to_string())
    }
}

struct NamespaceRef<'a>(&'a dyn StorageNamespace);

impl StorageNamespace for NamespaceRef<'_> {
    fn user_sub(&self) -> &str {
        self.0.user_sub()
    }

    fn application_id(&self) -> Id {
        self.0.application_id()
    }
}

fn validate_subject(subject: &str) -> io::Result<()> {
    if subject.is_empty()
        || matches!(subject, "." | "..")
        || !subject
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid storage namespace",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{env, fs, process};

    use of_model::{
        id::Id,
        storage::{
            database_resource::DatabaseResource,
            namespace::StorageNamespace,
            resource_catalog::{ResourceCatalog, ResourceIdentity, ResourceKind},
        },
    };

    use crate::database_catalog::DatabaseCatalog;

    #[derive(Clone, Copy)]
    struct Scope {
        subject: &'static str,
        application: Id,
    }

    impl StorageNamespace for Scope {
        fn user_sub(&self) -> &str {
            self.subject
        }

        fn application_id(&self) -> Id {
            self.application
        }
    }

    #[test]
    fn refuses_to_initialize_over_existing_data_without_a_catalog() {
        let root = env::temp_dir().join(format!("database-catalog-existing-{}", Id::now_v7()));
        fs::create_dir_all(&root).expect("existing data directory");
        fs::write(root.join("existing-data"), b"data").expect("existing data");
        DatabaseCatalog::new(root.clone()).expect_err("existing data requires a catalog");
        assert!(!root.join("catalog.redb").exists());
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[tokio::test]
    async fn standalone_catalog_does_not_create_other_owners_tables() {
        let root = env::temp_dir().join(format!("database-catalog-schema-{}", Id::now_v7()));
        let catalog = DatabaseCatalog::new(root.clone()).expect("catalog root is created");
        for table in ["users", "devices"] {
            catalog
                .client
                .execute_sql(&format!("SELECT * FROM {table} LIMIT 0"), None)
                .await
                .expect_err("standalone catalog must not create other owners' tables");
        }
        drop(catalog);
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[tokio::test]
    async fn resources_are_isolated_persistent_and_tombstoned() {
        let root = env::temp_dir().join(format!(
            "database-catalog-{}-{}",
            process::id(),
            Id::now_v7()
        ));
        let scope = Scope {
            subject: "subject-a",
            application: Id::now_v7(),
        };
        let other_subject = Scope {
            subject: "subject-b",
            ..scope
        };
        let other_application = Scope {
            application: Id::now_v7(),
            ..scope
        };
        let catalog = DatabaseCatalog::new(root.clone()).expect("catalog root is created");

        let first = catalog
            .create(&scope, Some("same name".into()))
            .expect("first resource is created");
        let second = catalog
            .create(&scope, Some("same name".into()))
            .expect("second resource is created");
        assert_ne!(first.id, second.id);
        let first_identity = ResourceIdentity {
            kind: ResourceKind::Database,
            id: first.id.to_string(),
        };
        assert!(
            catalog
                .contains(&scope, &first_identity)
                .await
                .expect("catalog lookup works")
        );
        assert!(
            !catalog
                .contains(&other_subject, &first_identity)
                .await
                .expect("foreign lookup works")
        );
        assert!(
            !catalog
                .contains(
                    &scope,
                    &ResourceIdentity {
                        kind: ResourceKind::FileSystem,
                        id: first.id.to_string(),
                    },
                )
                .await
                .expect("cross-kind lookup works")
        );
        assert_eq!(
            catalog.list(&other_subject).expect("other subject lists"),
            [] as [DatabaseResource; 0]
        );
        assert_eq!(
            catalog
                .list(&other_application)
                .expect("other application lists"),
            [] as [DatabaseResource; 0]
        );
        drop(catalog);

        let catalog = DatabaseCatalog::new(root.clone()).expect("catalog reopens");
        assert_eq!(
            catalog
                .list(&scope)
                .expect("resources list after restart")
                .len(),
            2
        );
        assert!(
            catalog
                .delete(&scope, first.id)
                .expect("resource tombstones")
        );
        assert!(
            catalog
                .get(&scope, first.id)
                .expect("lookup succeeds")
                .is_none()
        );
        assert!(
            !catalog
                .delete(&scope, first.id)
                .expect("repeat delete succeeds")
        );
        assert_eq!(
            catalog.list(&scope).expect("live list succeeds"),
            vec![second]
        );
        drop(catalog);
        fs::remove_dir_all(root).expect("test catalog is removed");
    }

    #[test]
    fn imported_tombstone_blocks_late_resource_provisioning_after_restart() {
        let root = env::temp_dir().join(format!(
            "database-catalog-imported-tombstone-{}-{}",
            process::id(),
            Id::now_v7()
        ));
        let scope = Scope {
            subject: "subject-a",
            application: Id::now_v7(),
        };
        let id = Id::now_v7();
        let catalog = DatabaseCatalog::new(root.clone()).expect("catalog root is created");
        catalog
            .apply_tombstone(&scope, id)
            .expect("unknown resource tombstone imports");
        assert!(
            !catalog
                .register_selected(&scope, id)
                .expect("late selected-resource projection is rejected")
        );
        drop(catalog);

        let catalog = DatabaseCatalog::new(root.clone()).expect("catalog reopens");
        assert!(
            catalog
                .is_tombstoned(&scope, id)
                .expect("imported tombstone survives restart")
        );
        assert!(
            !catalog
                .register_selected(&scope, id)
                .expect("reselection cannot revive imported tombstone")
        );
        drop(catalog);
        fs::remove_dir_all(root).expect("test catalog is removed");
    }

    #[test]
    fn rejects_invalid_namespace_paths() {
        let root = env::temp_dir().join(format!("database-catalog-invalid-{}", process::id()));
        let catalog = DatabaseCatalog::new(root.clone()).expect("catalog root is created");
        let scope = Scope {
            subject: "../other",
            application: Id::now_v7(),
        };
        catalog.list(&scope).unwrap_err();
        fs::remove_dir_all(root).expect("test catalog is removed");
    }
}
