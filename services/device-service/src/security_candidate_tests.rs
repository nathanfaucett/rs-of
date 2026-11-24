#[cfg(feature = "ofdb")]
#[cfg(feature = "ofdb")]
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[cfg(feature = "ofdb")]
use ofdb_sql::Database;

use ofdb_sql::Row;

use of_model::id::Id;
use ofdb_sql::Value;
use ofdb_sql::{AutomergeRowCodec, Engine, InMemoryKernel, Kernel, KernelTransaction, RowCodec};
use ofdb_sql::{SyncKey, SyncRole};
use ofdb_sql_history::{SyncMessage, apply_sync_state_batch_for, export_sync_state_for};

use super::{Pair, Poison, RecoveryAttack, sync_fixture, unvalidated_initiate};
use crate::{
    device_repo::DeviceRepo,
    ofdb::DbDeviceRepo,
    security_policy::SecurityContext,
    security_protocol::test_fixtures::{insert_device, sql},
};

async fn seed_device(database: &Database) -> Id {
    let endpoint = iroh::SecretKey::generate().public();
    let Value::Text(owner) =
        &sql(database, "SELECT owner_subject FROM devices LIMIT 1").await[0].rows[0].values[0]
    else {
        panic!("fixture owner")
    };
    insert_device(database, owner, endpoint, "extra").await
}

#[tokio::test]
async fn approved_endpoints_without_idp_signers_exchange_and_device_revocation_denies() {
    tokio::time::timeout(Duration::from_secs(180), async {
        let mut pair = Pair::new().await;
        pair.serve(true);
        let connection = pair.connect().await;
        pair.protocol_a
            .synchronize(&connection, SecurityContext::Idp)
            .await
            .expect("approved endpoints with exact policy need no IdP signer or membership");
        sql(
            &pair.management_b,
            &format!(
                "UPDATE devices SET revoked_at = 2 WHERE id = CAST('{}' AS UUID)",
                pair.member_a
            ),
        )
        .await;
        let before = pair
            .idp_b
            .history_manifest()
            .await
            .expect("revoked receiver manifest");
        pair.protocol_a
            .synchronize(&connection, SecurityContext::Idp)
            .await
            .expect_err("Device revocation denies on existing connection");
        assert_eq!(
            pair.idp_b
                .history_manifest()
                .await
                .expect("unchanged receiver"),
            before
        );
        pair.close().await;
    })
    .await
    .expect("bounded signer-independent transport admission");
}

#[tokio::test]
async fn approved_devices_reject_conflicted_public_device_bindings() {
    tokio::time::timeout(Duration::from_secs(180), async {
        let pair = Pair::new().await;
        let mut bindings = Vec::new();
        for column in ["owner_subject", "name", "address"] {
            bindings.push((column, seed_device(&pair.management_a).await));
        }
        let fork = Database::open(pair.root.join("device-binding-fork.redb"))
            .expect("independent Device contender");
        crate::ofdb::schema::up(&fork).await.expect("public schema");
        super::sync_fixture_context(&pair.management_a, &fork, SecurityContext::Device).await;
        for (column, member) in &bindings {
            for database in [pair.management_a.as_ref(), &fork] {
                let value = Id::now_v7();
                sql(database, &format!("UPDATE devices SET {column} = '{value}' WHERE id = CAST('{member}' AS UUID)")).await;
            }
        }
        super::sync_fixture_context(&pair.management_a, &fork, SecurityContext::Device).await;
        for database in [pair.management_a.as_ref(), &fork] {
            for (column, member) in &bindings {
                assert!(database.client().row_conflicts("devices", &Row::new(vec![Value::Uuid(*member)])).await.expect("actual public Device conflict").contains(&column.to_string()));
            }
            let devices = DbDeviceRepo::new(Arc::new(database.clone()).client());
            assert!(devices.find_approved_by_public_key(&pair.a.endpoint().id().to_string()).await.expect("unrelated conflict-clear Device").is_some());
            for (_, member) in &bindings {
                let Value::Text(endpoint) = &sql(database, &format!("SELECT public_key FROM devices WHERE id = CAST('{member}' AS UUID)")).await[0].rows[0].values[0] else {
                    panic!("fixture endpoint")
                };
                assert!(devices.find_approved_by_public_key(endpoint).await.is_err(), "conflicted public Device binding cannot grant transport trust");
            }
        }
        drop(fork);
        pair.close().await;
    }).await.expect("bounded Device binding conflict regression");
}

async fn inject_overwritten_device_state(source: &Database, member: Id) {
    let kernel = InMemoryKernel::new();
    let engine = Engine::new(kernel.clone(), AutomergeRowCodec::new());
    apply_sync_state_batch_for(
        &engine,
        source.export_history().await.expect("source history"),
    )
    .await
    .expect("copy Device history into raw fixture");
    let result = sql(
        source,
        &format!("SELECT * FROM devices WHERE id = CAST('{member}' AS UUID)"),
    )
    .await;
    let state = result[0]
        .columns
        .iter()
        .position(|column| column.name == "state")
        .expect("Device state column");
    let mut row = result[0].rows[0].clone();
    let codec = AutomergeRowCodec::new();
    let mut transaction = kernel.transaction().await.expect("raw history transaction");
    // Typed SQL rejects this write; the raw codec models an unvalidated peer's retained history.
    row.values[state] = Value::Text("invalid-historical-state".into());
    codec
        .put_row(&mut transaction, "devices", member, row.clone())
        .await
        .expect("inject raw text state");
    assert_eq!(
        codec
            .get_row(&transaction, "devices", &member)
            .await
            .expect("raw injected row")
            .expect("Device row")
            .values[state],
        Value::Text("invalid-historical-state".into())
    );
    row.values[state] = Value::Integer(1);
    codec
        .put_row(&mut transaction, "devices", member, row)
        .await
        .expect("overwrite with valid current state");
    transaction
        .commit()
        .await
        .expect("commit overwritten corrupt history");
    let unit = export_sync_state_for(&engine)
        .await
        .expect("raw Device history")
        .into_iter()
        .find(|unit| {
            unit.key
                == SyncKey::Row {
                    table: "devices".into(),
                    row: member,
                }
        })
        .expect("injected Device history unit");
    source
        .apply_history(unit)
        .await
        .expect("retain corrupt history behind valid current projection");
}

#[tokio::test]
async fn management_rejects_overwritten_invalid_device_history_before_export_or_apply() {
    tokio::time::timeout(Duration::from_secs(240), async {
        for deleted in [false, true] {
            for outgoing in [true, false] {
                for role in [SyncRole::Initiator, SyncRole::Responder] {
                    let mut pair = Pair::new().await;
                    let source_is_a = outgoing == matches!(role, SyncRole::Initiator);
                    let (source, target) = if source_is_a {
                        (pair.management_a.clone(), pair.management_b.clone())
                    } else {
                        (pair.management_b.clone(), pair.management_a.clone())
                    };
                    let member = seed_device(&source).await;
                    inject_overwritten_device_state(&source, member).await;
                    if deleted {
                        sql(&source, &format!("DELETE FROM devices WHERE id = CAST('{member}' AS UUID)")).await;
                        assert!(sql(&source, &format!("SELECT state FROM devices WHERE id = CAST('{member}' AS UUID)")).await[0].rows.is_empty());
                    } else {
                        assert_eq!(sql(&source, &format!("SELECT state FROM devices WHERE id = CAST('{member}' AS UUID)")).await[0].rows[0].values,
                            vec![Value::Integer(1)]);
                    }
                    let before = target.history_manifest().await.expect("pre-import Management manifest");
                    let source_before = source.history_manifest().await.expect("corrupt source manifest");
                    pair.serve(matches!(role, SyncRole::Responder));
                    let connection = pair.connect().await;
                    let error = match role {
                        SyncRole::Initiator => pair.protocol_a.synchronize(&connection, SecurityContext::Device).await,
                        SyncRole::Responder => super::unvalidated_initiate_context(&pair, &connection, SecurityContext::Device).await,
                    }.expect_err("overwriting or deleting a row cannot hide invalid public Device history");
                    if outgoing && matches!(role, SyncRole::Initiator) {
                        assert!(error.to_string().contains("Invalid historical scoped value type"),
                            "historical type gate, not incidental integrity rejection: {deleted}/{outgoing}/{role:?}: {error}");
                    }
                    assert_eq!(target.history_manifest().await.expect("unchanged target"), before);
                    assert_eq!(source.history_manifest().await.expect("unchanged corrupt source"), source_before);
                    if outgoing {
                        assert!(!pair.received.lock().expect("wire frames").iter().any(|frame|
                            frame.windows(b"invalid-historical-state".len()).any(|bytes| bytes == b"invalid-historical-state")),
                            "invalid historical state is never disclosed");
                    }
                    drop((source, target));
                    pair.close().await;
                }
            }
        }
    }).await.expect("bounded Management historical Device validation");
}
