use std::{future::ready, io, sync::Arc};

use of_storage_client::{
    handle::{KvStoreHandle, SqlDatabaseHandle},
    protocol::StoreIdentity,
    transport::{TokenProvider, TransportConfig},
};
use ofdb_sql::history::{SessionConfig, SyncRole};
use tokio::sync::mpsc;
use uuid::Uuid;

struct Stream {
    receive: mpsc::Receiver<Vec<u8>>,
    send: mpsc::Sender<Vec<u8>>,
}
impl ofdb_sql::history::SyncTransport for Stream {
    type Error = io::Error;
    async fn receive(&mut self) -> io::Result<Vec<u8>> {
        self.receive
            .recv()
            .await
            .ok_or_else(|| io::Error::other("closed"))
    }
    async fn send(&mut self, frame: Vec<u8>) -> io::Result<()> {
        self.send.send(frame).await.map_err(io::Error::other)
    }
}
impl ofdb_kv::history::SyncTransport for Stream {
    type Error = io::Error;
    async fn receive(&mut self) -> io::Result<Vec<u8>> {
        ofdb_sql::history::SyncTransport::receive(self).await
    }
    async fn send(&mut self, frame: Vec<u8>) -> io::Result<()> {
        ofdb_sql::history::SyncTransport::send(self, frame).await
    }
}
fn pair() -> (Stream, Stream) {
    let (a, b) = mpsc::channel(8);
    let (c, d) = mpsc::channel(8);
    (
        Stream {
            receive: d,
            send: a,
        },
        Stream {
            receive: b,
            send: c,
        },
    )
}
fn identity() -> StoreIdentity {
    StoreIdentity {
        store_id: Uuid::now_v7(),
        replica_id: Uuid::now_v7(),
        creation_epoch: 0,
        schema_fingerprint: [0; 32],
    }
}
fn token() -> TokenProvider {
    Arc::new(|| Box::pin(async { Ok("resource-token".into()) }))
}
fn sql() -> SqlDatabaseHandle {
    let identity = identity();
    SqlDatabaseHandle::new(
        ofdb_sql::Database::in_memory(),
        identity.clone(),
        TransportConfig::new("ws://unused".into()),
        token(),
    )
}

#[tokio::test]
async fn checked_sql_exchange_preserves_rows_and_rejects_revoked_apply() {
    let source = sql();
    source
        .execute("CREATE TABLE records (id TEXT PRIMARY KEY, value TEXT)")
        .await
        .expect("create table");
    source
        .execute("INSERT INTO records VALUES ('one', 'kept')")
        .await
        .expect("insert row");
    let target = sql();
    let config = SessionConfig::default();
    let directory = tempfile::tempdir().expect("staging directory");
    let (mut a, mut b) = pair();
    let (sent, received) = tokio::join!(
        source.synchronize_with(
            &mut a,
            &config,
            SyncRole::Initiator,
            directory.path(),
            || ready(Ok(()))
        ),
        target.synchronize_with(
            &mut b,
            &config,
            SyncRole::Responder,
            directory.path(),
            || ready(Ok(()))
        ),
    );
    sent.expect("source exchange");
    received.expect("target exchange");
    let before = target
        .execute("SELECT * FROM records")
        .await
        .expect("read received row");
    assert_eq!(before.rows.len(), 1);
    source
        .execute("INSERT INTO records VALUES ('two', 'denied')")
        .await
        .expect("insert denied row");
    let (mut a, mut b) = pair();
    let denied = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        tokio::select! {
            result = source.synchronize_with(&mut a, &config, SyncRole::Initiator, directory.path(), || ready(Ok(()))) => result.map(|_| ()),
            result = target.synchronize_with(&mut b, &config, SyncRole::Responder, directory.path(), || ready(Err("revoked".into()))) => result.map(|_| ()),
        }
    }).await.expect("revocation finishes");
    assert!(denied.is_err());
    assert_eq!(
        target
            .execute("SELECT * FROM records")
            .await
            .expect("read after denial")
            .rows
            .len(),
        1
    );
}

#[tokio::test]
async fn checked_kv_exchange_uses_current_owner_protocol() {
    let identity = identity();
    let source = KvStoreHandle::new(ofdb_kv::Database::in_memory(), identity);
    source
        .database()
        .client()
        .set("one", ofdb_sql::Value::Text("kept".into()), None)
        .await
        .expect("set value");
    let target = ofdb_kv::Database::in_memory();
    let (mut a, mut b) = pair();
    let (sent, received) = tokio::join!(
        source.synchronize_with(
            &mut a,
            ofdb_kv::history::SyncRole::Initiator,
            ofdb_kv::history::Config::default(),
            || ready(Ok(()))
        ),
        target.exchange_history(
            &mut b,
            ofdb_kv::history::SyncRole::Responder,
            ofdb_kv::history::Config::default(),
            || ready(Ok(()))
        ),
    );
    sent.expect("source KV exchange");
    received.expect("target KV exchange");
    assert_eq!(
        target
            .client()
            .get("one")
            .await
            .expect("get received value"),
        Some(ofdb_sql::Value::Text("kept".into()))
    );
}
