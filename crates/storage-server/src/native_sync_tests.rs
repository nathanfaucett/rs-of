use std::sync::Arc;

use of_model::idp::{contract::IntrospectionResponse, model::Id};
use iroh_chain::{EndpointIdStore, Server};
use of_model::contract::SelectedResource;
use storage_service::{DatabaseRuntime, ScopedFileSystemRuntime};
use tokio_util::sync::CancellationToken;

use crate::{IdpClient, StorageReplicationRuntime, StorageSyncDirection, StorageSyncRequest};
use crate::execution_test_support::{DomainNode, IrohPair, TestDirectory, approved_signers, sql};

struct Namespace { owner: String, application: Id }
impl of_model::storage::StorageNamespace for Namespace {
    fn user_sub(&self) -> &str { &self.owner }
    fn application_id(&self) -> Id { self.application }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_driven_native_sync_persists_sql_and_kv_at_reopened_destination() {
    tokio::time::timeout(std::time::Duration::from_secs(60), async {
        let root = TestDirectory::new();
        let pair = IrohPair::new().await;
        let owner = Id::now_v7();
        let devices = [Id::now_v7(), Id::now_v7()];
        let signers = approved_signers(devices);
        let nodes = [
            DomainNode::new(root.0.join("left"), owner, devices, pair.ids(), &signers).await,
            DomainNode::new(root.0.join("right"), owner, devices, pair.ids(), &signers).await,
        ];
        let scope = Namespace { owner: owner.to_string(), application: Id::now_v7() };
        let paths = [root.0.join("data-left"), root.0.join("data-right")];
        let databases = paths.each_ref().map(|path| Arc::new(DatabaseRuntime::new(path.clone()).expect("persistent owner")));
        let (resource, source) = databases[0].create(&scope, Some("Notes".into())).expect("source database");
        let selection = SelectedResource { owner_subject: scope.owner.clone(), application_id: scope.application.to_string(), kind: "database".into(), resource_id: resource.id.to_string() };
        for node in &nodes { node.select(&selection).await; }
        sql(&source, &format!("CREATE TABLE documents (id UUID PRIMARY KEY, value TEXT); INSERT INTO documents VALUES (CAST('{}' AS UUID), 'native-kept')", Id::now_v7())).await;
        let source_kv = databases[0].open_kv_selected(&scope, resource.id).expect("source KV").expect("selected KV owner");
        source_kv.client().set("documents/body", ofdb_kv::Value::Text("real-kv-body".into()), None).await.expect("write local KV");
        let stop = CancellationToken::new();
        let mut runtimes = Vec::new();
        let mut routers = Vec::new();
        for side in 0..2 {
            let peers = EndpointIdStore::default(); peers.add(pair.ids()[1-side]);
            let server = Server::new(pair.endpoints[side].clone(), peers);
            let mut runtime = StorageReplicationRuntime::start(server.clone(), nodes[side].authorizer(), databases[side].clone(),
                Arc::new(ScopedFileSystemRuntime::new(root.0.join(format!("files-{side}")), pair.ids()[side]).expect("file owner")),
                root.0.join(format!("stage-{side}")), stop.clone());
            runtime.set_policy_changes(nodes[side].idp.sync_changes().expect("IdP events").merge(nodes[side].management.sync_changes().expect("Device events")));
            runtime.prepare().await.expect("current policy projection");
            routers.push(server.router(runtime.data_handler()));
            runtimes.push(runtime);
        }
        let destination_kv = databases[1].open_kv_selected(&scope, resource.id).expect("destination KV").expect("selected destination");
        destination_kv.client().set("documents/from-right", ofdb_kv::Value::Text("right-body".into()), None).await.expect("write destination KV before sync");
        let subject = scope.owner.clone(); let application = scope.application.to_string();
        let idp = IdpClient::local(move |token| {
            let subject = subject.clone(); let application = application.clone();
            async move {
                if token != "current-user-token" { return Err(crate::IdpIntrospectionError::InvalidToken); }
                Ok(serde_json::from_value::<IntrospectionResponse>(serde_json::json!({
                    "claims": { "type":"Bearer", "use":"Access", "exp":i64::MAX, "nbf":0, "iat":0,
                        "iss":"https://installation.example/idp", "aud":"storage:databases", "resource":"storage:databases",
                        "client_id":"app-client", "sub":subject, "principal_type":"user", "scope":"",
                        "authorization_details":[{"type":"storage","actions":["read","write"]}] },
                    "applicationId":application,
                })).expect("current User grant"))
            }
        });
        let driver = runtimes[0].sync_driver(idp).expect("live native driver");
        let request = StorageSyncRequest { target_endpoint_id: pair.ids()[1].to_string(), resource_id: resource.id.to_string(), kind: "database".into(), direction: StorageSyncDirection::Bidirectional };
        assert!(driver.synchronize("invalid", request.clone(), async { panic!("invalid User must not resolve a target") }).await.is_err(), "invalid User cannot drive an approved Device");
        for direction in [StorageSyncDirection::Push, StorageSyncDirection::Pull] {
            let unsupported = StorageSyncRequest { direction, ..request.clone() };
            assert_eq!(driver.synchronize("current-user-token", unsupported, async { panic!("unsupported direction must not resolve") }).await.expect_err("direction is unsupported").kind(), std::io::ErrorKind::Unsupported);
        }
        let filesystem = StorageSyncRequest { kind: "filesystem".into(), ..request.clone() };
        assert_eq!(driver.synchronize("current-user-token", filesystem, async { panic!("unsupported content must not resolve") }).await.expect_err("filesystem unsupported").kind(), std::io::ErrorKind::Unsupported);
        assert!(driver.synchronize("current-user-token", request.clone(), async { Ok(pair.endpoints[0].addr()) }).await.is_err(), "metadata endpoint must match selected Device");
        driver.synchronize("current-user-token", request.clone(), async { Ok(pair.endpoints[1].addr()) }).await.expect("full SQL + KV owner completion");
        assert_eq!(source_kv.client().get("documents/from-right").await.expect("reverse KV apply"), Some(ofdb_kv::Value::Text("right-body".into())));
        let destination_sql = databases[1].open_selected(&scope, resource.id).expect("destination SQL").expect("selected SQL");
        sql(&destination_sql, &format!("INSERT INTO documents VALUES (CAST('{}' AS UUID), 'right-kept')", Id::now_v7())).await;
        driver.synchronize("current-user-token", request.clone(), async { Ok(pair.endpoints[1].addr()) }).await.expect("reverse SQL exchange");
        let source_rows = source.client().translate_and_execute("SELECT value FROM documents ORDER BY value", &db::SqlTranslator).await.expect("reverse SQL apply");
        assert_eq!(source_rows[0].rows.iter().map(|row| row.values[0].clone()).collect::<Vec<_>>(), [db::Value::Text("native-kept".into()), db::Value::Text("right-kept".into())]);
        drop(destination_sql); drop(destination_kv);
        nodes[0].restrict(1, false).await;
        assert!(driver.synchronize("current-user-token", request, async { panic!("restricted Device must not resolve a target") }).await.is_err(), "learned Device restriction stops the next run");
        drop(driver);
        for runtime in runtimes { runtime.shutdown().await.expect("stop policy workers"); }
        for router in routers { router.shutdown().await.expect("stop iroh DATA"); }
        drop(source_kv); drop(source); drop(databases);
        let reopened = DatabaseRuntime::new(paths[1].clone()).expect("reopen destination owner");
        let sql_owner = reopened.open_selected(&scope, resource.id).expect("reopen selected SQL").expect("SQL exists");
        let rows = sql_owner.client().translate_and_execute("SELECT value FROM documents ORDER BY value", &db::SqlTranslator).await.expect("read reopened SQL");
        assert_eq!(rows[0].rows.iter().map(|row| row.values[0].clone()).collect::<Vec<_>>(), [db::Value::Text("native-kept".into()), db::Value::Text("right-kept".into())]);
        let kv_owner = reopened.open_kv_selected(&scope, resource.id).expect("reopen KV").expect("KV exists");
        assert_eq!(kv_owner.client().get("documents/body").await.expect("read reopened KV"), Some(ofdb_kv::Value::Text("real-kv-body".into())));
        drop(kv_owner); drop(sql_owner); drop(reopened);
        pair.close().await;
    }).await.expect("native transfer and reopen complete within bound");
}
