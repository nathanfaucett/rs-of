use std::{
    any::Any,
    collections::BTreeMap,
    io,
    path::Path,
    sync::{Arc, Mutex, Weak},
};

use async_lock::Mutex as AsyncMutex;
use redb::{ReadableDatabase, TableDefinition, TableHandle, backends::InMemoryBackend};

use crate::{Bytes, RedbByteBTree, RedbDatabase, resource::retired_error, table_definition};

const RETIRED: TableDefinition<&str, bool> = TableDefinition::new("ofdb.shared.retired");
type Owners = BTreeMap<String, Weak<dyn Any + Send + Sync>>;

#[derive(Clone, Debug)]
pub struct SharedDatabase {
    database: Arc<redb::Database>,
    admission: Arc<AsyncMutex<()>>,
    owners: Arc<Mutex<Owners>>,
}

impl SharedDatabase {
    /// Creates a shared database at a filesystem path.
    ///
    /// # Errors
    ///
    /// Returns an error when database creation or initialization fails.
    pub fn create(path: impl AsRef<Path>) -> io::Result<Self> {
        Self::from_database(redb::Database::create(path).map_err(io::Error::other)?)
    }

    /// Creates an in-memory shared database.
    ///
    /// # Errors
    ///
    /// Returns an error when database initialization fails.
    pub fn in_memory() -> io::Result<Self> {
        Self::from_database(
            redb::Database::builder()
                .create_with_backend(InMemoryBackend::new())
                .map_err(io::Error::other)?,
        )
    }

    /// Initializes shared ownership over an open database.
    ///
    /// # Errors
    ///
    /// Returns an error when the retirement table cannot be initialized.
    pub fn from_database(database: redb::Database) -> io::Result<Self> {
        let transaction = database.begin_write().map_err(io::Error::other)?;
        transaction.open_table(RETIRED).map_err(io::Error::other)?;
        transaction.commit().map_err(io::Error::other)?;
        Ok(Self {
            database: Arc::new(database),
            admission: Arc::new(AsyncMutex::new(())),
            owners: Arc::new(Mutex::new(BTreeMap::new())),
        })
    }

    /// Process-local identity for caches; never a persistent resource identity.
    #[must_use]
    pub fn instance_id(&self) -> usize {
        Arc::as_ptr(&self.database) as usize
    }

    /// Opens a validated storage namespace.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid or retired namespace.
    pub fn namespace(&self, name: &str) -> io::Result<SharedNamespace> {
        if name.is_empty() || name.len() > 1024 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid storage namespace",
            ));
        }
        let namespace = SharedNamespace {
            host: self.clone(),
            prefix: format!("ofdb.ns.{}:{name}:", name.len()),
        };
        namespace.check()?;
        Ok(namespace)
    }
}

#[derive(Clone, Debug)]
pub struct SharedNamespace {
    host: SharedDatabase,
    prefix: String,
}

impl SharedNamespace {
    #[must_use]
    pub fn table(&self, name: &str) -> String {
        format!("{}{name}", self.prefix)
    }

    #[must_use]
    pub fn database(&self) -> RedbDatabase {
        RedbDatabase::with_admission(
            Arc::clone(&self.host.database),
            Arc::clone(&self.host.admission),
        )
    }

    #[must_use]
    pub fn tree(&self, name: &str) -> RedbByteBTree {
        RedbByteBTree::with_admission(
            Arc::clone(&self.host.database),
            self.table(name),
            Arc::clone(&self.host.admission),
        )
    }

    /// Checks that this namespace has not been retired.
    ///
    /// # Errors
    ///
    /// Returns an error when the namespace is retired or storage reads fail.
    pub fn check(&self) -> io::Result<()> {
        let transaction = self.host.database.begin_read().map_err(io::Error::other)?;
        let table = transaction.open_table(RETIRED).map_err(io::Error::other)?;
        if table
            .get(self.prefix.as_str())
            .map_err(io::Error::other)?
            .is_some()
        {
            return Err(retired_error());
        }
        Ok(())
    }

    /// Returns or builds the process-local owner for this namespace.
    ///
    /// # Errors
    ///
    /// Returns an error when the namespace is retired or its owner type changes.
    ///
    /// # Panics
    ///
    /// Panics if the owners lock is poisoned.
    #[allow(
        clippy::map_err_ignore,
        reason = "failed downcasts have one stable backend-type error"
    )]
    pub fn owner<T: Any + Send + Sync>(&self, build: impl FnOnce() -> T) -> io::Result<Arc<T>> {
        let mut owners = self
            .host
            .owners
            .lock()
            .expect("shared namespace owners lock");
        self.check()?;
        if let Some(owner) = owners.get(&self.prefix).and_then(Weak::upgrade) {
            return owner.downcast().map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "namespace backend type mismatch",
                )
            });
        }
        owners.retain(|_, owner| owner.strong_count() != 0);
        let owner = Arc::new(build());
        let erased = Arc::clone(&owner);
        let erased: Arc<dyn Any + Send + Sync> = erased;
        owners.insert(self.prefix.clone(), Arc::downgrade(&erased));
        drop(owners);
        Ok(owner)
    }

    /// Removes one table from this namespace.
    ///
    /// # Errors
    ///
    /// Returns an error when table deletion or commit fails.
    pub async fn remove_table(&self, name: &str) -> io::Result<()> {
        let _admission = self.host.admission.lock().await;
        self.remove_table_admitted(name)
    }

    /// Removes one table only if writer admission is immediately available.
    ///
    /// # Errors
    ///
    /// Returns an error when table deletion or commit fails.
    pub fn try_remove_table(&self, name: &str) -> io::Result<bool> {
        let Some(_admission) = self.host.admission.try_lock() else {
            return Ok(false);
        };
        self.remove_table_admitted(name)?;
        Ok(true)
    }

    fn remove_table_admitted(&self, name: &str) -> io::Result<()> {
        let transaction = self.host.database.begin_write().map_err(io::Error::other)?;
        transaction
            .delete_table(table_definition::<Bytes, Vec<u8>>(&self.table(name)))
            .map_err(io::Error::other)?;
        transaction.commit().map_err(io::Error::other)
    }

    /// Low-level cleanup. Fence and drain resource owners before calling this method.
    ///
    /// # Errors
    ///
    /// Returns an error when retirement, table deletion, or commit fails.
    pub async fn retire(&self) -> io::Result<()> {
        let _admission = self.host.admission.lock().await;
        let transaction = self.host.database.begin_write().map_err(io::Error::other)?;
        transaction
            .open_table(RETIRED)
            .map_err(io::Error::other)?
            .insert(self.prefix.as_str(), true)
            .map_err(io::Error::other)?;
        let tables = transaction
            .list_tables()
            .map_err(io::Error::other)?
            .filter(|table| table.name().starts_with(&self.prefix))
            .collect::<Vec<_>>();
        for table in tables {
            transaction.delete_table(table).map_err(io::Error::other)?;
        }
        transaction.commit().map_err(io::Error::other)
    }
}
