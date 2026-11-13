#![cfg(feature = "network")]

use std::{net::TcpListener, time::Duration};

use embedded_db::{RemoteDatabase, RemoteDatabaseConfig};
use of_model::db::Database;

#[tokio::test]
async fn network_database_rejects_unsafe_configuration_before_connecting() {
    let mut config = RemoteDatabaseConfig {
        endpoint: "http://sql.example:50051".into(),
        ca_certificate: None,
        client_certificate: None,
        client_key: None,
        deadline_ms: 1000,
    };
    for endpoint in [
        "http://sql.example:50051",
        "https://sql.example:50051",
        "http://user:secret@127.0.0.1:50051",
        "http://127.0.0.1:50051/path",
    ] {
        config.endpoint = endpoint.into();
        let error = match RemoteDatabase::connect(&config).await {
            Ok(_) => panic!("reject unsafe SQL endpoint"),
            Err(error) => error,
        };
        assert_eq!(error.kind, db::QueryErrorKind::Validation);
    }
    config.endpoint = "http://127.0.0.1:50051".into();
    config.deadline_ms = 0;
    assert!(RemoteDatabase::connect(&config).await.is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn network_database_migrates_and_shares_real_remote_transactions() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("reserve loopback port");
    let address = listener.local_addr().expect("loopback address");
    drop(listener);
    let hosted = db::Database::in_memory();
    let server_database = hosted.clone();
    let (shutdown, signal) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        server_database
            .serve_tcp(address, async {
                let _ = signal.await;
            })
            .await
            .expect("serve SQL");
    });
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if tokio::net::TcpStream::connect(address).await.is_ok() {
                break;
            }
            assert!(!server.is_finished(), "SQL server stopped before readiness");
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("SQL server readiness deadline");

    let config = RemoteDatabaseConfig {
        endpoint: format!("http://{address}"),
        ca_certificate: None,
        client_certificate: None,
        client_key: None,
        deadline_ms: 5000,
    };
    let database = RemoteDatabase::connect(&config)
        .await
        .expect("connect and migrate remote SQL");
    let shared: &dyn Database = &database;
    shared.idp().client().execute_sql("INSERT INTO users (id, handle) VALUES (CAST('00000000-0000-7000-8000-000000000001' AS UUID), 'remote')", None).await.expect("remote owner write");
    let mut transaction = shared.txn().await.expect("remote transaction");
    transaction
        .execute_sql("UPDATE users SET handle = 'rolled-back'", None)
        .await
        .expect("remote update");
    transaction.rollback().await.expect("remote rollback");
    let result = shared
        .device()
        .client()
        .execute_sql("SELECT handle FROM users", None)
        .await
        .expect("remote owner read");
    assert_eq!(
        result[0].rows,
        vec![db::Row::new(vec![db::Value::Text("remote".into())])]
    );
    shared
        .storage()
        .client()
        .execute_sql("SELECT * FROM resources LIMIT 0", None)
        .await
        .expect("remote Storage schema");
    shared
        .management()
        .client()
        .execute_sql("SELECT * FROM sync_selections LIMIT 0", None)
        .await
        .expect("remote Management schema");
    let result = hosted
        .client()
        .execute_sql("SELECT handle FROM users", None)
        .await
        .expect("read server state");
    assert_eq!(
        result[0].rows,
        vec![db::Row::new(vec![db::Value::Text("remote".into())])]
    );
    let mut transaction = shared.txn().await.expect("remote commit transaction");
    transaction
        .execute_sql("UPDATE users SET handle = 'committed'", None)
        .await
        .expect("remote committed update");
    transaction.commit().await.expect("remote commit");
    let reconnected = RemoteDatabase::connect(&config)
        .await
        .expect("repeat remote migration");
    let result = reconnected
        .management()
        .client()
        .execute_sql("SELECT handle FROM users", None)
        .await
        .expect("read remote commit after reconnect");
    assert_eq!(
        result[0].rows,
        vec![db::Row::new(vec![db::Value::Text("committed".into())])]
    );
    drop(reconnected);
    drop(database);
    shutdown.send(()).expect("request SQL shutdown");
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .expect("SQL shutdown deadline")
        .expect("SQL server task");
}
