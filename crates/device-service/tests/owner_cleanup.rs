#![cfg(feature = "replica")]

use std::{fs, sync::Arc};

use db::Database;
use device_service::replica::{DbSelectionPolicyRepo, SelectionPolicy, up};
use idp_model::model::Id;

#[tokio::test]
async fn stale_cleanup_cannot_clear_current_binding_and_exact_retry_stays_restricted() {
    let root = std::env::temp_dir().join(format!("independent-transfer-{}", Id::now_v7()));
    let path = root.join("devices.redb");
    fs::create_dir_all(&root).expect("isolated Management root");
    let device = Id::now_v7();
    let old = Id::now_v7().to_string();
    let next = Id::now_v7().to_string();
    let third = Id::now_v7().to_string();
    let endpoint = iroh::SecretKey::generate().public().to_string();
    let cleanup_id;
    {
        let engine = Arc::new(Database::open(&path).expect("Management owner"));
        up(&engine).await.expect("Management schema");
        engine.client().translate_and_execute(&format!("INSERT INTO devices (id, owner_subject, name, public_key, address, state, approved_at, revoked_at, created_at, updated_at) VALUES (CAST('{device}' AS UUID), '{next}', 'transfer', '{endpoint}', '', 1, 1, NULL, 1, 1)"), &db::SqlTranslator).await.expect("current Management binding");
        let policies = DbSelectionPolicyRepo::new(engine.clone().client());
        policies
            .set_prevalidated(SelectionPolicy {
                device_id: device,
                owner_subject: old.clone(),
                application_id: None,
                selected_kind: None,
                selected_id: None,
                admin_allowed: true,
            })
            .await
            .expect("old policy");
        assert!(
            !policies
                .reconcile_owner_prevalidated(device, &next)
                .await
                .expect("learn transfer")
        );
        cleanup_id = policies
            .pending_cleanup_id(device)
            .await
            .expect("cleanup obligation")
            .expect("durable cleanup ID");
        engine.client().translate_and_execute(&format!("UPDATE devices SET owner_subject = '{third}' WHERE id = CAST('{device}' AS UUID)"), &db::SqlTranslator).await.expect("owner changes during Storage cleanup");
        assert!(
            !policies
                .complete_owner_cleanup_bound(device, &old, &next, &endpoint, cleanup_id)
                .await
                .expect("stale ack denied")
        );
        assert!(
            policies
                .local_cleanup_pending(device)
                .await
                .expect("fence retained")
        );
        assert!(
            !policies
                .complete_owner_cleanup_bound(
                    device,
                    &old,
                    &third,
                    &iroh::SecretKey::generate().public().to_string(),
                    cleanup_id,
                )
                .await
                .expect("wrong endpoint denied")
        );
    }
    {
        let engine = Arc::new(Database::open(&path).expect("stopped offline Management reopen"));
        let policies = DbSelectionPolicyRepo::new(engine.client());
        assert_eq!(
            policies
                .pending_cleanup_owner(device)
                .await
                .expect("durable intent"),
            Some(old.clone())
        );
        assert!(
            policies
                .complete_owner_cleanup_bound(device, &old, &third, &endpoint, cleanup_id)
                .await
                .expect("current exact acknowledgement")
        );
        assert!(
            policies
                .complete_owner_cleanup_bound(device, &old, &third, &endpoint, cleanup_id)
                .await
                .expect("lost response exact retry")
        );
        assert!(
            !policies
                .local_cleanup_pending(device)
                .await
                .expect("cleanup complete")
        );
        let policy = policies
            .get(device, &third, Id::nil())
            .await
            .expect("new policy")
            .expect("retained policy");
        assert!(
            !policy.admin_allowed,
            "cleanup never approves or restores selections"
        );
        assert!(
            policies
                .selected_resources_for_device(device)
                .await
                .expect("empty selection")
                .is_empty()
        );
    }
    {
        let engine = Arc::new(Database::open(&path).expect("owner-cycle reopen"));
        let policies = DbSelectionPolicyRepo::new(engine.clone().client());
        for owner in [&old, &third] {
            engine.client().translate_and_execute(&format!("UPDATE devices SET owner_subject = '{owner}' WHERE id = CAST('{device}' AS UUID)"), &db::SqlTranslator).await.expect("later owner cycle");
            assert!(
                !policies
                    .reconcile_owner_prevalidated(device, owner)
                    .await
                    .expect("new obligation")
            );
            let current_id = policies
                .pending_cleanup_id(device)
                .await
                .expect("current cleanup ID")
                .expect("pending");
            assert_ne!(
                current_id, cleanup_id,
                "separate transfers cannot reuse acknowledgement evidence"
            );
            if owner == &third {
                assert!(
                    !policies
                        .complete_owner_cleanup_bound(device, &old, &third, &endpoint, cleanup_id)
                        .await
                        .expect("old tuple replay denied")
                );
                assert!(
                    policies
                        .local_cleanup_pending(device)
                        .await
                        .expect("replay retains new fence")
                );
            } else {
                assert!(
                    policies
                        .complete_owner_cleanup_bound(device, &third, &old, &endpoint, current_id)
                        .await
                        .expect("reverse transfer completion")
                );
            }
        }
    }
    fs::remove_dir_all(root).expect("remove isolated fixture");
}

#[tokio::test]
async fn original_owner_return_keeps_retirement_pending_after_stopped_reopen() {
    let root = std::env::temp_dir().join(format!("original-owner-return-{}", Id::now_v7()));
    fs::create_dir_all(&root).expect("isolated owner store");
    let path = root.join("devices.redb");
    let device = Id::now_v7();
    let old = Id::now_v7().to_string();
    let next = Id::now_v7().to_string();
    let endpoint = iroh::SecretKey::generate().public().to_string();
    let first_id;
    {
        let engine = Arc::new(Database::open(&path).expect("Management owner"));
        up(&engine).await.expect("schema");
        engine.client().translate_and_execute(&format!("INSERT INTO devices (id, owner_subject, name, public_key, address, state, approved_at, created_at, updated_at) VALUES (CAST('{device}' AS UUID), '{next}', 'cycle', '{endpoint}', '', 1, 1, 1, 1)"), &db::SqlTranslator).await.expect("current endpoint");
        let policies = DbSelectionPolicyRepo::new(engine.clone().client());
        policies
            .set_prevalidated(SelectionPolicy {
                device_id: device,
                owner_subject: old.clone(),
                application_id: Some(Id::now_v7()),
                selected_kind: Some("database".into()),
                selected_id: Some(Id::now_v7()),
                admin_allowed: true,
            })
            .await
            .expect("original selected policy");
        assert!(
            !policies
                .reconcile_owner_prevalidated(device, &next)
                .await
                .expect("transfer")
        );
        first_id = policies
            .pending_cleanup_id(device)
            .await
            .expect("intent")
            .expect("pending");
        engine
            .client()
            .translate_and_execute(
                &format!(
                    "UPDATE devices SET owner_subject = '{old}' WHERE id = CAST('{device}' AS UUID)"
                ),
                &db::SqlTranslator,
            )
            .await
            .expect("original owner returns before cleanup");
        assert!(
            !policies
                .reconcile_owner_prevalidated(device, &old)
                .await
                .expect("return keeps fence")
        );
    }
    {
        let engine = Arc::new(Database::open(&path).expect("stopped pending reopen"));
        let policies = DbSelectionPolicyRepo::new(engine.clone().client());
        assert_eq!(
            policies
                .pending_cleanup_id(device)
                .await
                .expect("retained ID"),
            Some(first_id)
        );
        assert!(
            !policies
                .complete_owner_cleanup_bound(device, &old, &next, &endpoint, first_id)
                .await
                .expect("stale target")
        );
        assert!(
            policies
                .local_cleanup_pending(device)
                .await
                .expect("fence remains")
        );
        assert!(
            policies
                .complete_owner_cleanup_bound(device, &old, &old, &endpoint, first_id)
                .await
                .expect("retire original copies even when owner returns")
        );
        let policy = policies
            .get(device, &old, Id::nil())
            .await
            .expect("policy")
            .expect("retained");
        assert!(!policy.admin_allowed);
        assert!(policy.selected_id.is_none());
        assert!(
            policies
                .selected_resources_for_device(device)
                .await
                .expect("no restored selection")
                .is_empty()
        );
        engine.client().translate_and_execute(&format!("UPDATE devices SET owner_subject = '{next}' WHERE id = CAST('{device}' AS UUID)"), &db::SqlTranslator).await.expect("separate transfer");
        assert!(
            !policies
                .reconcile_owner_prevalidated(device, &next)
                .await
                .expect("fresh obligation")
        );
        let fresh = policies
            .pending_cleanup_id(device)
            .await
            .expect("fresh ID")
            .expect("pending");
        assert_ne!(fresh, first_id);
        engine
            .client()
            .translate_and_execute(
                &format!(
                    "UPDATE devices SET owner_subject = '{old}' WHERE id = CAST('{device}' AS UUID)"
                ),
                &db::SqlTranslator,
            )
            .await
            .expect("return again");
        assert!(
            !policies
                .reconcile_owner_prevalidated(device, &old)
                .await
                .expect("keep fresh fence")
        );
        assert!(
            !policies
                .complete_owner_cleanup_bound(device, &old, &old, &endpoint, first_id)
                .await
                .expect("same-owner stale replay denied")
        );
        assert!(
            policies
                .local_cleanup_pending(device)
                .await
                .expect("replay cannot drop fence")
        );
        assert!(
            policies
                .complete_owner_cleanup_bound(device, &old, &old, &endpoint, fresh)
                .await
                .expect("fresh same-owner acknowledgement")
        );
    }
    {
        let engine = Arc::new(Database::open(&path).expect("stopped completed reopen"));
        let policies = DbSelectionPolicyRepo::new(engine.client());
        assert!(
            !policies
                .local_cleanup_pending(device)
                .await
                .expect("durable completion")
        );
        assert!(
            !policies
                .get(device, &old, Id::nil())
                .await
                .expect("policy")
                .expect("retained")
                .admin_allowed
        );
        assert!(
            policies
                .selected_resources_for_device(device)
                .await
                .expect("empty after reopen")
                .is_empty()
        );
    }
    fs::remove_dir_all(root).expect("isolated fixture cleanup");
}
