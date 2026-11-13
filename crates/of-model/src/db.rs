use core::{future::Future, pin::Pin};
use std::boxed::Box;

pub use db::{Client, QueryError, Transaction};

pub trait IdpDatabase: Send + Sync {
    fn client(&self) -> Client;
}

pub trait DeviceDatabase: Send + Sync {
    fn client(&self) -> Client;
}

pub trait StorageDatabase: Send + Sync {
    fn client(&self) -> Client;
}

pub trait ManagementDatabase: Send + Sync {
    fn client(&self) -> Client;
}

/// Query access is shared; hosting and replication stay with the database owner.
pub trait Database: Send + Sync {
    fn client(&self) -> Client;
    fn idp(&self) -> &dyn IdpDatabase;
    fn device(&self) -> &dyn DeviceDatabase;
    fn storage(&self) -> &dyn StorageDatabase;
    fn management(&self) -> &dyn ManagementDatabase;

    fn txn(&self) -> Pin<Box<dyn Future<Output = Result<Transaction, QueryError>> + Send + '_>> {
        let client = self.client();
        Box::pin(async move { client.transaction().await })
    }
}

macro_rules! database {
    ($type:ty, $client:expr) => {
        impl IdpDatabase for $type {
            fn client(&self) -> Client {
                ($client)(self)
            }
        }
        impl DeviceDatabase for $type {
            fn client(&self) -> Client {
                ($client)(self)
            }
        }
        impl StorageDatabase for $type {
            fn client(&self) -> Client {
                ($client)(self)
            }
        }
        impl ManagementDatabase for $type {
            fn client(&self) -> Client {
                ($client)(self)
            }
        }
        impl Database for $type {
            fn client(&self) -> Client {
                ($client)(self)
            }
            fn idp(&self) -> &dyn IdpDatabase {
                self
            }
            fn device(&self) -> &dyn DeviceDatabase {
                self
            }
            fn storage(&self) -> &dyn StorageDatabase {
                self
            }
            fn management(&self) -> &dyn ManagementDatabase {
                self
            }
        }
    };
}

database!(db::Database, db::Database::client);
database!(Client, Client::clone);
