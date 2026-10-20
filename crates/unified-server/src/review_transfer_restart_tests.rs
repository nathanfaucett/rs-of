use std::{fs, net::SocketAddr, path::Path, sync::Arc, time::Duration};

use api::IndependentCreateIntent;
use idp_model::model::Id;
use management_service::replica::{DbDeviceRepo, DbSelectionPolicyRepo};
use reqwest::{Client, StatusCode};
use tokio_util::sync::CancellationToken;

pub(super) async fn qualify(
    root: &Path,
    addresses: &[SocketAddr],
    intent: &IndependentCreateIntent,
    http: &Client,
    management_token: &str,
) {
    let record: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("management.setup-host/worker.json")).expect("completed worker"),
    )
    .expect("worker JSON");
    let device: Id = serde_json::from_value(record["device"].clone()).expect("local Device");
    let old: Id = serde_json::from_value(record["user"].clone()).expect("original owner");

    let endpoint = record["endpoint"].as_str().expect("retained endpoint");
    let path = root.join("management/management.redb");
    let pending_id;
    {
        let engine = Arc::new(db::Database::open(&path).expect("stopped Management owner"));

        let devices = DbDeviceRepo::new(engine.clone());
        let policies = DbSelectionPolicyRepo::new(engine);
        let next = Id::now_v7().to_string();
        assert!(
            devices
                .transfer_owned(device, &old.to_string(), &next)
                .await
                .expect("transfer")
        );
        assert!(
            !policies
                .reconcile_owner_prevalidated(device, &next)
                .await
                .expect("learn departure")
        );
        pending_id = policies
            .pending_cleanup_id(device)
            .await
            .expect("cleanup ID")
            .expect("pending");
        assert!(
            devices
                .transfer_owned(device, &next, &old.to_string())
                .await
                .expect("exact original owner return")
        );
        assert!(
            !policies
                .reconcile_owner_prevalidated(device, &old.to_string())
                .await
                .expect("return cannot drop fence")
        );
    }
    for pending in [true, false] {
        let shutdown = CancellationToken::new();
        let mut idp = idp_server::AppConfig {
            data_dir: root.join("idp").to_string_lossy().into_owned(),
            ..Default::default()
        };
        idp.server.prefix = Some("/idp".into());
        let mut management = management_server::AppConfig {
            data_dir: root.join("management").to_string_lossy().into_owned(),
            ..Default::default()
        };
        management.server.prefix = Some("/management".into());
        let servers = [
            tokio::spawn(idp_server::serve_independent_create(
                Arc::new(idp),
                super::setup(root, "idp", addresses[0]),
                shutdown.child_token(),
            )),
            tokio::spawn(management_server::serve_independent_create(
                Arc::new(management),
                super::setup(root, "management", addresses[1]),
                shutdown.child_token(),
            )),
        ];
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        loop {
            let idp_ready = match http
                .get(format!("{}/setup/status", intent.idp))
                .send()
                .await
            {
                Ok(response) if response.status() == StatusCode::OK => response
                    .json::<serde_json::Value>()
                    .await
                    .is_ok_and(|status| status["ready"] == true),
                _ => false,
            };
            if let Ok(response) = http
                .get(format!("{}/devices/trusted", intent.management))
                .send()
                .await
            {
                if response.status() == StatusCode::UNAUTHORIZED && idp_ready {
                    break;
                }
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "restricted restart must restore authenticated router"
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        super::current_status(http, &intent.management, false).await;
        let token = super::storage_policy_token(root, http, intent).await;
        let policy: model::contract::SelectedResourcesResponse = http
            .get(format!(
                "{}/replication/devices/{endpoint}/selections",
                intent.management
            ))
            .bearer_auth(&token)
            .send()
            .await
            .expect("policy remains available")
            .error_for_status()
            .expect("authorized policy read")
            .json()
            .await
            .expect("policy");
        assert_eq!(policy.owner_subject, old.to_string());
        assert!(!policy.admin_allowed);
        assert!(policy.resources.is_empty());
        if pending {
            let exact = policy
                .pending_cleanup
                .expect("retirement survives stopped restart");
            assert_eq!(exact.cleanup_id, pending_id.to_string());
            assert_eq!(exact.old_owner_subject, exact.new_owner_subject);
            let cleanup = format!("{}/replication/owner-cleanup", intent.management);
            assert_eq!(
                http.post(&cleanup)
                    .json(&exact)
                    .send()
                    .await
                    .expect("unauthenticated cleanup")
                    .status(),
                StatusCode::UNAUTHORIZED
            );
            let mut stale = exact.clone();
            stale.cleanup_id = Id::now_v7().to_string();
            assert_eq!(
                http.post(&cleanup)
                    .bearer_auth(&token)
                    .json(&stale)
                    .send()
                    .await
                    .expect("stale cleanup")
                    .status(),
                StatusCode::FORBIDDEN
            );
            assert_eq!(
                http.post(&cleanup)
                    .bearer_auth(&token)
                    .json(&exact)
                    .send()
                    .await
                    .expect("exact retirement ack after restart")
                    .status(),
                StatusCode::NO_CONTENT
            );
            super::current_status(http, &intent.management, false).await;
        } else {
            assert!(policy.pending_cleanup.is_none());
            let restriction = format!("{}/devices/{device}/restriction", intent.management);
            let body = serde_json::json!({"adminAllowed": true});
            assert_eq!(
                http.put(&restriction)
                    .json(&body)
                    .send()
                    .await
                    .expect("unauthenticated mutation")
                    .status(),
                StatusCode::UNAUTHORIZED
            );
            assert_eq!(
                http.put(&restriction)
                    .bearer_auth(management_token)
                    .json(&body)
                    .send()
                    .await
                    .expect("ordinary authorized recovery after restricted restart")
                    .status(),
                StatusCode::NO_CONTENT
            );
            super::current_status(http, &intent.management, true).await;
        }
        shutdown.cancel();
        for server in servers {
            server
                .await
                .expect("join restarted owner")
                .expect("clean owner shutdown");
        }
    }
    let engine = Arc::new(db::Database::open(&path).expect("stopped recovered reopen"));
    let policies = DbSelectionPolicyRepo::new(engine);
    assert!(
        !policies
            .local_cleanup_pending(device)
            .await
            .expect("completed fence")
    );
    assert!(
        policies
            .get(device, &old.to_string(), Id::nil())
            .await
            .expect("policy")
            .expect("retained")
            .admin_allowed
    );
    assert!(
        policies
            .selected_resources_for_device(device)
            .await
            .expect("no resurrected selection")
            .is_empty()
    );
}
