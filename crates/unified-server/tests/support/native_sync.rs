use std::{path::Path, sync::Arc, time::Duration};

use device_service::{DeviceRepo, replica::{DbDeviceRepo, DbSelectionPolicyRepo, SelectionPolicy}};
use db::{Database, SqlTranslator, Value};
use of_model::idp::model::Id;
use serde_json::Value as JsonValue;
use storage_service::DatabaseRuntime;
use tokio_util::sync::CancellationToken;
use unified_server::{SetupHost, StorageSyncDirection, StorageSyncRequest, UnifiedConfig};

struct Namespace { owner: String, application: Id }
impl of_model::storage::StorageNamespace for Namespace {
    fn user_sub(&self) -> &str { &self.owner }
    fn application_id(&self) -> Id { self.application }
}

pub async fn qualify(source: UnifiedConfig, target: UnifiedConfig, http: &reqwest::Client, login: &str, session: &JsonValue, owner: &str) -> std::io::Result<()> {
    let login_id: Id = login.parse().expect("real public login Client ID");
    let idp = Database::open(Path::new(&source.idp.data_dir).join("idp.redb")).expect("closed source IdP owner");
    let application = idp.client().translate_and_execute(&format!("SELECT application_id FROM clients WHERE client_id = '{login_id}'"), &SqlTranslator).await.expect("actual Application namespace");
    let application_id = application[0].rows[0].values[0].clone(); let db::Value::Text(ref application) = application_id else { panic!("actual Application UUID"); };
    let scope = Namespace { owner: owner.into(), application: Id::from_bytes(application.as_bytes().try_into().expect("application UUID")) };
    drop(idp);
    let device_db = Arc::new(Database::open(Path::new(&source.device.data_dir).join("devices.redb")).expect("closed Device owner"));
    let devices = DbDeviceRepo::new(device_db.client()).list().await.expect("approved persisted Devices");
    assert_eq!(devices.len(), 2);
    drop(device_db);
    let local = DatabaseRuntime::new(Path::new(&source.storage.data_dir).join("databases")).expect("actual source resource store");
    let (resource, sql) = local.create(&scope, Some("Native qualification".into())).expect("create real source database");
    sql.client().translate_and_execute(&format!("CREATE TABLE native_documents (id UUID PRIMARY KEY, body TEXT); INSERT INTO native_documents VALUES (CAST('{}' AS UUID), 'signed-user-native-transfer')", Id::now_v7()), &SqlTranslator).await.expect("write local SQL");
    let kv = local.open_kv_selected(&scope, resource.id).expect("source KV").expect("actual KV owner");
    kv.client().set("native/file-body", Value::Text("persisted native KV bytes".into()), None).await.expect("write actual local KV");
    drop(kv); drop(sql); drop(local);
    for config in [&source, &target] {
        let policy = Database::open(Path::new(&config.device.data_dir).join("devices.redb")).expect("closed persisted Device policy owner");
        let repo = DbSelectionPolicyRepo::new(policy.client());
        for device in &devices {
            repo.set_prevalidated(SelectionPolicy { device_id: device.id, owner_subject: scope.owner.clone(), application_id: Some(scope.application), selected_kind: Some("database".into()), selected_id: Some(resource.id.into()), admin_allowed: true }).await.expect("test fixture explicitly authorizes both existing Devices and resource");
        }
    }
    let from = SetupHost::prepare(source.clone()).expect("existing native source");
    let mut driver = from.sync_driver();
    let to = SetupHost::prepare(target.clone()).expect("existing native target");
    let mut remote_driver = to.sync_driver();
    let stop = CancellationToken::new();
    let source_task = tokio::spawn(from.serve(stop.clone()));
    let target_task = tokio::spawn(to.serve(stop.clone()));
    for owner in [&source.idp.api_public_uri, &source.device.api_public_uri, &source.storage.api_public_uri, &target.idp.api_public_uri, &target.device.api_public_uri, &target.storage.api_public_uri] {
        super::ready(http, owner).await;
    }
    let running = tokio::time::timeout(Duration::from_secs(30), driver.wait_for(|value| value.is_some())).await.expect("native source driver ready").expect("source channel").clone().expect("source driver");
    tokio::time::timeout(Duration::from_secs(30), remote_driver.wait_for(|value| value.is_some())).await.expect("native target driver ready").expect("target channel");
    let token = http.post(format!("{}/oauth2/token", source.idp.api_public_uri)).form(&[
        ("grant_type", "urn:ietf:params:oauth:grant-type:token-exchange"),
        ("subject_token", session["access_token"].as_str().expect("actual signed User token")),
        ("subject_token_type", "urn:ietf:params:oauth:token-type:access_token"),
        ("client_id", login), ("resource", "storage:databases"),
        ("authorization_details", r#"[{"type":"storage","actions":["read","write"]}]"#),
    ]).send().await.expect("signed User exchange").error_for_status().expect("approved database token exchange").json::<JsonValue>().await.expect("database token");
    let metadata = http.get(format!("{}/device", target.device.api_public_uri)).send().await.expect("live target Device metadata").error_for_status().expect("Device metadata status").json::<JsonValue>().await.expect("Device metadata");
    let request = StorageSyncRequest { target_endpoint_id: metadata["publicKey"].as_str().expect("actual target endpoint").into(), resource_id: resource.id.to_string(), kind: "database".into(), direction: StorageSyncDirection::Bidirectional };
    eprintln!("native qualification: run SQL + KV over iroh");
    loop {
        let result = tokio::time::timeout(Duration::from_secs(45), running.synchronize(
            token["access_token"].as_str().expect("real database grant"),
            request.clone(),
            async { Ok(serde_json::from_str(metadata["address"].as_str().expect("actual live iroh address")).expect("iroh address")) },
        )).await;
        eprintln!("native sync completed, result={:?}", result);
        match result {
            Ok(Ok(sync_result)) => { eprintln!("native sync completed, result={:?}", sync_result); break; }
            Ok(Err(error)) => { eprintln!("native sync error: {error:?}"); return Err(error); }
            Err(timeout) => { eprintln!("native sync timeout: {timeout:?}"); return Err(std::io::Error::other("native engine timeout")); }
        }
    }
    eprintln!("native qualification: completed, close and reopen destination");
    drop(running);
    stop.cancel();
    source_task.await.expect("source joins").expect("source closes");
    target_task.await.expect("target joins").expect("target closes");
    assert!(driver.borrow().is_none()); assert!(remote_driver.borrow().is_none());
    drop(driver); drop(remote_driver);
    let reopened = DatabaseRuntime::new(Path::new(&target.storage.data_dir).join("databases")).expect("reopen real destination");
    let sql = reopened.open_selected(&scope, resource.id).expect("destination SQL").expect("retained selected database");
    let values = sql.client().translate_and_execute("SELECT body FROM native_documents", &SqlTranslator).await.expect("read reopened native SQL");
    assert_eq!(values[0].rows[0].values, [Value::Text("signed-user-native-transfer".into())]);
    let kv = reopened.open_kv_selected(&scope, resource.id).expect("destination KV").expect("retained KV");
    assert_eq!(kv.client().get("native/file-body").await.expect("read reopened native KV"), Some(Value::Text("persisted native KV bytes".into())));
    Ok(())
}
