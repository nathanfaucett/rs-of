use std::{io, path::{Path, PathBuf}, sync::Mutex};

use idp_server::local_provisioning::{create_private_directory, private_options, require_absent, require_file, sync_ancestry};
use ofdb_sql::{Database, Value};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value as Json, json};

use crate::UnifiedConfig;

// ponytail: host metadata is low-volume; serialize synchronous calls until async lifecycle APIs are needed.
static ACCESS: Mutex<()> = Mutex::new(());

#[derive(Clone)]
pub(crate) struct HostStore { path: PathBuf }

impl HostStore {
    pub(crate) fn create(root: &Path) -> io::Result<Self> {
        let path = root.join("host.redb");
        require_absent(&path)?;
        private_options(true).open(&path)?.sync_all()?;
        let store = Self { path };
        store.run_inner(true, |database| async move {
            let client = database.client();
            let mut tx = client.transaction().await.map_err(error)?;
            tx.execute_sql("CREATE TABLE host_state (id UUID PRIMARY KEY, payload TEXT NOT NULL)", None).await.map_err(error)?;
            let params = ofdb_sql::QueryParams::Positional(vec![
                Value::Uuid(idp_model::model::Id::now_v7()),
                Value::Text(json!({"version": 1, "records": {}}).to_string()),
            ]);
            tx.execute_sql("INSERT INTO host_state (id, payload) VALUES (?, ?)", Some(&params)).await.map_err(error)?;
            tx.commit().await.map_err(error)
        })?;
        sync_ancestry(root)?;
        Ok(store)
    }

    pub(crate) fn open(root: &Path) -> io::Result<Self> {
        for legacy in ["host.json", "host.pending"] { require_absent(&root.join(legacy)).map_err(|_| invalid("unsupported legacy host layout; explicit reset or restore required"))?; }
        let store = Self { path: root.join("host.redb") };
        require_file(&store.path, true)?;
        store.update(|_| Ok(()))?;
        Ok(store)
    }

    pub(crate) fn for_config(config: &UnifiedConfig) -> io::Result<Self> {
        let explicit = crate::BootstrapConfig::explicit_directory(config);
        match std::fs::symlink_metadata(&explicit) {
            Ok(_) => return Self::open(&explicit),
            Err(error) if error.kind() == io::ErrorKind::NotFound => (),
            Err(error) => return Err(error),
        }
        let parent = config.endpoint_data_dir.parent().ok_or_else(|| invalid("missing host root"))?;
        Self::open(parent)
    }

    pub(crate) fn create_for_config(config: &UnifiedConfig) -> io::Result<Self> {
        let explicit = crate::BootstrapConfig::explicit_directory(config);
        let parent = config.endpoint_data_dir.parent().ok_or_else(|| invalid("missing host root"))?;
        if explicit.exists() || parent.join("host.redb").exists() || parent.join("host.json").exists() { return Self::for_config(config); }
        create_private_directory(&explicit, false)?;
        Self::create(&explicit)
    }

    pub(crate) fn get<T: DeserializeOwned + Send>(&self, name: &str) -> io::Result<Option<T>> {
        self.update(|records| records.get(name).cloned().map(serde_json::from_value).transpose().map_err(Into::into))
    }

    pub(crate) fn put<T: Serialize + Sync>(&self, name: &str, value: &T) -> io::Result<()> {
        self.update(|records| { records.insert(name.into(), serde_json::to_value(value)?); Ok(()) })
    }

    pub(crate) fn update<T: Send>(&self, operation: impl FnOnce(&mut serde_json::Map<String, Json>) -> io::Result<T> + Send) -> io::Result<T> {
        self.run(|database| async move {
            let client = database.client();
            let mut tx = client.transaction().await.map_err(error)?;
            let results = tx.execute_sql("SELECT payload FROM host_state", None).await.map_err(error)?;
            let rows = &results.first().ok_or_else(|| invalid("missing host schema"))?.rows;
            let [row] = rows.as_slice() else { return Err(invalid("invalid host state cardinality")); };
            let [Value::Text(payload)] = row.values.as_slice() else { return Err(invalid("invalid host state")); };
            let mut state: Json = serde_json::from_str(payload)?;
            if state["version"] != 1 || state.as_object().is_none_or(|object| object.len() != 2) { return Err(invalid("unsupported host schema; explicit recovery required")); }
            let records = state["records"].as_object_mut().ok_or_else(|| invalid("missing host records"))?;
            let result = operation(records)?;
            let updated = serde_json::to_string(&state)?;
            if updated != *payload {
                let params = ofdb_sql::QueryParams::Positional(vec![Value::Text(updated)]);
                tx.execute_sql("UPDATE host_state SET payload = ?", Some(&params)).await.map_err(error)?;
                tx.commit().await.map_err(error)?;
            }
            Ok(result)
        })
    }

    fn run<T: Send, F: Future<Output = io::Result<T>>>(&self, operation: impl FnOnce(Database) -> F + Send) -> io::Result<T> {
        self.run_inner(false, operation)
    }

    fn run_inner<T: Send, F: Future<Output = io::Result<T>>>(&self, fresh: bool, operation: impl FnOnce(Database) -> F + Send) -> io::Result<T> {
        let _guard = ACCESS.lock().map_err(|_| invalid("host store lock poisoned"))?;
        require_file(&self.path, true)?;
        if !fresh && std::fs::metadata(&self.path)?.len() == 0 {
            return Err(invalid("empty retained host database; explicit recovery required"));
        }
        std::thread::scope(|scope| scope.spawn(|| {
            let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
            let database = Database::open(&self.path).map_err(error)?;
            runtime.block_on(operation(database))
        }).join().map_err(|_| invalid("host SQL task failed"))?)
    }
}

fn error(error: impl std::fmt::Display) -> io::Error { io::Error::other(error.to_string()) }
fn invalid(message: &str) -> io::Error { io::Error::new(io::ErrorKind::InvalidData, message) }

impl api::SetupAdmissionHistory for HostStore {
    fn read(&self) -> io::Result<(String, bool)> {
        let value: Json = self.get("admission")?.ok_or_else(|| invalid("missing admission creation evidence"))?;
        let intent: String = self.get("operation")?.unwrap_or_default();
        let retired = value["retired"].as_bool().ok_or_else(|| invalid("invalid retirement"))?;
        Ok((intent, retired))
    }
    fn bind(&self, intent: &str) -> io::Result<()> {
        self.update(|records| {
            let admission = records.get("admission").ok_or_else(|| invalid("missing admission"))?;
            if admission["retired"] != false || records.get("operation").is_some_and(|bound| bound != intent) { return Err(invalid("conflicting or retired operation")); }
            records.insert("operation".into(), json!(intent));
            Ok(())
        })
    }
    fn retire(&self, intent: &str) -> io::Result<()> {
        self.update(|records| {
            let admission = records.get("admission").ok_or_else(|| invalid("missing admission"))?;
            if admission["retired"] != false || records.get("operation") != Some(&json!(intent)) { return Err(invalid("conflicting retirement")); }
            if let Some(create) = records.get_mut("create") {
                if create["storage_initialized"] != true { return Err(invalid("Create owners have not acknowledged initialization")); }
                create["complete"] = json!(true);
            }
            records.get_mut("admission").expect("validated admission")["retired"] = json!(true);
            Ok(())
        })
    }
}
