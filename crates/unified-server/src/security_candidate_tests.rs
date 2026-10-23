use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use db::{Database, Row, Value};
use device_service::DeviceRepo;
use device_service::replica::DbDeviceRepo;
use idp_model::model::Id;
use ofdb_sql::{SyncMessage, SyncRole};

use super::{Pair, Poison, RecoveryAttack, public_jwk, sync_fixture, unvalidated_initiate};
use crate::{
    SecurityContext,
    security_sync_tests::{enroll, sql},
};

struct ExtraMember {
    member: Id,
    membership: Id,
    signer: Id,
}

async fn seed(database: &Arc<Database>) -> ExtraMember {
    let member = Id::now_v7();
    let key = Id::now_v7();
    let endpoint = iroh::SecretKey::generate().public().to_string();
    enroll(
        database.clone(),
        member,
        endpoint.parse().expect("fixture endpoint"),
        key,
    )
    .await;
    let Value::Uuid(signer) = sql(
        database,
        &format!("SELECT id FROM idp_replica_signers WHERE member_id = CAST('{member}' AS UUID)"),
    )
    .await[0]
        .rows[0]
        .values[0]
    else {
        panic!("fixture signer")
    };
    let Value::Uuid(membership) = sql(
        database,
        &format!("SELECT id FROM idp_replica_members WHERE member_id = CAST('{member}' AS UUID)"),
    )
    .await[0]
        .rows[0]
        .values[0]
    else {
        panic!("fixture membership")
    };
    ExtraMember {
        member,
        membership,
        signer,
    }
}

async fn seed_device(database: &Database) -> Id {
    let member = Id::now_v7();
    let endpoint = iroh::SecretKey::generate().public();
    let Value::Text(owner) =
        &sql(database, "SELECT owner_subject FROM devices LIMIT 1").await[0].rows[0].values[0]
    else {
        panic!("fixture owner")
    };
    sql(database, &format!("INSERT INTO devices (id, owner_subject, name, public_key, address, state, approved_at, created_at, updated_at) VALUES (CAST('{member}' AS UUID), '{owner}', 'extra', '{endpoint}', 'local', 1, 1, 1, 1)")).await;
    member
}

#[tokio::test]
async fn marker3_revalidates_semantic_recovery_and_preserves_local_contender() {
    tokio::time::timeout(Duration::from_secs(600), async {
        for contender in [false, true] {
            for outgoing in [true, false] {
                if contender && outgoing { continue; }
                for role in [SyncRole::Initiator, SyncRole::Responder] {
                    let mut pair = Pair::new().await;
                    let source_is_a = outgoing == matches!(role, SyncRole::Initiator);
                    let (source, target) = if source_is_a {
                        (pair.idp_a.clone(), pair.idp_b.clone())
                    } else {
                        (pair.idp_b.clone(), pair.idp_a.clone())
                    };
                    let extra = seed(&source).await;
                    sync_fixture(&source, &target).await;
                    let query = format!("SELECT * FROM idp_replica_signers WHERE id = CAST('{}' AS UUID)", extra.signer);
                    let before = sql(&target, &query).await[0].rows.clone();
                    if contender {
                        sql(&source, &format!("UPDATE idp_replica_signers SET expires_at = 9999999999 WHERE id = CAST('{}' AS UUID)", extra.signer)).await;
                    }
                    let statement = corruption(&extra, if contender { Attack::Endpoint } else { Attack::Kid });
                    let fired = Arc::new(AtomicBool::new(false));
                    pair.recovery = Some(RecoveryAttack {
                        source: if contender { target.clone() } else { source.clone() },
                        root: pair.root.clone(), row: extra.signer, variant: Poison::Current,
                        semantic: Some(("idp_replica_signers".into(), statement)),
                        before_payload: contender,
                        on_send: contender || outgoing, fired: fired.clone(),
                    });
                    pair.serve(matches!(role, SyncRole::Responder));
                    let connection = pair.connect().await;
                    let error = match role {
                        SyncRole::Initiator => pair.protocol_a.synchronize(&connection, SecurityContext::Idp).await,
                        SyncRole::Responder => unvalidated_initiate(&pair, &connection).await,
                    }.expect_err("late recovery candidate must reject");
                    assert!(fired.load(Ordering::SeqCst), "clean preflight reaches recovery {contender}/{outgoing}/{role:?}: {error}");
                    if matches!(role, SyncRole::Initiator) && (outgoing || contender) {
                        assert!(error.to_string().contains("invalid public security candidate"), "domain candidate gate: {error}");
                    }
                    if !outgoing && !contender && matches!(role, SyncRole::Initiator) {
                        assert!(error.to_string().contains("Unexpected or duplicate recovery snapshot"), "unsolicited incoming recovery is integrity rejection, not domain-gate proof: {error}");
                    }
                    assert_eq!(sql(&target, &query).await[0].rows, before, "invalid recovery batch rolls back");
                    if contender {
                        let device = sql(&target, &format!("SELECT endpoint_id FROM idp_replica_members WHERE id = CAST('{}' AS UUID)", extra.membership)).await;
                        let member = sql(&source, &format!("SELECT endpoint_id FROM idp_replica_members WHERE id = CAST('{}' AS UUID)", extra.membership)).await;
                        assert_ne!(device[0].rows[0].values[0], member[0].rows[0].values[0], "independent local contender survives rollback");
                    }
                    if outgoing {
                        assert!(!pair.received.lock().expect("wire frames").iter().any(|frame| matches!(postcard::from_bytes::<SyncMessage>(frame), Ok(SyncMessage::State(_) | SyncMessage::Changes(_)))), "late invalid recovery never disclosed");
                    }
                    drop((source, target));
                    pair.close().await;
                }
            }
        }
    }).await.expect("bounded genuine recovery/current-contender matrix");
}

#[tokio::test]
async fn approved_endpoints_without_idp_signers_exchange_and_device_revocation_denies() {
    tokio::time::timeout(Duration::from_secs(180), async {
        let mut pair = Pair::new().await;
        for database in [&pair.idp_a, &pair.idp_b] {
            sql(
                database,
                "DELETE FROM idp_replica_signers; DELETE FROM idp_replica_members",
            )
            .await;
            assert!(
                idp_service::replica::DbReplicaSignerRepo::new(database.client())
                    .trusted_signers(super::INSTALLATION, 10)
                    .await
                    .expect("no token-signing trust")
                    .is_empty()
            );
        }
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
        enroll(
            pair.idp_b.clone(),
            pair.member_a,
            pair.a.endpoint().id(),
            Id::now_v7(),
        )
        .await;
        assert!(
            idp_service::replica::DbReplicaSignerRepo::new(pair.idp_b.client())
                .trusted_signers(super::INSTALLATION, 10)
                .await
                .expect("IdP signing trust is independent of revoked Device")
                .iter()
                .any(|(member, _)| member.member_id == pair.member_a)
        );
        let before = pair
            .idp_b
            .sync_manifest()
            .await
            .expect("revoked receiver manifest");
        pair.protocol_a
            .synchronize(&connection, SecurityContext::Idp)
            .await
            .expect_err("valid signer cannot override Device revocation on existing connection");
        assert_eq!(
            pair.idp_b
                .sync_manifest()
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
        device_service::replica::up(&fork).await.expect("public schema");
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

#[tokio::test]
async fn marker3_persists_pending_conflicts_and_tombstones_without_trust() {
    tokio::time::timeout(Duration::from_secs(180), async {
        let mut pair = Pair::new().await;
        let revoked_signer = seed(&pair.idp_a).await;
        sql(&pair.idp_a, &format!("DELETE FROM idp_replica_members WHERE id = CAST('{}' AS UUID); UPDATE idp_replica_signers SET active_member_id = NULL, revoked_at = 2 WHERE id = CAST('{}' AS UUID)", revoked_signer.membership, revoked_signer.signer)).await;
        let pending = seed_device(&pair.management_a).await;
        let conflicted = seed_device(&pair.management_a).await;
        let fork = Database::open(pair.root.join("candidate-fork.redb")).expect("independent native contender");
        device_service::replica::up(&fork).await.expect("Management schema");
        super::sync_fixture_context(&pair.management_a, &fork, SecurityContext::Device).await;
        for database in [pair.management_a.as_ref(), &fork] {
            let endpoint = iroh::SecretKey::generate().public();
            sql(database, &format!("UPDATE devices SET public_key = '{endpoint}' WHERE id = CAST('{conflicted}' AS UUID)")).await;
        }
        super::sync_fixture_context(&pair.management_a, &fork, SecurityContext::Device).await;
        sql(&pair.management_a, &format!("DELETE FROM devices WHERE id = CAST('{pending}' AS UUID)")).await;
        pair.serve(true);
        let connection = pair.connect().await;
        pair.protocol_a.synchronize(&connection, SecurityContext::Device).await.expect("pending/conflicted/revoked history may persist");
        assert!(pair.management_b.client().row_conflicts("devices", &Row::new(vec![Value::Uuid(conflicted)])).await.expect("public conflict inspection").contains(&"public_key".to_string()));
        let Value::Text(endpoint) = &sql(&pair.management_b, &format!("SELECT public_key FROM devices WHERE id = CAST('{conflicted}' AS UUID)")).await[0].rows[0].values[0] else {
            panic!("fixture endpoint")
        };
        assert!(DbDeviceRepo::new(pair.management_b.clone().client()).find_approved_by_public_key(endpoint).await.is_err(), "persisted Device conflict cannot grant transport trust");
        assert!(sql(&pair.management_b, &format!("SELECT * FROM devices WHERE id = CAST('{pending}' AS UUID)")).await[0].rows.is_empty(), "tombstone retained");
        pair.protocol_a.synchronize(&connection, SecurityContext::Idp).await.expect("revoked IdP history persists independently");
        assert_eq!(sql(&pair.idp_b, &format!("SELECT revoked_at FROM idp_replica_signers WHERE id = CAST('{}' AS UUID)", revoked_signer.signer)).await[0].rows[0].values[0], Value::Integer(2));
        assert!(!idp_service::replica::DbReplicaSignerRepo::new(pair.idp_b.client()).trusted_signers(super::INSTALLATION, 10).await.expect("current signer trust").iter().any(|(member, _)| member.member_id == revoked_signer.member));
        drop(fork);
        pair.close().await;
    }).await.expect("bounded genuine Management conflict/tombstone exchange");
}

#[tokio::test]
async fn marker3_rejects_correlated_rebinding_of_authenticated_session_peer() {
    tokio::time::timeout(Duration::from_secs(240), async {
        for endpoint in [false, true] {
            for role in [SyncRole::Initiator, SyncRole::Responder] {
                let mut pair = Pair::new().await;
                let (source, target, member) = match role {
                    SyncRole::Initiator => (pair.management_b.clone(), pair.management_a.clone(), pair.member_b),
                    SyncRole::Responder => (pair.management_a.clone(), pair.management_b.clone(), pair.member_a),
                };
                let query = format!("SELECT id, owner_subject, public_key FROM devices WHERE id = CAST('{member}' AS UUID)");
                let before = sql(&target, &query).await[0].rows.clone();
                if endpoint {
                    let replacement = iroh::SecretKey::generate().public();
                    sql(&source, &format!("UPDATE devices SET public_key = '{replacement}' WHERE id = CAST('{member}' AS UUID)")).await;
                } else {
                    let owner = Id::now_v7();
                    sql(&source, &format!("UPDATE devices SET owner_subject = '{owner}' WHERE id = CAST('{member}' AS UUID)")).await;
                }
                let source_before = source.sync_manifest().await.expect("source manifest");
                pair.serve(matches!(role, SyncRole::Responder));
                let connection = pair.connect().await;
                match role {
                    SyncRole::Initiator => pair.protocol_a.synchronize(&connection, SecurityContext::Device).await,
                    SyncRole::Responder => super::unvalidated_initiate_context(&pair, &connection, SecurityContext::Device).await,
                }.expect_err("correlated rows cannot rebind authenticated session evidence");
                assert_eq!(sql(&target, &query).await[0].rows, before, "pinned Device batch rolls back");
                assert_eq!(source.sync_manifest().await.expect("unchanged source"), source_before);
                drop((source, target));
                pair.close().await;
            }
        }
    }).await.expect("bounded genuine pinned session binding matrix");
}

#[tokio::test]
async fn management_rejects_overwritten_invalid_device_history_before_export_or_apply() {
    tokio::time::timeout(Duration::from_secs(240), async {
        for deleted in [false, true] {
            for role in [SyncRole::Initiator, SyncRole::Responder] {
                let mut pair = Pair::new().await;
                let (source, target) = match role {
                    SyncRole::Initiator => (pair.management_b.clone(), pair.management_a.clone()),
                    SyncRole::Responder => (pair.management_a.clone(), pair.management_b.clone()),
                };
                let member = seed_device(&source).await;
                sql(&source, &format!("UPDATE devices SET state = 'invalid-historical-state' WHERE id = CAST('{member}' AS UUID); UPDATE devices SET state = 1 WHERE id = CAST('{member}' AS UUID)")).await;
                if deleted {
                    sql(&source, &format!("DELETE FROM devices WHERE id = CAST('{member}' AS UUID)")).await;
                }
                let before = target.sync_manifest().await.expect("pre-import Management manifest");
                pair.serve(matches!(role, SyncRole::Responder));
                let connection = pair.connect().await;
                match role {
                    SyncRole::Initiator => pair.protocol_a.synchronize(&connection, SecurityContext::Device).await,
                    SyncRole::Responder => super::unvalidated_initiate_context(&pair, &connection, SecurityContext::Device).await,
                }.expect_err("overwriting or deleting a row cannot hide invalid public Device history");
                assert_eq!(target.sync_manifest().await.expect("unchanged target"), before);
                drop((source, target));
                pair.close().await;
            }
        }
    }).await.expect("bounded Management historical Device validation");
}

#[derive(Clone, Copy, Debug)]
enum Attack {
    Kid,
    ActiveMember,
    Endpoint,
    HistoricalIssuer,
}

fn corruption(extra: &ExtraMember, attack: Attack) -> String {
    match attack {
        Attack::Kid => format!(
            "UPDATE idp_replica_signers SET public_jwk = '{}' WHERE id = CAST('{}' AS UUID)",
            public_jwk(&Id::now_v7().to_string()),
            extra.signer
        ),
        Attack::ActiveMember => format!(
            "UPDATE idp_replica_signers SET active_member_id = CAST('{}' AS UUID) WHERE id = CAST('{}' AS UUID)",
            Id::now_v7(),
            extra.signer
        ),
        Attack::Endpoint => format!(
            "UPDATE idp_replica_members SET endpoint_id = 'not-an-endpoint' WHERE id = CAST('{}' AS UUID)",
            extra.membership
        ),
        Attack::HistoricalIssuer => format!(
            "UPDATE idp_replica_members SET issuer = 'https://foreign.example' WHERE id = CAST('{}' AS UUID); UPDATE idp_replica_members SET issuer = '{}' WHERE id = CAST('{}' AS UUID)",
            extra.membership,
            super::INSTALLATION,
            extra.membership
        ),
    }
}

async fn corrupt(database: &Database, extra: &ExtraMember, attack: Attack) {
    sql(database, &corruption(extra, attack)).await;
}

#[tokio::test]
async fn marker3_rejects_semantic_candidates_in_both_directions_and_roles() {
    reject_candidates(false).await;
}

#[tokio::test]
async fn marker3_rejects_semantic_candidates_in_known_row_deltas() {
    reject_candidates(true).await;
}

async fn reject_candidates(known: bool) {
    tokio::time::timeout(Duration::from_secs(600), async {
        for attack in [
            Attack::Kid,
            Attack::ActiveMember,
            Attack::Endpoint,
            Attack::HistoricalIssuer,
        ] {
            for outgoing in [true, false] {
                for role in [SyncRole::Initiator, SyncRole::Responder] {
                    let mut pair = Pair::new().await;
                    let source_is_a = outgoing == matches!(role, SyncRole::Initiator);
                    let (source, target) = if source_is_a {
                        (pair.idp_a.clone(), pair.idp_b.clone())
                    } else {
                        (pair.idp_b.clone(), pair.idp_a.clone())
                    };
                    let extra = seed(&source).await;
                    if known {
                        sync_fixture(&source, &target).await;
                    }
                    let offending_table = match attack {
                        Attack::Kid | Attack::ActiveMember => "idp_replica_signers",
                        Attack::Endpoint => "idp_replica_members",
                        Attack::HistoricalIssuer => "idp_replica_members",
                    };
                    let offending_row = match offending_table {
                        "idp_replica_signers" => extra.signer,
                        "idp_replica_members" => extra.membership,
                        _ => extra.member,
                    };
                    let query = format!(
                        "SELECT * FROM {offending_table} WHERE id = CAST('{offending_row}' AS UUID)"
                    );
                    let before = sql(&target, &query).await[0].rows.clone();
                    corrupt(&source, &extra, attack).await;
                    let source_before = source.sync_manifest().await.expect("source manifest");
                    let baseline = sql(&target, "SELECT * FROM idp_replica_signers").await[0]
                        .rows
                        .clone();
                    pair.serve(matches!(role, SyncRole::Responder));
                    let connection = pair.connect().await;
                    let result = match role {
                        SyncRole::Initiator => {
                            pair.protocol_a
                                .synchronize(&connection, SecurityContext::Idp)
                                .await
                        }
                        SyncRole::Responder => unvalidated_initiate(&pair, &connection).await,
                    };
                    let error =
                        result.expect_err("semantic invalidity must reject at actual export/apply");
                    assert!(!error.to_string().contains("https://foreign.example"));
                    assert_eq!(
                        source.sync_manifest().await.expect("unchanged source"),
                        source_before
                    );
                    if outgoing {
                        assert_eq!(
                            sql(&target, "SELECT * FROM idp_replica_signers").await[0].rows,
                            baseline
                        );
                        assert!(
                            !pair.received.lock().expect("wire frames").iter().any(
                                |frame| matches!(
                                    postcard::from_bytes::<SyncMessage>(frame),
                                    Ok(SyncMessage::State(_) | SyncMessage::Changes(_))
                                )
                            ),
                            "no invalid history disclosure to even an unvalidated peer"
                        );
                    } else {
                        assert_eq!(
                            sql(&target, &query).await[0].rows,
                            before,
                            "offending batch rolls back; earlier legitimate pending rows may remain"
                        );
                    }
                    drop((source, target));
                    pair.close().await;
                }
            }
        }
    })
    .await
    .expect("bounded genuine semantic candidate matrix");
}
