use std::sync::Arc;

use db::{Database, SqlTranslator};
use device_service::DeviceRepo;
use device_service::replica::DbDeviceRepo;
use device_service::replica::DbSelectionPolicyRepo;
use device_service::replica::SelectionPolicy;
use idp_model::model::Id;
use idp_service::{PasswordConfig, replica::DbUserRepo, repo::UserRepo};
use model::contract::SelectedResource;

use super::{DeviceReplicationPolicy, LocalReplicationAuthorizer};

#[tokio::test]
async fn native_approved_unselected_devices_have_no_resource_access() {
    native_selection_fixture(false).await;
}

#[tokio::test]
async fn native_selected_resource_persistence() {
    native_selection_fixture(true).await;
}

async fn native_selection_fixture(select: bool) {
    let root = std::env::temp_dir().join(format!("of03-native-selection-{}", Id::now_v7()));
    std::fs::create_dir_all(&root).expect("isolated Node stores");
    let idp = Arc::new(Database::open(root.join("idp.redb")).expect("IdP store"));
    let management =
        Arc::new(Database::open(root.join("management.redb")).expect("Management store"));
    idp_model::replica::up(&idp).await.expect("IdP schema");
    device_service::replica::up(&management)
        .await
        .expect("Management schema");
    let owner = Id::now_v7();
    let local = iroh::SecretKey::generate().public();
    let remote = iroh::SecretKey::generate().public();
    let devices = [Id::now_v7(), Id::now_v7()];
    idp.client().translate_and_execute(&format!("INSERT INTO users (id, handle, name, created_at, updated_at) VALUES (CAST('{owner}' AS UUID), 'owner', 'Owner', 1, 1)"), &SqlTranslator).await.expect("persist owner");
    for (device, endpoint) in devices.into_iter().zip([local, remote]) {
        management.client().translate_and_execute(&format!("INSERT INTO devices (id, owner_subject, name, public_key, address, state, approved_at, created_at, updated_at) VALUES (CAST('{device}' AS UUID), '{owner}', 'node', '{endpoint}', 'local', 1, 1, 1, 1)"), &SqlTranslator).await.expect("persist approved Device");
        DbSelectionPolicyRepo::new(management.clone().client())
            .set_prevalidated(SelectionPolicy {
                device_id: device,
                owner_subject: owner.to_string(),
                application_id: None,
                selected_kind: None,
                selected_id: None,
                admin_allowed: true,
            })
            .await
            .expect("persist unselected policy");
    }
    let authorizer = LocalReplicationAuthorizer::new({
        let idp = idp.clone();
        let management = management.clone();
        move |endpoint: iroh::EndpointId| {
            let idp = idp.clone();
            let management = management.clone();
            async move {
                let device = DbDeviceRepo::new(management.clone().client())
                    .find_approved_by_public_key(&endpoint.to_string())
                    .await
                    .map_err(|error| error.to_string())?
                    .ok_or("unknown or unapproved Device")?;
                let owner_id = device.owner_subject.parse().map_err(|_| "invalid owner")?;
                let owner_active = DbUserRepo::new(idp.client(), PasswordConfig::default())
                    .find_user_by_id(owner_id)
                    .await
                    .map_err(|error| error.to_string())?
                    .is_some();
                let policies = DbSelectionPolicyRepo::new(management.client());
                let policy = policies
                    .get(device.id, &device.owner_subject, Id::nil())
                    .await
                    .map_err(|error| error.to_string())?
                    .ok_or("missing local policy")?;
                let resources = policies
                    .selected_resources_for_device(device.id)
                    .await
                    .map_err(|error| error.to_string())?
                    .into_iter()
                    .map(|resource| SelectedResource {
                        owner_subject: resource.owner_subject,
                        application_id: resource.application_id.to_string(),
                        kind: resource.kind,
                        resource_id: resource.resource_id.to_string(),
                    })
                    .collect();
                Ok(DeviceReplicationPolicy {
                    device_id: device.id,
                    endpoint_id: endpoint,
                    owner_subject: device.owner_subject,
                    state: device.state,
                    owner_active,
                    admin_allowed: policy.admin_allowed,
                    resources,
                })
            }
        }
    });
    for kind in ["database", "filesystem"] {
        let resource = SelectedResource {
            owner_subject: owner.to_string(),
            application_id: Id::now_v7().to_string(),
            kind: kind.into(),
            resource_id: Id::now_v7().to_string(),
        };

        assert!(
            !authorizer.authorize(local, remote, &resource).await,
            "approved pairing is not resource access"
        );
        if !select {
            continue;
        }
        for device_id in devices {
            DbSelectionPolicyRepo::new(management.clone().client())
                .set_prevalidated(SelectionPolicy {
                    device_id,
                    owner_subject: owner.to_string(),
                    application_id: Some(resource.application_id.parse().expect("application ID")),
                    selected_kind: Some(kind.into()),
                    selected_id: Some(resource.resource_id.parse().expect("resource ID")),
                    admin_allowed: true,
                })
                .await
                .expect("real stable selection must persist");
        }
        assert!(
            authorizer.authorize(local, remote, &resource).await,
            "real persisted matching selections authorize offline"
        );
        DbSelectionPolicyRepo::new(management.clone().client())
            .set_admin_allowed_prevalidated(devices[1], &owner.to_string(), false)
            .await
            .expect("learn remote restriction");
        assert!(
            !authorizer.authorize(local, remote, &resource).await,
            "next operation uses learned restriction"
        );
        DbSelectionPolicyRepo::new(management.clone().client())
            .set_admin_allowed_prevalidated(devices[1], &owner.to_string(), true)
            .await
            .expect("restore permission");
    }
    drop(authorizer);
    drop(idp);
    drop(management);
    std::fs::remove_dir_all(root).expect("cleanup isolated stores");
}

#[tokio::test]
async fn callback_cleanup_preserves_exact_binding_cancellation_and_remote_denial() {
    use std::sync::{
        Mutex, OnceLock,
        atomic::{AtomicUsize, Ordering},
    };

    use model::contract::{OwnerCopyCleanup, SelectedResourcesResponse};
    use storage_service::{DatabaseRuntime, ScopedFileSystemRuntime};
    use tokio_util::sync::CancellationToken;

    for scenario in [
        "transfer",
        "cleanup-id",
        "binding",
        "after-ack",
        "pending",
        "cancel",
        "stopped",
        "remote",
        "revoked",
        "not-ready",
    ] {
        let root = std::env::temp_dir().join(format!("of-shared-cleanup-{}", Id::now_v7()));
        let endpoint = iroh::SecretKey::generate().public();
        let pending = OwnerCopyCleanup {
            cleanup_id: Id::now_v7().to_string(),
            device_id: Id::now_v7().to_string(),
            endpoint_id: endpoint.to_string(),
            old_owner_subject: Id::now_v7().to_string(),
            new_owner_subject: Id::now_v7().to_string(),
        };
        let cancellation = CancellationToken::new();
        let copies = Arc::new(OnceLock::new());
        let weak = Arc::downgrade(&copies);
        let calls = Arc::new(AtomicUsize::new(0));
        let acknowledgements = Arc::new(Mutex::new(Vec::new()));
        let authorizer = LocalReplicationAuthorizer::from_callbacks(
            {
                let pending = pending.clone();
                let calls = calls.clone();
                let cancellation = cancellation.clone();
                move |_| {
                    let call = calls.fetch_add(1, Ordering::SeqCst);
                    let mut current = pending.clone();
                    let cancellation = cancellation.clone();
                    async move {
                        if scenario == "revoked" {
                            return Err("Device revoked".into());
                        }
                        if call == 1 {
                            if scenario == "cleanup-id" {
                                current.cleanup_id = Id::now_v7().to_string();
                            }
                            if scenario == "binding" {
                                current.new_owner_subject = Id::now_v7().to_string();
                            }
                            if scenario == "cancel" {
                                cancellation.cancel();
                            }
                        }
                        if call == 2 && scenario == "after-ack" {
                            current.new_owner_subject = Id::now_v7().to_string();
                        }
                        Ok(SelectedResourcesResponse {
                            device_id: current.device_id.clone(),
                            endpoint_id: current.endpoint_id.clone(),
                            owner_subject: current.new_owner_subject.clone(),
                            pending_cleanup: (call < 2 || scenario == "pending").then_some(current),
                            admin_allowed: true,
                            resources: vec![],
                        })
                    }
                }
            },
            {
                let acknowledgements = acknowledgements.clone();
                move |pending| {
                    acknowledgements
                        .lock()
                        .expect("acknowledgements")
                        .push(pending);
                    async { Ok(()) }
                }
            },
            Some((
                if scenario == "remote" {
                    iroh::SecretKey::generate().public()
                } else {
                    endpoint
                },
                cancellation.clone(),
            )),
            move || {
                weak.upgrade()
                    .ok_or("Storage runtime was released")?
                    .get()
                    .cloned()
                    .ok_or_else(|| "Storage cleanup is not ready".into())
            },
        );
        // The authorizer is built before the runtimes without retaining their owner.
        if scenario != "not-ready" {
            let retirement = crate::StorageCopyRetirement::new(
                Arc::new(DatabaseRuntime::new(root.join("sql")).expect("database runtime")),
                Arc::new(
                    ScopedFileSystemRuntime::new(root.join("fs"), endpoint)
                        .expect("filesystem runtime"),
                ),
            );
            assert!(copies.set(retirement).is_ok());
        }
        if scenario == "stopped" {
            cancellation.cancel();
        }
        let result = authorizer.local_owner(endpoint).await;
        if scenario == "transfer" {
            assert_eq!(
                result.expect("shared local cleanup"),
                pending.new_owner_subject
            );
            assert_eq!(
                *acknowledgements.lock().expect("acknowledgements"),
                vec![pending]
            );
            assert_eq!(calls.load(Ordering::SeqCst), 3);
        } else {
            assert!(result.is_err(), "{scenario}");
            let expected = usize::from(matches!(scenario, "after-ack" | "pending"));
            assert_eq!(
                acknowledgements.lock().expect("acknowledgements").len(),
                expected,
                "{scenario}"
            );
        }
        assert_eq!(Arc::strong_count(&copies), 1, "no runtime ownership cycle");
        drop(copies);
        assert!(authorizer.local_owner(endpoint).await.is_err());
        drop(authorizer);
        if root.exists() {
            std::fs::remove_dir_all(root).expect("remove cleanup fixture");
        }
    }
}
