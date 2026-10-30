use model::contract::StorageAuthorizationAction::{Read, Write};
use serde_json::{Value, json};

use crate::support::native::Fixture;

async fn ensure(f: &Fixture, token: &str, key: &str) -> reqwest::Response {
    f.http
        .post(format!(
            "{}/storage/databases/ensure",
            f.storage_listener.base
        ))
        .bearer_auth(token)
        .json(&json!({"creationKey": key, "name": "Label"}))
        .send()
        .await
        .expect("authenticated ensure")
}

#[tokio::test]
async fn explicit_creation_retries_converge_and_deletion_consumes_identity() {
    let mut f = Fixture::new(Default::default()).await;
    let token = f.token("storage:databases", vec![Read, Write]).await;
    let (first, retry) = tokio::join!(
        ensure(&f, &token, "logical-one"),
        ensure(&f, &token, "logical-one")
    );
    assert_eq!(first.status(), 200);
    assert_eq!(retry.status(), 200);
    let first: Value = first.json().await.expect("resource");
    assert_eq!(first, retry.json::<Value>().await.expect("same resource"));
    let other: Value = ensure(&f, &token, "logical-two")
        .await
        .json()
        .await
        .expect("different identity");
    assert_ne!(first["id"], other["id"]);
    let deleted = f
        .http
        .delete(format!(
            "{}/storage/databases/{}",
            f.storage_listener.base,
            first["id"].as_str().expect("id")
        ))
        .bearer_auth(&token)
        .send()
        .await
        .expect("delete");
    assert_eq!(deleted.status(), 204);
    restart(&mut f).await;
    assert_eq!(ensure(&f, &token, "logical-one").await.status(), 410);
    assert_eq!(
        sql(
            &f,
            &token,
            first["id"].as_str().expect("ID"),
            json!([{"sql":"CREATE TABLE denied (id TEXT PRIMARY KEY)"}])
        )
        .await
        .status(),
        404
    );
    f.stop().await;
}

async fn sql(f: &Fixture, token: &str, id: &str, statements: Value) -> reqwest::Response {
    f.http
        .post(format!(
            "{}/storage/databases/{id}/sql",
            f.storage_listener.base
        ))
        .bearer_auth(token)
        .json(&json!({"statements": statements}))
        .send()
        .await
        .expect("authenticated SQL")
}

#[tokio::test]
async fn parameterized_sql_is_atomic_and_parsed_actions_deny_mutations() {
    let f = Fixture::new(Default::default()).await;
    let token = f.token("storage:databases", vec![Read, Write]).await;
    let resource: Value = ensure(&f, &token, "sql-one")
        .await
        .json()
        .await
        .expect("resource");
    let id = resource["id"].as_str().expect("id");
    let response = sql(&f, &token, id, json!([
        {"sql":"CREATE TABLE records (id TEXT PRIMARY KEY, content TEXT NOT NULL)"},
        {"sql":"INSERT INTO records (id, content) VALUES (?, ?)", "params":[{"type":"text","value":"one"},{"type":"text","value":"private-value"}]},
        {"sql":"SELECT content FROM records WHERE id = ?", "params":[{"type":"text","value":"one"}]}
    ])).await;
    assert_eq!(response.status(), 200);
    let result: Value = response.json().await.expect("SQL results");
    assert_eq!(
        result["results"][2]["rows"],
        json!([[{"type":"text", "value":"private-value"}]])
    );
    let read = f.token("storage:databases", vec![Read]).await;
    for statement in [
        "/* SELECT */ DELETE FROM records",
        "CREATE TABLE forbidden (id TEXT PRIMARY KEY)",
        "SELECT content FROM records; DELETE FROM records",
    ] {
        assert_eq!(
            sql(&f, &read, id, json!([{"sql":statement}]))
                .await
                .status(),
            403
        );
    }
    let failed = sql(&f, &token, id, json!([
        {"sql":"DELETE FROM records"}, {"sql":"INSERT INTO absent (id) VALUES ('credential-do-not-echo')"}
    ])).await;
    assert_eq!(failed.status(), 400);
    let body = failed.text().await.expect("redacted failure");
    assert!(!body.contains("credential-do-not-echo"));
    let persisted: Value = sql(
        &f,
        &read,
        id,
        json!([{"sql":"SELECT content FROM records"}]),
    )
    .await
    .json()
    .await
    .expect("rollback snapshot");
    assert_eq!(
        persisted["results"][0]["rows"],
        result["results"][2]["rows"]
    );
    let wrong = f.token("storage:filesystems", vec![Read, Write]).await;
    assert_eq!(
        sql(
            &f,
            &wrong,
            id,
            json!([{"sql":"SELECT content FROM records"}])
        )
        .await
        .status(),
        401
    );
    f.stop().await;
}

async fn restart(f: &mut Fixture) {
    f.storage_listener.stop().await;
    f.databases = std::sync::Arc::new(
        storage_service::DatabaseRuntime::new(f._directory.0.join("databases"))
            .expect("reopen Storage owner"),
    );
    f.storage_listener = crate::support::native::Listener::start(storage_server::resource_router(
        storage_server::RouterState::new("https://storage.test").with_idp_client(f.client.clone()),
        Some(f.databases.clone()),
        Some(f.files.clone()),
    ))
    .await;
}

#[tokio::test]
async fn restart_preserves_sql_and_fences_never_reinitialize_content() {
    let mut f = Fixture::new(Default::default()).await;
    let token = f.token("storage:databases", vec![Read, Write]).await;
    let resource: Value = ensure(&f, &token, "restart")
        .await
        .json()
        .await
        .expect("resource");
    let id = resource["id"].as_str().expect("id").to_owned();
    assert_eq!(
        sql(
            &f,
            &token,
            &id,
            json!([
                {"sql":"CREATE TABLE records (id TEXT PRIMARY KEY)"},
                {"sql":"INSERT INTO records (id) VALUES ('persisted')"}
            ])
        )
        .await
        .status(),
        200
    );
    restart(&mut f).await;
    assert_eq!(
        ensure(&f, &token, "restart")
            .await
            .json::<Value>()
            .await
            .expect("reopened"),
        resource
    );
    let result: Value = sql(&f, &token, &id, json!([{"sql":"SELECT id FROM records"}]))
        .await
        .json()
        .await
        .expect("persisted SQL");
    assert_eq!(
        result["results"][0]["rows"],
        json!([[{"type":"text", "value":"persisted"}]])
    );
    f.databases
        .remove_local_copy(&f.scope, id.parse().expect("database ID"))
        .await
        .expect("retire local owner copy");
    assert_eq!(
        sql(&f, &token, &id, json!([{"sql":"SELECT id FROM records"}]))
            .await
            .status(),
        409
    );
    assert_eq!(ensure(&f, &token, "restart").await.status(), 409);
    restart(&mut f).await;
    assert_eq!(ensure(&f, &token, "restart").await.status(), 409);
    f.stop().await;
}

#[tokio::test]
async fn acknowledged_missing_owner_content_is_not_replaced() {
    let mut f = Fixture::new(Default::default()).await;
    let token = f.token("storage:databases", vec![Read, Write]).await;
    let resource: Value = ensure(&f, &token, "unavailable")
        .await
        .json()
        .await
        .expect("resource");
    let id = resource["id"].as_str().expect("id");
    let handle = f
        .databases
        .open_existing(&f.scope, id.parse().expect("ID"))
        .expect("lookup")
        .expect("owner");
    handle
        .retire_and_remove()
        .await
        .expect("public owner retirement simulates unavailable acknowledged content");
    assert_eq!(ensure(&f, &token, "unavailable").await.status(), 503);
    assert_eq!(
        sql(
            &f,
            &token,
            id,
            json!([{"sql":"CREATE TABLE replacement (id TEXT PRIMARY KEY)"}])
        )
        .await
        .status(),
        503
    );
    drop(handle);
    restart(&mut f).await;
    assert_eq!(ensure(&f, &token, "unavailable").await.status(), 503);
    f.stop().await;
}

#[tokio::test]
async fn disconnected_nodes_derive_the_same_identity_and_learned_tombstones_win() {
    let f = Fixture::new(Default::default()).await;
    let token = f.token("storage:databases", vec![Read, Write]).await;
    let other_runtime = std::sync::Arc::new(
        storage_service::DatabaseRuntime::new(f._directory.0.join("offline-other"))
            .expect("independent offline Node root"),
    );
    let mut other = crate::support::native::Listener::start(storage_server::resource_router(
        storage_server::RouterState::new("https://storage.test").with_idp_client(f.client.clone()),
        Some(other_runtime.clone()),
        None,
    ))
    .await;
    let first: Value = ensure(&f, &token, "offline-key")
        .await
        .json()
        .await
        .expect("first identity");
    let response = f
        .http
        .post(format!("{}/storage/databases/ensure", other.base))
        .bearer_auth(&token)
        .json(&json!({"creationKey":"offline-key", "name":"Different label"}))
        .send()
        .await
        .expect("disconnected independent creation");
    assert_eq!(response.status(), 200);
    let second: Value = response.json().await.expect("second identity");
    assert_eq!(first["id"], second["id"]);
    let id = first["id"].as_str().expect("ID").parse().expect("UUID");
    other_runtime
        .apply_tombstone(&f.scope, id)
        .expect("locally learned remote tombstone");
    let response = f
        .http
        .post(format!("{}/storage/databases/ensure", other.base))
        .bearer_auth(&token)
        .json(&json!({"creationKey":"offline-key"}))
        .send()
        .await
        .expect("learned deletion denies");
    assert_eq!(response.status(), 410);
    assert_eq!(
        ensure(&f, &token, "offline-key").await.status(),
        200,
        "unaware Node retains its prior local view"
    );
    other.stop().await;
    f.stop().await;
}

#[tokio::test]
async fn sql_bounds_and_unknown_fields_reject_without_partial_writes() {
    let f = Fixture::new(Default::default()).await;
    let token = f.token("storage:databases", vec![Read, Write]).await;
    let resource: Value = ensure(&f, &token, "bounds")
        .await
        .json()
        .await
        .expect("resource");
    let id = resource["id"].as_str().expect("id");
    for statements in [
        json!([]),
        json!(vec![json!({"sql":"SELECT id FROM records"}); 33]),
        json!([{"sql":"SELECT id FROM records", "owner":"spoof"}]),
        json!([{"sql":"SELECT id FROM records", "params":[{"type":"integer","value":"9223372036854775808"}]}]),
    ] {
        assert_eq!(sql(&f, &token, id, statements).await.status(), 400);
    }
    let oversized = sql(&f, &token, id, json!([{"sql":"SELECT id FROM records", "params":[{"type":"text","value":"x".repeat(256 * 1024)}]}])).await;
    assert_eq!(oversized.status(), 413);
    let spoof = f
        .http
        .post(format!(
            "{}/storage/databases/ensure",
            f.storage_listener.base
        ))
        .bearer_auth(&token)
        .json(&json!({"creationKey":"spoof", "applicationId":f.scope.application_id}))
        .send()
        .await
        .expect("spoof request");
    assert_eq!(spoof.status(), 422);
    f.stop().await;
}

#[tokio::test]
async fn canonical_application_shares_clients_but_isolates_other_users_and_applications() {
    use idp_model::contract::{GrantType, TokenEndpointAuthMethod};
    use idp_service::repo::ClientRepo;
    let f = Fixture::new(Default::default()).await;
    for (client, uri) in [
        ("second-client", "https://human.test"),
        ("other-app", "https://different.test"),
    ] {
        let mut registration = crate::support::native::registration(
            client,
            vec![GrantType::Password, GrantType::TokenExchange],
            vec!["openid".into()],
            vec!["storage:databases".into()],
            TokenEndpointAuthMethod::ClientSecretBasic,
        );
        registration.application.uri = uri.into();
        f.clients
            .create_client(registration)
            .await
            .expect("registered OAuth Client");
    }
    let token = f.token("storage:databases", vec![Read, Write]).await;
    let same = f
        .token_for_client("storage:databases", vec![Read, Write], "second-client")
        .await;
    let other_app = f
        .token_for_client("storage:databases", vec![Read, Write], "other-app")
        .await;
    let resource: Value = ensure(&f, &token, "namespace-key")
        .await
        .json()
        .await
        .expect("resource");
    assert_eq!(
        ensure(&f, &same, "namespace-key")
            .await
            .json::<Value>()
            .await
            .expect("same Application"),
        resource
    );
    let other: Value = ensure(&f, &other_app, "namespace-key")
        .await
        .json()
        .await
        .expect("other namespace");
    assert_ne!(resource["id"], other["id"]);
    let id = resource["id"].as_str().expect("id");
    assert_eq!(
        sql(
            &f,
            &other_app,
            id,
            json!([{"sql":"CREATE TABLE denied (id TEXT PRIMARY KEY)"}])
        )
        .await
        .status(),
        404
    );
    use idp_model::contract::EntityType;
    use idp_service::{replica::DbUserRepo, repo::UserRepo};
    let user = DbUserRepo::new(f.idp.client(), Default::default())
        .create_user_with_password("other-user", "valid-password")
        .await
        .expect("other User");
    f.keys
        .ensure_entity_master_key(EntityType::User, user.id, "valid-password")
        .expect("User key");
    f.keys
        .create_key(
            None,
            EntityType::User,
            user.id,
            true,
            "other User signing key".into(),
            None,
        )
        .await
        .expect("User signing key");
    let other_user = f
        .token_for_user(
            "storage:databases",
            vec![Read, Write],
            "human",
            "other-user",
        )
        .await;
    assert_eq!(
        sql(
            &f,
            &other_user,
            id,
            json!([{"sql":"CREATE TABLE denied (id TEXT PRIMARY KEY)"}])
        )
        .await
        .status(),
        404
    );
    assert_ne!(
        ensure(&f, &other_user, "namespace-key")
            .await
            .json::<Value>()
            .await
            .expect("other User resource")["id"],
        resource["id"]
    );
    f.revoke_user().await;
    assert_eq!(
        sql(
            &f,
            &token,
            id,
            json!([{"sql":"CREATE TABLE denied (id TEXT PRIMARY KEY)"}])
        )
        .await
        .status(),
        401
    );
    f.stop().await;
}

#[test]
#[ignore = "subprocess entry point for creation_crash_recovers_at_public_owner_boundary"]
fn creation_crash_worker() {
    use std::io::Write;
    let root = std::env::var_os("OF_SQL_CRASH_ROOT").expect("parent supplies isolated root");
    let runtime =
        storage_service::DatabaseRuntime::new(root.into()).expect("child Storage runtime");
    let scope = crate::support::native::StorageNamespace {
        user_sub: "crash-test-user".into(),
        application_id: "00000000-0000-0000-0000-000000000001"
            .parse()
            .expect("Application"),
    };
    let resource = runtime
        .ensure(&scope, "acknowledged", None)
        .expect("acknowledged resource");
    let owner = runtime
        .open_existing(&scope, resource.id)
        .expect("lookup")
        .expect("owner");
    tokio::runtime::Builder::new_current_thread().build().expect("child executor").block_on(async {
        owner.client().execute_sql("CREATE TABLE records (id TEXT PRIMARY KEY); INSERT INTO records (id) VALUES ('survives-crash')", None).await.expect("committed SQL");
    });
    println!("CREATING");
    std::io::stdout().flush().expect("signal parent");
    for i in 0..20 {
        runtime
            .ensure(&scope, &format!("interrupted-{i}"), None)
            .expect("creation attempt");
    }
}

#[tokio::test]
async fn creation_crash_recovers_at_public_owner_boundary() {
    use std::io::BufRead;
    let root = tempfile::tempdir().expect("isolated child root");
    let mut child = std::process::Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "sql_http_tests::creation_crash_worker",
            "--ignored",
            "--nocapture",
        ])
        .env("OF_SQL_CRASH_ROOT", root.path())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("creation subprocess");
    let mut output = std::io::BufReader::new(child.stdout.take().expect("child output"));
    let mut line = String::new();
    loop {
        assert_ne!(
            output.read_line(&mut line).expect("read child progress"),
            0,
            "child reached creation phase"
        );
        if line.contains("CREATING") {
            break;
        }
        line.clear();
    }
    child.kill().expect("interrupt creation process");
    child.wait().expect("reap interrupted process");
    let runtime = storage_service::DatabaseRuntime::new(root.path().into()).expect("restart owner");
    let scope = crate::support::native::StorageNamespace {
        user_sub: "crash-test-user".into(),
        application_id: "00000000-0000-0000-0000-000000000001"
            .parse()
            .expect("Application"),
    };
    let ready = runtime
        .ensure(&scope, "acknowledged", None)
        .expect("reopen acknowledged resource");
    assert_eq!(
        ready.id.to_string(),
        "464d0adf-0786-54eb-a64a-5b363a30b618",
        "independent UUID v5 contract vector"
    );
    let owner = runtime
        .open_existing(&scope, ready.id)
        .expect("lookup")
        .expect("owner");
    let rows = owner
        .client()
        .execute_sql("SELECT id FROM records", None)
        .await
        .expect("acknowledged data survives crash");
    assert_eq!(
        rows[0].rows[0].values[0],
        ofdb_sql::Value::Text("survives-crash".into())
    );
    for i in 0..20 {
        let key = format!("interrupted-{i}");
        let first = runtime
            .ensure(&scope, &key, None)
            .expect("interrupted creation retries");
        assert_eq!(
            runtime.ensure(&scope, &key, None).expect("stable retry"),
            first
        );
    }
}

#[tokio::test]
async fn result_limits_rollback_batches_and_schema_initialization_is_retryable() {
    let f = Fixture::new(Default::default()).await;
    let token = f.token("storage:databases", vec![Read, Write]).await;
    let resource: Value = ensure(&f, &token, "atomic-limits")
        .await
        .json()
        .await
        .expect("resource");
    let id = resource["id"].as_str().expect("ID");
    assert_eq!(
        sql(
            &f,
            &token,
            id,
            json!([
                {"sql":"CREATE TABLE records (id TEXT PRIMARY KEY)"},
                {"sql":"INSERT INTO absent (id) VALUES ('fails')"}
            ])
        )
        .await
        .status(),
        400
    );
    assert_eq!(
        sql(&f, &token, id, json!([{"sql":"SELECT id FROM records"}]))
            .await
            .status(),
        400,
        "failed initialization left no partial schema"
    );
    for _ in 0..2 {
        assert_eq!(
            sql(
                &f,
                &token,
                id,
                json!([{"sql":"CREATE TABLE IF NOT EXISTS records (id TEXT PRIMARY KEY)"}])
            )
            .await
            .status(),
            200
        );
    }
    let values = (0..1001)
        .map(|i| format!("('{i}')"))
        .collect::<Vec<_>>()
        .join(",");
    assert_eq!(
        sql(
            &f,
            &token,
            id,
            json!([
                {"sql":format!("INSERT INTO records (id) VALUES {values}")},
                {"sql":"SELECT id FROM records"}
            ])
        )
        .await
        .status(),
        413
    );
    let snapshot: Value = sql(&f, &token, id, json!([{"sql":"SELECT id FROM records"}]))
        .await
        .json()
        .await
        .expect("complete snapshot");
    assert_eq!(snapshot["results"][0]["rows"], json!([]));
    f.stop().await;
}

#[tokio::test]
async fn generated_sql_contract_roundtrips_lossless_wire_scalars() {
    let f = Fixture::new(Default::default()).await;
    let token = f.token("storage:databases", vec![Read, Write]).await;
    let config = of_storage_client::apis::configuration::Configuration {
        base_path: format!("{}/storage", f.storage_listener.base),
        client: f.http.clone(),
        bearer_access_token: Some(token),
        ..Default::default()
    };
    let resource = of_storage_client::apis::resources_api::ensure_database(
        &config,
        of_storage_client::apis::resources_api::EnsureDatabaseParams {
            ensure_database_request: of_storage_client::models::EnsureDatabaseRequest::new(
                "wire-values".into(),
            ),
        },
    )
    .await
    .expect("generated ensure contract");
    let values = json!([
        {"type":"text","value":"one"}, {"type":"integer","value":"9223372036854775807"},
        {"type":"boolean","value":true}, {"type":"float","value":1.25},
        {"type":"uuid","value":"00000000-0000-0000-0000-000000000001"},
        {"type":"blob","value":[0,128,255]}, {"type":"null"}
    ]);
    let request = serde_json::from_value(json!({"statements":[
        {"sql":"CREATE TABLE scalars (id TEXT PRIMARY KEY, i BIGINT, b BOOLEAN, f DOUBLE, u UUID, bytes BLOB, n TEXT)"},
        {"sql":"INSERT INTO scalars (id, i, b, f, u, bytes, n) VALUES (?, ?, ?, ?, ?, ?, ?)", "params":values},
        {"sql":"SELECT id, i, b, f, u, bytes, n FROM scalars"}
    ]})).expect("generated request accepts scalar union");
    let result = of_storage_client::apis::resources_sql_api::execute_sql(
        &config,
        of_storage_client::apis::resources_sql_api::ExecuteSqlParams {
            database_id: resource.id,
            sql_request: request,
        },
    )
    .await
    .expect("generated SQL request and response");
    let result = serde_json::to_value(result).expect("generated result wire shape");
    assert_eq!(result["results"][2]["rows"][0], values);
    f.stop().await;
}

#[tokio::test]
async fn learned_device_restriction_denies_subsequent_sql_and_creation() {
    use device_service::replica::{DbSelectionPolicyRepo, SelectionPolicy};
    use std::sync::{Arc, OnceLock};
    let mut f = Fixture::new(Default::default()).await;
    let policy = |allowed| SelectionPolicy {
        device_id: f.device.device_id,
        owner_subject: f.owner.to_string(),
        application_id: None,
        selected_kind: None,
        selected_id: None,
        admin_allowed: allowed,
    };
    let policies = DbSelectionPolicyRepo::new(f.devices.clone().client());
    policies
        .set_prevalidated(policy(true))
        .await
        .expect("allow local Device");
    let copies = Arc::new(OnceLock::new());
    assert!(
        copies
            .set(storage_server::StorageCopyRetirement::new(
                f.databases.clone(),
                f.files.clone()
            ))
            .is_ok()
    );
    let authorizer = crate::support::replication::owner_replication_authorizer(
        device_server::DeviceAuthority::new(f.devices.clone(), f.control.clone()),
        f.identity.endpoint().id(),
        copies.clone(),
    );
    let client = f
        .client
        .clone()
        .with_local_device_policy(f.identity.endpoint().id(), authorizer);
    f.storage_listener.stop().await;
    f.storage_listener = crate::support::native::Listener::start(storage_server::resource_router(
        storage_server::RouterState::new("https://storage.test").with_idp_client(client),
        Some(f.databases.clone()),
        Some(f.files.clone()),
    ))
    .await;
    let token = f.token("storage:databases", vec![Read, Write]).await;
    let resource: Value = ensure(&f, &token, "restricted")
        .await
        .json()
        .await
        .expect("allowed creation");
    let id = resource["id"].as_str().expect("ID");
    assert_eq!(
        sql(
            &f,
            &token,
            id,
            json!([{"sql":"CREATE TABLE records (id TEXT PRIMARY KEY)"}])
        )
        .await
        .status(),
        200
    );
    policies
        .set_admin_allowed_prevalidated(f.device.device_id, &f.owner.to_string(), false)
        .await
        .expect("learn restriction");
    assert_eq!(
        sql(&f, &token, id, json!([{"sql":"SELECT id FROM records"}]))
            .await
            .status(),
        401
    );
    assert_eq!(ensure(&f, &token, "restricted").await.status(), 401);
    assert_eq!(ensure(&f, &token, "cannot-provision").await.status(), 401);
    f.stop().await;
}

#[tokio::test]
async fn response_revalidation_runs_after_commit_without_holding_the_sql_owner() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let mut f = Fixture::new(Default::default()).await;
    let token = f.token("storage:databases", vec![Read, Write]).await;
    let resource: Value = ensure(&f, &token, "commit-revalidation")
        .await
        .json()
        .await
        .expect("resource");
    let id = resource["id"].as_str().expect("ID");
    let gate = crate::support::native::Gate::default();
    let calls = Arc::new(AtomicUsize::new(0));
    let original = f.client.clone();
    let pause = gate.clone();
    let client = storage_server::IdpClient::local(move |token| {
        let original = original.clone();
        let gate = pause.clone();
        let calls = calls.clone();
        async move {
            if calls.fetch_add(1, Ordering::SeqCst) == 2 {
                gate.pause().await;
            }
            original.introspect(&token).await
        }
    });
    f.storage_listener.stop().await;
    f.storage_listener = crate::support::native::Listener::start(storage_server::resource_router(
        storage_server::RouterState::new("https://storage.test").with_idp_client(client),
        Some(f.databases.clone()),
        Some(f.files.clone()),
    ))
    .await;
    {
        let request = sql(
            &f,
            &token,
            id,
            json!([
                {"sql":"CREATE TABLE records (id TEXT PRIMARY KEY)"},
                {"sql":"INSERT INTO records (id) VALUES ('admitted')"}
            ]),
        );
        tokio::pin!(request);
        tokio::select! {
            _ = &mut request => panic!("must reach post-commit response authorization"),
            () = gate.reached() => {},
        }
        let owner = f
            .databases
            .open_existing(&f.scope, id.parse().expect("UUID"))
            .expect("lookup")
            .expect("owner");
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            owner.client().execute_sql("SELECT id FROM records", None),
        )
        .await
        .expect("authorization holds no owner transaction")
        .expect("already committed SQL");
        assert_eq!(
            result[0].rows[0].values[0],
            ofdb_sql::Value::Text("admitted".into())
        );
        f.revoke_user().await;
        gate.release();
        assert_eq!(
            request.await.status(),
            401,
            "committed work does not authorize obsolete results"
        );
        assert_eq!(
            sql(&f, &token, id, json!([{"sql":"DELETE FROM records"}]))
                .await
                .status(),
            401
        );
        drop(owner);
    }
    f.stop().await;
}

#[tokio::test]
async fn password_manager_schema_and_crud_requests_survive_retry_and_native_restart() {
    let mut f = Fixture::new(Default::default()).await;
    let token = f.token("storage:databases", vec![Read, Write]).await;
    let resource: Value = ensure(&f, &token, "password-manager:default")
        .await
        .json()
        .await
        .expect("application database");
    let id = resource["id"].as_str().expect("database ID").to_owned();
    let schema = json!([
        {"sql":"CREATE TABLE IF NOT EXISTS secret_schema (id UUID PRIMARY KEY, version INTEGER NOT NULL)","params":[]},
        {"sql":"CREATE TABLE IF NOT EXISTS secrets (id UUID PRIMARY KEY, content TEXT NOT NULL)","params":[]},
        {"sql":"INSERT INTO secret_schema (id, version) VALUES (?, ?) ON CONFLICT (id) DO NOTHING","params":[{"type":"uuid","value":"f4077b65-9a42-5ade-b648-299c09235271"},{"type":"integer","value":"1"}]},
        {"sql":"SELECT id, version FROM secret_schema","params":[]}
    ]);
    for _ in 0..2 {
        let response = sql(&f, &token, &id, schema.clone()).await;
        assert_eq!(response.status(), 200);
        let response: Value = response.json().await.expect("atomic schema/version");
        assert_eq!(
            response["results"][3]["rows"],
            json!([[{"type":"uuid","value":"f4077b65-9a42-5ade-b648-299c09235271"},{"type":"integer","value":"1"}]])
        );
    }
    let record = json!({"id":"01900000-0000-7000-8000-000000000001","name":"Test only","secret":"not-a-real-password","uris":[],"fields":[],"history":[],"favorite":false,"createdAt":"2026-01-01T00:00:00.000Z","updatedAt":"2026-01-01T00:00:00.000Z"}).to_string();
    let key = json!({"type":"uuid","value":"01900000-0000-7000-8000-000000000001"});
    assert_eq!(sql(&f, &token, &id, json!([{"sql":"INSERT INTO secrets (id, content) VALUES (?, ?)","params":[key,{"type":"text","value":record}]}])).await.status(), 200);
    restart(&mut f).await;
    assert_eq!(
        ensure(&f, &token, "password-manager:default")
            .await
            .json::<Value>()
            .await
            .expect("same resource"),
        resource
    );
    let result: Value = sql(&f, &token, &id, json!([{"sql":"SELECT name FROM tables","params":[]},{"sql":"SELECT id, content FROM secrets","params":[]}])).await.json().await.expect("application snapshot");
    assert_eq!(result["results"][1]["columns"], json!(["id", "content"]));
    assert_eq!(
        result["results"][1]["rows"],
        json!([[key,{"type":"text","value":record}]])
    );
    assert_eq!(sql(&f, &token, &id, json!([{"sql":"UPDATE secrets SET content = ? WHERE id = ?","params":[{"type":"text","value":"updated test payload"},key]}])).await.status(), 200);
    let result: Value = sql(
        &f,
        &token,
        &id,
        json!([{"sql":"SELECT id, content FROM secrets"}]),
    )
    .await
    .json()
    .await
    .expect("updated snapshot");
    assert_eq!(
        result["results"][0]["rows"][0][1],
        json!({"type":"text","value":"updated test payload"})
    );
    assert_eq!(
        sql(
            &f,
            &token,
            &id,
            json!([{"sql":"DELETE FROM secrets WHERE id = ?","params":[key]}])
        )
        .await
        .status(),
        200
    );
    let result: Value = sql(
        &f,
        &token,
        &id,
        json!([{"sql":"SELECT id, content FROM secrets"}]),
    )
    .await
    .json()
    .await
    .expect("deleted snapshot");
    assert_eq!(result["results"][0]["rows"], json!([]));
    f.stop().await;
}

#[tokio::test]
async fn corrupt_acknowledged_content_fails_ensure_and_execution_after_restart() {
    for damage in [b"".as_slice(), b"not a database".as_slice()] {
        let mut f = Fixture::new(Default::default()).await;
        let token = f.token("storage:databases", vec![Read, Write]).await;
        let resource: Value = ensure(&f, &token, "corruption")
            .await
            .json()
            .await
            .expect("resource");
        let id = resource["id"].as_str().expect("ID");
        assert_eq!(
            sql(
                &f,
                &token,
                id,
                json!([
                    {"sql":"CREATE TABLE records (id TEXT PRIMARY KEY)"},
                    {"sql":"INSERT INTO records (id) VALUES ('committed')"}
                ])
            )
            .await
            .status(),
            200
        );
        // Stop every issued owner before damaging its persisted content.
        f.storage_listener.stop().await;
        f.databases = std::sync::Arc::new(
            storage_service::DatabaseRuntime::new(f._directory.0.join("unused"))
                .expect("release owner"),
        );
        let path = f
            ._directory
            .0
            .join("databases/databases")
            .join(&f.scope.user_sub)
            .join(f.scope.application_id.to_string())
            .join(format!("{id}.redb"));
        std::fs::write(&path, damage).expect("damage closed acknowledged file");
        restart(&mut f).await;
        assert_eq!(ensure(&f, &token, "corruption").await.status(), 503);
        assert_eq!(
            sql(
                &f,
                &token,
                id,
                json!([{"sql":"CREATE TABLE replacement (id TEXT PRIMARY KEY)"}])
            )
            .await
            .status(),
            503
        );
        assert_eq!(std::fs::read(&path).expect("content not replaced"), damage);
        f.stop().await;
    }
}

#[tokio::test]
async fn returning_requires_read_and_write_before_any_mutation() {
    let f = Fixture::new(Default::default()).await;
    let token = f.token("storage:databases", vec![Read, Write]).await;
    let write = f.token("storage:databases", vec![Write]).await;
    let read = f.token("storage:databases", vec![Read]).await;
    let resource: Value = ensure(&f, &token, "returning")
        .await
        .json()
        .await
        .expect("resource");
    let id = resource["id"].as_str().expect("ID");
    assert_eq!(
        sql(
            &f,
            &token,
            id,
            json!([
                {"sql":"CREATE TABLE secrets (id TEXT PRIMARY KEY, content TEXT NOT NULL)"},
                {"sql":"INSERT INTO secrets (id, content) VALUES ('saved', 'private')"}
            ])
        )
        .await
        .status(),
        200
    );
    for statement in [
        "UPDATE secrets SET content=content RETURNING content",
        "UPDATE secrets SET content='changed' RETURNING content",
        "DELETE FROM secrets RETURNING content",
        "INSERT INTO secrets (id, content) VALUES ('denied', 'new') RETURNING content",
    ] {
        for token in [&write, &read] {
            let response = sql(&f, token, id, json!([{"sql":statement}])).await;
            assert_eq!(response.status(), 403, "{statement}");
            assert_eq!(
                response.json::<Value>().await.expect("redacted denial"),
                json!({"code":"forbidden"})
            );
        }
        let snapshot: Value = sql(
            &f,
            &read,
            id,
            json!([{"sql":"SELECT id, content FROM secrets"}]),
        )
        .await
        .json()
        .await
        .expect("unchanged snapshot");
        assert_eq!(
            snapshot["results"][0]["rows"],
            json!([[{"type":"text","value":"saved"},{"type":"text","value":"private"}]])
        );
    }
    assert_eq!(
        sql(
            &f,
            &token,
            id,
            json!([{"sql":"UPDATE secrets SET content=content RETURNING content"}])
        )
        .await
        .status(),
        200
    );
    assert_eq!(
        sql(
            &f,
            &write,
            id,
            json!([{"sql":"UPDATE secrets SET content='write-only'"}])
        )
        .await
        .status(),
        200
    );
    f.stop().await;
}

#[tokio::test]
async fn amplified_sql_is_rejected_before_transaction_admission() {
    let f = Fixture::new(Default::default()).await;
    let token = f.token("storage:databases", vec![Read, Write]).await;
    let resource: Value = ensure(&f, &token, "query-subset")
        .await
        .json()
        .await
        .expect("resource");
    let id = resource["id"].as_str().expect("ID");
    assert_eq!(
        sql(
            &f,
            &token,
            id,
            json!([
                {"sql":"CREATE TABLE records (id TEXT PRIMARY KEY, content TEXT)"},
                {"sql":"INSERT INTO records (id, content) VALUES ('saved', 'original')"}
            ])
        )
        .await
        .status(),
        200
    );
    let owner = f
        .databases
        .open_existing(&f.scope, id.parse().expect("ID"))
        .expect("lookup")
        .expect("owner");
    let client = owner.client();
    let transaction = client
        .transaction()
        .await
        .expect("hold transaction admission");
    for query in [
        "SELECT records.content FROM records JOIN other ON records.id=other.id",
        "SELECT content FROM records WHERE id IN (SELECT id FROM records)",
        "SELECT content || content AS doubled FROM records",
        "SELECT content, content FROM records",
    ] {
        let response = sql(
            &f,
            &token,
            id,
            json!([
                {"sql":"UPDATE records SET content='must-not-commit'"}, {"sql":query}
            ]),
        );
        let response = tokio::time::timeout(std::time::Duration::from_secs(5), response)
            .await
            .expect("shape rejected before waiting for the held owner transaction");
        assert_eq!(response.status(), 400, "{query}");
        assert_eq!(
            response
                .json::<Value>()
                .await
                .expect("redacted unsupported shape"),
            json!({"code":"invalidRequest"})
        );
    }
    transaction
        .rollback()
        .await
        .expect("release transaction admission");
    let snapshot: Value = sql(
        &f,
        &token,
        id,
        json!([{"sql":"SELECT content FROM records"}]),
    )
    .await
    .json()
    .await
    .expect("unchanged");
    assert_eq!(
        snapshot["results"][0]["rows"],
        json!([[{"type":"text","value":"original"}]])
    );
    f.stop().await;
}

#[tokio::test]
async fn native_budgets_bound_plain_http_scans_even_with_limit_or_filter() {
    let f = Fixture::new(Default::default()).await;
    let token = f.token("storage:databases", vec![Read, Write]).await;
    let resource: Value = ensure(&f, &token, "bounded-scan")
        .await
        .json()
        .await
        .expect("resource");
    let id = resource["id"].as_str().expect("ID");
    let owner = f
        .databases
        .open_existing(&f.scope, id.parse().expect("ID"))
        .expect("lookup")
        .expect("owner");
    let client = owner.client();
    client
        .execute_sql(
            "CREATE TABLE records (id TEXT PRIMARY KEY, content TEXT)",
            None,
        )
        .await
        .expect("schema");
    let params =
        ofdb_sql::QueryParams::Positional(vec![ofdb_sql::Value::Text("x".repeat(64 * 1024))]);
    let mut transaction = client
        .transaction()
        .await
        .expect("native seed outside HTTP budget");
    for i in 0..160 {
        transaction
            .execute_sql(
                &format!("INSERT INTO records (id, content) VALUES ('{i}', ?)"),
                Some(&params),
            )
            .await
            .expect("seed compressed large content");
    }
    transaction
        .commit()
        .await
        .expect("seed content beyond decoded scan budget");
    for query in [
        "SELECT content FROM records",
        "SELECT id FROM records LIMIT 1",
        "SELECT id FROM records WHERE id='absent'",
    ] {
        let response = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            sql(&f, &token, id, json!([{"sql":query}])),
        )
        .await
        .expect("bounded scan returns promptly");
        assert_eq!(response.status(), 413, "{query}");
        assert_eq!(
            response.json::<Value>().await.expect("redacted limit"),
            json!({"code":"limitExceeded"})
        );
    }
    f.stop().await;
}

#[tokio::test]
async fn aggregate_byte_limit_rolls_back_mutation_and_snapshot_together() {
    let f = Fixture::new(Default::default()).await;
    let token = f.token("storage:databases", vec![Read, Write]).await;
    let resource: Value = ensure(&f, &token, "snapshot-bytes")
        .await
        .json()
        .await
        .expect("resource");
    let id = resource["id"].as_str().expect("ID");
    assert_eq!(
        sql(
            &f,
            &token,
            id,
            json!([{"sql":"CREATE TABLE records (id TEXT PRIMARY KEY, content TEXT)"}])
        )
        .await
        .status(),
        200
    );
    for i in 0..8 {
        assert_eq!(sql(&f, &token, id, json!([{"sql":"INSERT INTO records (id, content) VALUES (?, ?)", "params":[{"type":"text","value":i.to_string()},{"type":"text","value":"x".repeat(120000)}]}])).await.status(), 200);
    }
    let before = sql(
        &f,
        &token,
        id,
        json!([{"sql":"SELECT id, content FROM records"}]),
    )
    .await;
    assert_eq!(before.status(), 200);
    let before: Value = before.json().await.expect("readable boundary");
    assert_eq!(sql(&f, &token, id, json!([
        {"sql":"UPDATE records SET content=? WHERE id='0'", "params":[{"type":"text","value":"y".repeat(225000)}]},
        {"sql":"SELECT id, content FROM records"}
    ])).await.status(), 413);
    let after: Value = sql(
        &f,
        &token,
        id,
        json!([{"sql":"SELECT id, content FROM records"}]),
    )
    .await
    .json()
    .await
    .expect("still readable");
    assert_eq!(after, before);
    f.stop().await;
}
