use core::slice::from_ref;
use std::{
    net::{SocketAddr, TcpListener},
    time::Duration,
};

use tokio::{net::TcpStream, sync::oneshot, time::sleep};

use ofdb_sql::{
    Client, Database, FromRow, FromRowError, Query, QueryColumn, QueryErrorKind, QueryFrom,
    QueryParams, QuerySelect, Row, SqlTranslator, Statement, Uuid, Value,
};

struct IdRow(Uuid);

impl FromRow for IdRow {
    fn from_row(row: &Row, columns: &[&str]) -> Result<Self, FromRowError> {
        let id = ofdb_sql::decode::<Uuid>(ofdb_sql::value(row, columns, "id")?, "id")?;
        Ok(Self(id))
    }
}

fn free_address() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind temporary listener");
    listener.local_addr().expect("read temporary address")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[allow(
    clippy::too_many_lines,
    reason = "single end-to-end remote client fixture"
)]
#[allow(
    clippy::significant_drop_tightening,
    reason = "rollback consumes the transaction"
)]
async fn remote_client_executes_typed_queries_over_tcp() {
    let address = free_address();
    let database = Database::in_memory();
    let database_client = database.client();
    database_client
        .execute_sql("CREATE TABLE items (id UUID PRIMARY KEY)", None)
        .await
        .expect("create table in embedded database");

    let hosted_database = database.clone();
    let (shutdown, signal) = oneshot::channel();
    let server = tokio::spawn(async move {
        hosted_database
            .serve_tcp(address, async {
                let _ = signal.await;
            })
            .await
            .expect("server should stop cleanly");
    });

    loop {
        if TcpStream::connect(address).await.is_ok() {
            break;
        }
        sleep(Duration::from_millis(5)).await;
    }

    let client = Client::connect(format!("http://{address}"))
        .await
        .expect("connect eagerly to running server");
    let results = client
        .execute(vec![Statement::Query(Query::Select(QuerySelect {
            from: QueryFrom {
                table: "items".into(),
                joins: Vec::new(),
            },
            projection: vec![QueryColumn {
                table: "items".into(),
                column: "id".into(),
            }],
            ..QuerySelect::default()
        }))])
        .await
        .expect("execute query remotely");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].rows, [] as [ofdb_sql::Row; 0]);

    let id = Uuid::parse_str("018f0f8e-7b6d-7c4a-8f12-123456789abc").expect("valid test UUID");

    let key = Row::new(vec![Value::Uuid(id)]);
    assert_eq!(
        client
            .row_conflicts("items", &key)
            .await
            .expect("remote conflict inspection"),
        [] as [String; 0]
    );
    for keys in [from_ref(&key), &[]] {
        assert_eq!(
            client
                .row_conflicts_batch("items", keys)
                .await
                .expect("remote batch conflicts"),
            database_client
                .row_conflicts_batch("items", keys)
                .await
                .expect("local batch conflicts")
        );
    }
    assert_eq!(
        client
            .row_conflict_values("items", &key)
            .await
            .expect("remote conflict values"),
        [] as [(String, Vec<ofdb_sql::Value>); 0]
    );

    let mut transaction = client.transaction().await.expect("remote transaction");
    assert_eq!(
        transaction
            .row_conflicts("items", &key)
            .await
            .expect("transaction conflicts"),
        [] as [String; 0]
    );
    assert_eq!(
        transaction
            .row_conflict_values("items", &key)
            .await
            .expect("transaction values"),
        [] as [(String, Vec<ofdb_sql::Value>); 0]
    );
    transaction
        .rollback()
        .await
        .expect("inspections did not close transaction");

    let params = QueryParams::Positional(vec![Value::Uuid(id)]);
    let error = client
        .execute_sql("INSERT INTO items VALUES ($1)", Some(&params))
        .await
        .expect_err("reject a bound UUID primary key remotely");
    assert_eq!(error.kind, QueryErrorKind::Rejected);
    assert!(error.message.contains("UUID primary keys are generated"));
    let mut transaction = client.transaction().await.expect("begin remote write");
    let error = transaction
        .execute_sql("INSERT INTO items VALUES ($1)", Some(&params))
        .await
        .expect_err("reject a bound UUID primary key in the remote transaction");
    assert_eq!(error.kind, QueryErrorKind::Rejected);
    assert!(error.message.contains("UUID primary keys are generated"));
    transaction
        .rollback()
        .await
        .expect("discard rejected write");

    let inserted = client
        .execute_sql("INSERT INTO items VALUES (DEFAULT) RETURNING id", None)
        .await
        .expect("generate the remote primary key");
    let generated = inserted[0].rows[0].values[0]
        .as_uuid()
        .expect("generated UUID");
    assert_eq!(generated.get_version_num(), 7);
    let typed = client
        .translate_and_select::<_, IdRow>("SELECT id FROM items", &SqlTranslator)
        .await
        .expect("decode typed remote row");
    assert_eq!(typed.len(), 1);
    assert_eq!(&typed[0].0, generated);
    assert!(
        client
            .translate_and_select::<_, IdRow>(
                "SELECT id FROM items; SELECT id FROM items",
                &SqlTranslator,
            )
            .await
            .is_err()
    );
    client
        .translate_and_execute("INVALID SQL", &SqlTranslator)
        .await
        .unwrap_err();

    let empty_error = client
        .execute(Vec::new())
        .await
        .expect_err("reject empty remote batches");
    assert_eq!(empty_error.kind, QueryErrorKind::Validation);

    client
        .execute_sql("CREATE TABLE atomic_items (id UUID PRIMARY KEY)", None)
        .await
        .expect("create batch rollback table");
    let invalid_batch = "INSERT INTO atomic_items VALUES (DEFAULT); INSERT INTO atomic_items VALUES (CAST('018f0f8e-7b6d-7c4a-8f12-123456789abc' AS UUID))";
    let batch_error = client
        .execute_sql(invalid_batch, None)
        .await
        .expect_err("reject supplied key in batch");
    let local_error = database_client
        .execute_sql(invalid_batch, None)
        .await
        .expect_err("reject supplied key in local batch");
    assert_eq!(batch_error.kind, QueryErrorKind::Rejected);
    assert_eq!(batch_error.kind, local_error.kind);
    let rows = client
        .execute_sql("SELECT id FROM atomic_items", None)
        .await
        .expect("read after failed batch");
    assert_eq!(rows[0].rows, [] as [ofdb_sql::Row; 0]);
    client.execute_sql("BEGIN", None).await.unwrap_err();
    client.execute_sql("COMMIT", None).await.unwrap_err();
    client.execute_sql("ROLLBACK", None).await.unwrap_err();

    client
        .execute_sql("CREATE TABLE transaction_items (id UUID PRIMARY KEY)", None)
        .await
        .expect("create transaction table");
    let mut transaction = client
        .transaction()
        .await
        .expect("begin remote transaction");
    transaction
        .execute_sql("INSERT INTO transaction_items VALUES (DEFAULT)", None)
        .await
        .expect("write in remote transaction");
    let rows = transaction
        .translate_and_execute("SELECT id FROM transaction_items", &SqlTranslator)
        .await
        .expect("read remote transaction write");
    assert_eq!(rows[0].rows.len(), 1);
    transaction
        .rollback()
        .await
        .expect("rollback remote transaction");
    let rows = client
        .execute_sql("SELECT id FROM transaction_items", None)
        .await
        .expect("read after remote rollback");
    assert_eq!(rows[0].rows, [] as [ofdb_sql::Row; 0]);

    let mut transaction = client
        .transaction()
        .await
        .expect("begin remote transaction");
    transaction
        .translate_and_execute(
            "INSERT INTO transaction_items VALUES (DEFAULT)",
            &SqlTranslator,
        )
        .await
        .expect("write before remote commit");
    transaction
        .commit()
        .await
        .expect("commit remote transaction");
    let rows = client
        .execute_sql("SELECT id FROM transaction_items", None)
        .await
        .expect("read after remote commit");
    assert_eq!(rows[0].rows.len(), 1);

    shutdown.send(()).expect("server is awaiting shutdown");
    server.await.expect("server task should complete");
}

#[tokio::test]
async fn sql_mtls_requires_and_accepts_client_identity() {
    let address = free_address();
    let database = Database::in_memory();
    let (shutdown, signal) = oneshot::channel();
    let server = tokio::spawn(async move {
        database
            .serve_tcp_with_mtls(
                address,
                Some((
                    include_bytes!("../../test-fixtures/sql-mtls/server.crt").to_vec(),
                    include_bytes!("../../test-fixtures/sql-mtls/server.key").to_vec(),
                )),
                Some(include_bytes!("../../test-fixtures/sql-mtls/ca.crt").to_vec()),
                async {
                    let _ = signal.await;
                },
            )
            .await
            .expect("mTLS server stops cleanly");
    });

    let endpoint = format!("https://localhost:{}", address.port());
    let missing_identity_client = Client::connect_with_ca(
        &endpoint,
        include_bytes!("../../test-fixtures/sql-mtls/ca.crt"),
    )
    .await
    .expect("TLS channel can be created before the handshake completes");
    assert!(
        missing_identity_client
            .execute_sql("CREATE TABLE unauthorized (id UUID PRIMARY KEY)", None)
            .await
            .is_err(),
        "server rejects RPCs from clients without a certificate"
    );

    let client = Client::connect_with_identity(
        &endpoint,
        include_bytes!("../../test-fixtures/sql-mtls/ca.crt"),
        include_bytes!("../../test-fixtures/sql-mtls/client.crt"),
        include_bytes!("../../test-fixtures/sql-mtls/client.key"),
    )
    .await
    .expect("authorized client connects with mTLS");
    client
        .execute_sql("CREATE TABLE mtls_items (id UUID PRIMARY KEY)", None)
        .await
        .expect("authorized client executes query");

    shutdown.send(()).expect("server is awaiting shutdown");
    server.await.expect("server task should complete");
}
