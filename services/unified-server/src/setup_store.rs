use std::{io, str, thread};

use db::{Client, Database, EmbeddedDatabase, QueryParams, Value};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use setup::{SetupAdmissionHistory, join::JoinJournalStore};
use tokio::runtime::Builder;

#[derive(Clone)]
pub struct SetupStore(Client);

impl SetupStore {
    pub async fn open(database: &EmbeddedDatabase) -> io::Result<Self> {
        let client = Database::open_shared(&database.storage(), "node-setup")
            .map_err(io::Error::other)?
            .client();
        client
            .execute_sql(
                "CREATE TABLE IF NOT EXISTS node_setup (name TEXT PRIMARY KEY, payload TEXT NOT NULL)",
                None,
            )
            .await
            .map_err(io::Error::other)?;
        Ok(Self(client))
    }

    pub async fn read<T: DeserializeOwned>(&self, name: &str) -> io::Result<Option<T>> {
        let rows = self
            .0
            .execute_sql(
                "SELECT payload FROM node_setup WHERE name = $1",
                Some(&QueryParams::Positional(vec![Value::Text(name.into())])),
            )
            .await
            .map_err(io::Error::other)?;
        match rows[0].rows.as_slice() {
            [] => Ok(None),
            [row] => match row.values.as_slice() {
                [Value::Text(payload)] => Ok(Some(serde_json::from_str(payload)?)),
                _ => Err(io::Error::other("invalid Node setup record")),
            },
            _ => Err(io::Error::other("duplicate Node setup record")),
        }
    }

    pub async fn write<T: Serialize + Sync + ?Sized>(
        &self,
        name: &str,
        value: &T,
    ) -> io::Result<()> {
        self.0.execute_sql(
            "INSERT INTO node_setup (name, payload) VALUES ($1, $2) ON CONFLICT (name) DO UPDATE SET payload = $2",
            Some(&QueryParams::Positional(vec![Value::Text(name.into()), Value::Text(serde_json::to_string(value)?)])),
        ).await.map_err(io::Error::other)?;
        Ok(())
    }
}

fn sync<T: Send>(future: impl Future<Output = io::Result<T>> + Send) -> io::Result<T> {
    thread::scope(|scope| {
        // Use the same stack as the Node's workers for embedded SQL.
        thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn_scoped(scope, || {
                Builder::new_current_thread().build()?.block_on(future)
            })?
            .join()
            .map_err(|_panic| io::Error::other("Node setup persistence worker panicked"))?
    })
}

#[derive(Default, Deserialize, Serialize)]
struct Admission {
    intent: String,
    retired: bool,
}

impl SetupAdmissionHistory for SetupStore {
    fn read(&self) -> io::Result<(String, bool)> {
        let record: Admission = sync(self.read("admission"))?.unwrap_or_default();
        Ok((record.intent, record.retired))
    }

    fn bind(&self, intent: &str) -> io::Result<()> {
        let (current, retired) = <Self as SetupAdmissionHistory>::read(self)?;
        if retired || (!current.is_empty() && current != intent) {
            return Err(io::Error::other("Node setup choice changed"));
        }
        sync(self.write(
            "admission",
            &Admission {
                intent: intent.into(),
                retired: false,
            },
        ))
    }

    fn retire(&self, intent: &str) -> io::Result<()> {
        let (current, _) = <Self as SetupAdmissionHistory>::read(self)?;
        if current != intent {
            return Err(io::Error::other("Node setup choice changed"));
        }
        sync(self.write(
            "admission",
            &Admission {
                intent: intent.into(),
                retired: true,
            },
        ))
    }
}

impl JoinJournalStore for SetupStore {
    fn load(&self) -> io::Result<Option<Vec<u8>>> {
        sync(self.read::<String>("join")).map(|value| value.map(String::into_bytes))
    }

    fn save(&self, journal: &[u8]) -> io::Result<()> {
        sync(self.write("join", str::from_utf8(journal).map_err(io::Error::other)?))
    }
}
