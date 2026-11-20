use ofdb_kv::{Client, Database};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::{net::TcpListener, sync::oneshot, time::sleep};
use value::Value;

#[tokio::test]
#[ignore = "remote gRPC transaction integration test"]
#[allow(
    clippy::too_many_lines,
    reason = "one fixture covers commit, rollback, and typed values"
)]
#[allow(
    clippy::significant_drop_tightening,
    reason = "test transactions are consumed by commit or rollback"
)]
async fn remote_client_transaction_commits_and_rolls_back() {
    let database = Arc::new(Database::in_memory());
    let address = SocketAddr::from(([127, 0, 0, 1], 0));
    let listener = TcpListener::bind(address)
        .await
        .expect("bind test listener");
    let address = listener.local_addr().expect("read test address");
    drop(listener);
    let (shutdown, stopped) = oneshot::channel::<()>();
    let server_database = Arc::clone(&database);
    let server = tokio::spawn(async move {
        server_database
            .serve_tcp(address, async move {
                let _ = stopped.await;
            })
            .await
            .expect("serve test database");
    });

    let endpoint = format!("http://{address}");
    let mut connected = None;
    for _ in 0..50 {
        match Client::connect(&endpoint).await {
            Ok(client) => {
                connected = Some(client);
                break;
            }
            Err(_) => sleep(Duration::from_millis(10)).await,
        }
    }
    let client = connected.expect("connect remote client");
    let mut transaction = client
        .transaction()
        .await
        .expect("start remote transaction");
    transaction
        .set("committed", Value::Blob(vec![1]), None)
        .await
        .expect("set committed value");
    assert_eq!(
        transaction
            .get("committed")
            .await
            .expect("read in transaction"),
        Some(Value::Blob(vec![1]))
    );
    transaction
        .commit()
        .await
        .expect("commit remote transaction");
    assert_eq!(
        client.get("committed").await.expect("read committed value"),
        Some(Value::Blob(vec![1]))
    );

    let mut transaction = client
        .transaction()
        .await
        .expect("start rollback transaction");
    transaction
        .set("rolled-back", Value::Blob(vec![2]), None)
        .await
        .expect("set rolled-back value");
    transaction
        .rollback()
        .await
        .expect("rollback remote transaction");
    assert_eq!(
        client
            .get("rolled-back")
            .await
            .expect("read rolled-back key"),
        None
    );

    let values = [
        Value::Null,
        Value::Type(ofdb_kv::ValueType::Text),
        Value::Uuid(
            "00000000-0000-0000-0000-000000000000"
                .parse()
                .expect("valid UUID"),
        ),
        Value::Bool(true),
        Value::Integer(i64::MIN),
        Value::Float(-0.0),
        Value::Float(f64::from_bits(0x7ff8_0000_0000_0042)),
        Value::Text("text".into()),
        Value::Blob(vec![0, 255]),
        Value::Json(ofdb_kv::JsonValue::Array(vec![
            ofdb_kv::JsonValue::Number(ofdb_kv::JsonNumber::U64(u64::MAX)),
            ofdb_kv::JsonValue::Number(ofdb_kv::JsonNumber::F64(f64::from_bits(
                0x7ff8_0000_0000_0042,
            ))),
        ])),
    ];
    for (index, value) in values.iter().enumerate() {
        let key = format!("typed:{index:02}");
        client
            .set(&key, value.clone(), None)
            .await
            .expect("set typed remote value");
        let actual = client
            .get(&key)
            .await
            .expect("get typed remote value")
            .expect("typed value exists");
        assert_eq!(
            ofdb_kv::encode_value(actual),
            ofdb_kv::encode_value(value.clone())
        );
    }
    let expected = values
        .into_iter()
        .map(ofdb_kv::encode_value)
        .collect::<Vec<_>>();
    for entries in [
        client.scan("typed:", "typed;").await.expect("typed range"),
        client.scan_prefix("typed:").await.expect("typed prefix"),
        client
            .scan_all()
            .await
            .expect("typed scan all")
            .into_iter()
            .filter(|(key, _)| key.starts_with("typed:"))
            .collect(),
    ] {
        assert_eq!(
            entries
                .into_iter()
                .map(|(_, value)| ofdb_kv::encode_value(value))
                .collect::<Vec<_>>(),
            expected
        );
    }
    let mut transaction = client.transaction().await.expect("start typed transaction");
    transaction
        .set("typed:00", Value::Text("transaction edit".into()), None)
        .await
        .expect("edit typed value in stream");
    assert_eq!(
        transaction.get("typed:00").await.expect("stream read"),
        Some(Value::Text("transaction edit".into()))
    );
    for entries in [
        transaction
            .scan("typed:", "typed;")
            .await
            .expect("stream range"),
        transaction
            .scan_prefix("typed:")
            .await
            .expect("stream prefix"),
        transaction
            .scan_all()
            .await
            .expect("stream all")
            .into_iter()
            .filter(|(key, _)| key.starts_with("typed:"))
            .collect(),
    ] {
        assert_eq!(entries.len(), expected.len());
        assert_eq!(entries[0].1, Value::Text("transaction edit".into()));
    }
    transaction.commit().await.expect("commit typed stream");
    let _ = shutdown.send(());
    server.await.expect("join server task");
}
