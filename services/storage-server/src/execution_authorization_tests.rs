use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex as StdMutex,
        atomic::{AtomicU64, AtomicUsize},
    },
    time::Duration,
};

use ofdb_sql::Database;

use device_service::device_repo::DeviceRepo;
use device_service::ofdb::device_repo::DbDeviceRepo;
use file_system::FileResponse;
use idp_service::{ofdb::DbUserRepo, password_config::PasswordConfig, repo::user_repo::UserRepo};
use iroh::{Endpoint, address_lookup::MemoryLookup, endpoint::presets};
use management_service::ofdb::permission_repo::DbPermissionRepo;
use management_service::ofdb::role_repo::DbRoleRepo;
use management_service::{LocalPermissions, ManagementService};
use of_model::id::Id;
use of_model::idp::contract::permission_evaluation::INSTALLATION_POLICY_ID;
use of_model::idp::contract::permission_evaluation::IdentityAction;
use of_model::idp::contract::permission_evaluation::IdentityResource;
use of_model::idp::contract::permission_evaluation::PermissionEvaluationRequest;
use of_model::idp::contract::permission_evaluation::PermissionSubject;
use of_model::idp::contract::permission_evaluation::PermissionTarget;
use tokio::sync::{Mutex, broadcast, mpsc, watch};

use super::{FrameAuthorizer, Inner, send_response, write_loop};

use crate::execution_test_support::{insert_device, insert_owner};

#[tokio::test]
async fn queued_response_observes_local_grant_revocation_before_authenticated_socket_write() {
    queued_write_fixture(false).await;
}

#[tokio::test]
async fn admitted_socket_write_finishes_then_next_write_observes_local_denial() {
    queued_write_fixture(true).await;
}

async fn queued_write_fixture(in_flight: bool) {
    tokio::time::timeout(Duration::from_secs(30), Box::pin(async {
        let lookup = MemoryLookup::new();
        let a = Endpoint::builder(presets::Minimal).alpns(vec![b"execution-policy".to_vec()]).address_lookup(lookup.clone()).bind().await.expect("bind A");
        let b = Endpoint::builder(presets::Minimal).address_lookup(lookup.clone()).bind().await.expect("bind B");
        lookup.add_endpoint_info(a.addr());
        let (outgoing, incoming) = tokio::join!(b.connect(a.id(), b"execution-policy"), async { a.accept().await.expect("incoming").accept().expect("accept").await });
        let outgoing = outgoing.expect("connect B");
        let incoming = incoming.expect("authenticate B");
        assert_eq!(incoming.remote_id(), b.id());
        let (mut peer_send, mut peer_recv) = outgoing.open_bi().await.expect("open stream");
        peer_send.write_all(&[1]).await.expect("make stream visible");
        let (send, _recv) = incoming.accept_bi().await.expect("accept stream");
        let root = std::env::temp_dir().join(format!("of03-queued-policy-{}", Id::now_v7()));
        std::fs::create_dir_all(&root).expect("create local stores");
        let idp = Arc::new(Database::open(root.join("idp.redb")).expect("open IdP"));
        let management = Arc::new(Database::open(root.join("management.redb")).expect("open Management"));
        db::ofdb::up(&idp.client()).await.expect("IdP schema");
        device_service::ofdb::schema::up(&management).await.expect("Device schema");
        db::ofdb::up(&management.client()).await.expect("Management policy schema");
        let actor = insert_owner(&idp).await;
        insert_device(&management, actor, incoming.remote_id()).await;
        management_service::provision_initial_administrator(&management.client(), actor, &[IdentityAction::DevicePairingRead, IdentityAction::DevicePairingUpdate, IdentityAction::IdpSecuritySync]).await.expect("explicit actor grant");
        let service = ManagementService::new(DbPermissionRepo::new(management.client()), DbRoleRepo::new(management.client()));
        let roles = service.list_user_roles(INSTALLATION_POLICY_ID, actor).await.expect("read provisioned role");
        let [role] = roles.as_slice() else { panic!("expected one provisioned role"); };
        let request = PermissionEvaluationRequest { request_id: Id::now_v7(), subject: PermissionSubject::User { id: actor }, action: IdentityAction::IdpSecuritySync, target: PermissionTarget::Installation { resource: IdentityResource::IdpSecurity } };
        for forged in [
            PermissionEvaluationRequest { subject: PermissionSubject::User { id: Id::now_v7() }, ..request.clone() },
            PermissionEvaluationRequest { action: IdentityAction::ManagementSecuritySync, target: PermissionTarget::Installation { resource: IdentityResource::ManagementSecurity }, ..request.clone() },
            PermissionEvaluationRequest { target: PermissionTarget::Application { application_id: Id::now_v7(), resource: IdentityResource::IdpSecurity }, ..request.clone() },
        ] {
            assert!(!LocalPermissions::new(management.client()).evaluate(&forged).await.expect("exact local actor/action/target evaluation"));
        }
        let remote = incoming.remote_id();
        let admitted = Arc::new(tokio::sync::Notify::new());
        let resume = Arc::new(tokio::sync::Notify::new());
        let checks = Arc::new(AtomicUsize::new(0));
        let authorize: FrameAuthorizer = Arc::new({
            let idp = idp.clone();
            let management = management.clone();
            let request = request.clone();
            let admitted = admitted.clone();
            let resume = resume.clone();
            let checks = checks.clone();
            move || {
                let idp = idp.clone(); let management = management.clone(); let request = request.clone();
                let admitted = admitted.clone(); let resume = resume.clone(); let checks = checks.clone();
                Box::pin(async move {
                    let Ok(Some(device)) = DbDeviceRepo::new(management.clone().client()).find_approved_by_public_key(&remote.to_string()).await else { return false; };
                    let owner_active = DbUserRepo::new(idp.client(), PasswordConfig::default()).find_user_by_id(actor).await.is_ok_and(|owner| owner.is_some());
                    let allowed = owner_active && device.owner_subject == actor.to_string()
                        && LocalPermissions::new(management.client()).evaluate(&request).await.unwrap_or(false);
                    if in_flight && checks.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 2 {
                        assert!(allowed, "first socket write has current local permission");
                        admitted.notify_one();
                        resume.notified().await;
                    }
                    allowed
                })
            }
        });
        let (queued, received) = mpsc::channel(2);
        let (metadata, _) = broadcast::channel(1);
        let (sessions, _) = broadcast::channel(1);
        let inner = Arc::new(Inner { peer: remote, authorize_frame: Some(authorize), outgoing: StdMutex::new(Some(queued)), metadata: StdMutex::new(Some(metadata)), sessions: StdMutex::new(Some(sessions)), sessions_ready: watch::channel(false).0, pending: Mutex::new(BTreeMap::new()),  next_id: AtomicU64::new(1), transport_handles: AtomicUsize::new(1), shutdown: watch::channel(false).0 });
        send_response(&inner, 1, FileResponse::ReadChunk(bytes::Bytes::from_static(b"must-not-disclose"))).await.expect("enqueue while locally granted");
        assert!(LocalPermissions::new(management.client()).evaluate(&request).await.expect("grant read"));
        let writer = if in_flight {
            send_response(&inner, 2, FileResponse::ReadChunk(bytes::Bytes::from_static(b"denied-next-write"))).await.expect("enqueue second response");
            let inner = inner.clone();
            let writer = tokio::spawn(async move { write_loop(inner.clone(), send, received, inner.shutdown.subscribe()).await });
            admitted.notified().await;
            writer
        } else {
            let inner = inner.clone();
            let resume = resume.clone();
            tokio::spawn(async move {
                resume.notified().await;
                write_loop(inner.clone(), send, received, inner.shutdown.subscribe()).await;
            })
        };
        service.remove_role_from_user(INSTALLATION_POLICY_ID, actor, role.id).await.expect("learn local denial while queued");
        assert!(!LocalPermissions::new(management.client()).evaluate(&request).await.expect("current local denial"));
        inner.outgoing.lock().expect("queue lock").take();
        resume.notify_one();
        writer.await.expect("writer completes after learned denial");
        let mut prefix = [0; 4];
        if in_flight {
            peer_recv.read_exact(&mut prefix).await.expect("admitted frame prefix");
            let mut frame = vec![0; u32::from_be_bytes(prefix) as usize];
            peer_recv.read_exact(&mut frame).await.expect("admitted frame finishes");
            assert!(matches!(postcard::from_bytes::<super::WireMessage<iroh::EndpointId>>(&frame).expect("response frame"), super::WireMessage::Response { id: 1, response: FileResponse::ReadChunk(bytes) } if bytes.as_ref() == b"must-not-disclose"));
        }
        assert!(peer_recv.read_exact(&mut prefix).await.is_err(), "queued content must not reach socket after learned local denial");
        assert_eq!(incoming.remote_id(), remote, "no new handshake is needed for denial");
        a.close().await; b.close().await;
        drop(service); drop(idp); drop(management); drop(inner);
        let reopened = Arc::new(Database::open(root.join("management.redb")).expect("offline policy reopen"));
        assert!(!LocalPermissions::new(reopened.client()).evaluate(&request).await.expect("persisted learned denial survives restart"));
        drop(reopened);
        std::fs::remove_dir_all(root).expect("cleanup isolated stores");
    })).await.expect("test completes without delivery sleeps");
}
