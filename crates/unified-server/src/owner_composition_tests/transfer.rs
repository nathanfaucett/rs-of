use std::{
    fs,
    net::SocketAddr,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    time::{Duration, Instant},
};

use db::Database;
use idp_model::model::Id;
use management_service::replica::{DbSelectionPolicyRepo, SelectionPolicy, up};
use tower::ServiceExt;

#[tokio::test]
async fn stale_cleanup_cannot_clear_current_binding_and_exact_retry_stays_restricted() {
    let root = std::env::temp_dir().join(format!("independent-transfer-{}", Id::now_v7()));
    let path = root.join("management.redb");
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
        let policies = DbSelectionPolicyRepo::new(engine.clone());
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
        let policies = DbSelectionPolicyRepo::new(engine);
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
        let policies = DbSelectionPolicyRepo::new(engine.clone());
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
    let path = root.join("management.redb");
    let device = Id::now_v7();
    let old = Id::now_v7().to_string();
    let next = Id::now_v7().to_string();
    let endpoint = iroh::SecretKey::generate().public().to_string();
    let first_id;
    {
        let engine = Arc::new(Database::open(&path).expect("Management owner"));
        up(&engine).await.expect("schema");
        engine.client().translate_and_execute(&format!("INSERT INTO devices (id, owner_subject, name, public_key, address, state, approved_at, created_at, updated_at) VALUES (CAST('{device}' AS UUID), '{next}', 'cycle', '{endpoint}', '', 1, 1, 1, 1)"), &db::SqlTranslator).await.expect("current endpoint");
        let policies = DbSelectionPolicyRepo::new(engine.clone());
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
        let policies = DbSelectionPolicyRepo::new(engine.clone());
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
        let policies = DbSelectionPolicyRepo::new(engine);
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

struct TlsConfig {
    root: PathBuf,
    address: SocketAddr,
}

impl api::LocalTlsConfig for TlsConfig {
    fn tls_directory(&self) -> PathBuf {
        self.root.clone()
    }

    fn tls_address(&self) -> SocketAddr {
        self.address
    }
}

#[test]
fn independent_transfer_production_https() {
    let root = std::env::temp_dir().join(format!("independent-transfer-https-{}", Id::now_v7()));
    fs::create_dir(&root).expect("isolated native trust fixture");
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("private fixture");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("reserve TLS address");
    let address = listener.local_addr().expect("TLS address");
    let mut bundle = Vec::new();
    for owner in ["idp", "management"] {
        let tls = api::LocalTls::create(&TlsConfig {
            root: root.join(owner),
            address,
        })
        .expect("independent owner TLS material");
        bundle.extend(fs::read(tls.ca_path()).expect("isolated owner CA"));
    }
    drop(listener);
    let ca_bundle = root.join("native-roots.pem");
    fs::write(&ca_bundle, bundle).expect("isolated native CA bundle");
    let empty_roots = root.join("empty-roots");
    fs::create_dir(&empty_roots).expect("isolated native CA directory");
    let mut child = Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "owner_composition_tests::transfer::independent_transfer_https_child",
            "--ignored",
            "--nocapture",
        ])
        .env("INDEPENDENT_TRANSFER_HTTPS_ROOT", &root)
        .env("SSL_CERT_FILE", &ca_bundle)
        .env("SSL_CERT_DIR", &empty_roots)
        .env("NO_PROXY", "127.0.0.1,::1,localhost")
        .stdin(Stdio::null())
        .spawn()
        .expect("isolated production HTTPS child");
    let deadline = Instant::now() + Duration::from_secs(90);
    let status = loop {
        if let Some(status) = child.try_wait().expect("HTTPS child status") {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().expect("bound HTTPS child");
            child.wait().expect("reap HTTPS child");
            panic!("production independent transfer child exceeded 90 seconds");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(
        status.success(),
        "production independent transfer HTTPS child"
    );
    fs::remove_dir_all(root).expect("isolated native trust cleanup");
}

#[derive(Clone)]
struct Scope {
    owner: String,
    application: Id,
}

impl storage_service::StorageNamespace for Scope {
    fn user_sub(&self) -> &str {
        &self.owner
    }
    fn application_id(&self) -> Id {
        self.application
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "parent supplies isolated native roots; never installs OS trust"]
async fn independent_transfer_https_child() {
    tokio::time::timeout(Duration::from_secs(90), async {
        use axum::{
            extract::Request,
            http::StatusCode,
            middleware::{self, Next},
            response::IntoResponse,
        };
        use idp_service::{PasswordConfig, replica::DbUserRepo, repo::UserRepo};
        use management_service::{DeviceRepo, HostedControlPlane, replica::DbDeviceRepo};
        use model::contract::{OwnerCopyCleanup, SelectedResourcesResponse};
        use storage_server::{LocalReplicationAuthorizer, ManagementClient, StorageCopyRetirement};
        use storage_service::{DatabaseRuntime, ScopedFileSystemRuntime};
        use tokio_util::sync::CancellationToken;

        let root = PathBuf::from(
            std::env::var_os("INDEPENDENT_TRANSFER_HTTPS_ROOT").expect("isolated child root"),
        );
        let owners = root.join("owners");
        let issuer = format!("https://transfer-{}.test", Id::now_v7());
        let secret = iroh::SecretKey::generate();
        let server = iroh_chain::Server::bind_with_secret_key(
            iroh::endpoint::presets::Minimal,
            secret.clone(),
            iroh_chain::EndpointIdStore::default(),
        )
        .await
        .expect("isolated signer transport");
        let local = iroh::SecretKey::generate().public();
        let fixture = crate::permission_http_tests::seed_idp(
            &owners,
            &issuer,
            &server.endpoint().id().to_string(),
            &local.to_string(),
        )
        .await;
        let idp_engine = Arc::new(Database::open(owners.join("idp/idp.redb")).expect("IdP owner"));
        let users = DbUserRepo::new(idp_engine.clone(), PasswordConfig::default());
        let next = users
            .find_user_by_username_or_email("app-administrator")
            .await
            .expect("next owner")
            .expect("active User")
            .id
            .to_string();
        let third = users
            .find_user_by_username_or_email("unassigned")
            .await
            .expect("third owner")
            .expect("active User")
            .id
            .to_string();
        let old = fixture.administrator.to_string();
        let management_path = owners.join("management/management.redb");
        let management = Arc::new(Database::open(&management_path).expect("Management owner"));
        let devices = DbDeviceRepo::new(management.clone());
        let device = devices
            .find_approved_by_public_key(&local.to_string())
            .await
            .expect("Management lookup")
            .expect("approved endpoint without its own signer");
        let policies = Arc::new(DbSelectionPolicyRepo::new(management.clone()));
        policies
            .set_prevalidated(SelectionPolicy {
                device_id: device.id,
                owner_subject: old.clone(),
                application_id: None,
                selected_kind: None,
                selected_id: None,
                admin_allowed: true,
            })
            .await
            .expect("explicit old-owner policy");
        let idp_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("IdP listener");
        let idp_address = idp_listener.local_addr().expect("IdP address");
        let management_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Management listener");
        let management_address = management_listener
            .local_addr()
            .expect("Management address");
        let idp_base = format!("https://{idp_address}/idp");
        let management_base = format!("https://{management_address}/management");
        let mut config = idp_server::AppConfig {
            data_dir: owners.join("idp").to_string_lossy().into_owned(),
            service_audience: Some("idp-services".into()),
            ..Default::default()
        };
        config.oauth2.issuer = issuer.clone();
        config.oauth2.local_signer = Some(fixture.local_signer);
        config.server.prefix = Some("/idp".into());
        let idp_runtime = idp_server::build_runtime(
            &config,
            idp_engine.clone(),
            None,
            Arc::new(
                management_service::device_identity_from_server(&server, secret)
                    .expect("signer transport identity"),
            ),
            server.clone(),
            None,
        )
        .await
        .expect("production independent IdP runtime");
        let control = Arc::new(
            HostedControlPlane::new_with_services(&idp_base, &idp_base, &issuer)
                .expect("Management service binding")
                .with_idp_service_client("management-idp", "management-idp-secret", "idp-services")
                .expect("existing introspection/active-User relationship"),
        );
        let management_router = management_server::build_router(
            management.clone(),
            &management_base,
            "/management",
            "storage-service",
            control,
            None,
        )
        .await
        .expect("production Management router");
        // Faults are injected at the deployed HTTP boundary, not inside owner collaborators.
        let fault = Arc::new(AtomicU8::new(0));
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let middleware_fault = fault.clone();
        let middleware_entered = entered.clone();
        let middleware_release = release.clone();
        let management_router =
            management_router.layer(middleware::from_fn(move |request: Request, next: Next| {
                let fault = middleware_fault.clone();
                let entered = middleware_entered.clone();
                let release = middleware_release.clone();
                async move {
                    if request.uri().path().ends_with("/replication/owner-cleanup") {
                        match fault.swap(0, Ordering::SeqCst) {
                            1 => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
                            2 => {
                                entered.notify_one();
                                release.notified().await;
                            }
                            _ => {}
                        }
                    }
                    next.run(request).await
                }
            }));
        let idp_tls = api::LocalTls::open(&TlsConfig {
            root: root.join("idp"),
            address: idp_address,
        })
        .expect("retained IdP TLS");
        let management_tls = api::LocalTls::open(&TlsConfig {
            root: root.join("management"),
            address: management_address,
        })
        .expect("retained Management TLS");
        let idp_router = idp_runtime.router();
        let idp_task = tokio::spawn(async move {
            axum::serve(idp_tls.listener(idp_listener), idp_router)
                .await
                .expect("IdP HTTPS");
        });
        let management_task = tokio::spawn(async move {
            axum::serve(
                management_tls.listener(management_listener),
                management_router,
            )
            .await
            .expect("Management HTTPS");
        });
        let client = ManagementClient::new(
            &management_base,
            &idp_base,
            "storage-management",
            "storage-management-secret",
            &issuer,
            "storage-service",
        )
        .expect("existing scoped Storage relationship");
        let http = api::LocalTls::verification_client().expect("native-root HTTPS client");
        let cleanup_url = format!("{management_base}/replication/owner-cleanup");
        let selection_url = format!("{management_base}/replication/devices/{local}/selections");
        assert_eq!(
            http.get(&selection_url)
                .send()
                .await
                .expect("unauthenticated read")
                .status(),
            StatusCode::UNAUTHORIZED
        );
        let application = Id::now_v7();
        let old_scope = Scope {
            owner: old.clone(),
            application,
        };
        let other_scope = Scope {
            owner: third.clone(),
            application,
        };
        let storage_root = owners.join("storage");
        let databases = Arc::new(
            DatabaseRuntime::new(storage_root.join("databases")).expect("Storage databases"),
        );
        let files = Arc::new(
            ScopedFileSystemRuntime::new(storage_root.join("filesystems"), local)
                .expect("Storage filesystems"),
        );
        let (old_database, old_handle) = databases
            .create(&old_scope, None)
            .expect("old owner created SQL/KV copy");
        let old_kv = databases
            .open_kv_selected(&old_scope, old_database.id)
            .expect("old KV");
        let old_files = files
            .create_resource(&old_scope, None)
            .await
            .expect("old filesystem");
        let old_file_handle = files
            .open_resource(&old_scope, old_files.id)
            .await
            .expect("old filesystem handle");
        let (other_database, other_handle) = databases
            .create(&other_scope, None)
            .expect("unaffected database");
        let other_files = files
            .create_resource(&other_scope, None)
            .await
            .expect("unaffected filesystem");
        let copies = StorageCopyRetirement::new(databases.clone(), files.clone());
        let cancellation = CancellationToken::new();
        let authorizer = LocalReplicationAuthorizer::with_management_cleanup(
            client.clone(),
            local,
            copies.clone(),
            cancellation.clone(),
        );
        assert_eq!(
            authorizer
                .local_owner(local)
                .await
                .expect("current authenticated owner"),
            old
        );
        assert!(
            devices
                .transfer_owned(device.id, &old, &next)
                .await
                .expect("Management transfer")
        );
        let pending = client
            .selected_resources(&local.to_string())
            .await
            .expect("authenticated pending evidence");
        let exact = pending.pending_cleanup.expect("durable pending cleanup");
        assert_eq!(
            exact,
            OwnerCopyCleanup {
                cleanup_id: exact.cleanup_id.clone(),
                device_id: device.id.to_string(),
                endpoint_id: local.to_string(),
                old_owner_subject: old.clone(),
                new_owner_subject: next.clone()
            }
        );
        assert!(!pending.admin_allowed);
        assert!(pending.resources.is_empty());
        assert_eq!(
            http.post(&cleanup_url)
                .json(&exact)
                .send()
                .await
                .expect("unauthenticated ack")
                .status(),
            StatusCode::UNAUTHORIZED
        );
        let token_response: model::contract::TokenResponse = http
            .post(format!("{idp_base}/oauth2/token"))
            .form(&[
                ("grant_type", "client_credentials"),
                ("client_id", "storage-management"),
                ("client_secret", "storage-management-secret"),
                ("audience", "storage-service"),
                ("scope", "management.replication.read"),
            ])
            .send()
            .await
            .expect("read-only scoped service request")
            .error_for_status()
            .expect("read-only service token")
            .json()
            .await
            .expect("typed service token");
        assert_eq!(
            http.get(&selection_url)
                .bearer_auth(&token_response.access_token.0)
                .send()
                .await
                .expect("authorized read-only policy")
                .status(),
            StatusCode::OK
        );
        assert_eq!(
            http.post(&cleanup_url)
                .bearer_auth(&token_response.access_token.0)
                .json(&exact)
                .send()
                .await
                .expect("read scope cannot acknowledge cleanup")
                .status(),
            StatusCode::FORBIDDEN
        );
        let human_token = crate::permission_http_tests::user_token(
            &http,
            &format!("https://{idp_address}"),
            "administrator",
            "idp-services",
        )
        .await;
        assert_eq!(
            http.post(&cleanup_url)
                .bearer_auth(&human_token)
                .json(&exact)
                .send()
                .await
                .expect("User cannot act as Storage service")
                .status(),
            StatusCode::FORBIDDEN
        );
        let read_only = LocalReplicationAuthorizer::from_management_client(client.clone());
        assert!(
            read_only.local_owner(local).await.is_err(),
            "policy read cannot acknowledge another Storage's cleanup"
        );
        let filesystem_evidence = storage_root.join("filesystems/filesystem-local-copies.json");
        let saved_evidence = filesystem_evidence.with_extension("saved");
        let had_evidence = filesystem_evidence.exists();
        if had_evidence {
            fs::rename(&filesystem_evidence, &saved_evidence)
                .expect("retain exact filesystem evidence");
        }
        fs::create_dir(&filesystem_evidence).expect("inject local retirement persistence failure");
        assert!(
            authorizer.local_owner(local).await.is_err(),
            "partial retirement cannot acknowledge"
        );
        assert!(
            policies
                .local_cleanup_pending(device.id)
                .await
                .expect("local persistence failure retains Management fence")
        );
        fs::remove_dir(&filesystem_evidence).expect("remove injected failure");
        if had_evidence {
            fs::rename(&saved_evidence, &filesystem_evidence)
                .expect("restore exact retained evidence");
        }
        fault.store(1, Ordering::SeqCst);
        assert!(
            authorizer.local_owner(local).await.is_err(),
            "failed acknowledgement keeps fence"
        );
        assert!(
            policies
                .local_cleanup_pending(device.id)
                .await
                .expect("pending survives HTTPS failure")
        );
        assert!(databases.open(&old_scope, old_database.id).is_err());
        assert!(
            old_handle
                .client()
                .translate_and_execute(
                    "CREATE TABLE denied (id UUID PRIMARY KEY)",
                    &db::SqlTranslator
                )
                .await
                .is_err(),
            "live SQL handle retired before acknowledgement"
        );
        assert!(files.open_resource(&old_scope, old_files.id).await.is_err());
        assert!(
            databases
                .open(&other_scope, other_database.id)
                .expect("unaffected owner")
                .is_some()
        );
        cancellation.cancel();
        assert!(
            authorizer.local_owner(local).await.is_err(),
            "stopping runtime cannot acknowledge"
        );
        assert!(
            policies
                .local_cleanup_pending(device.id)
                .await
                .expect("cancel retains fence")
        );
        drop(authorizer);
        drop(copies);
        drop(old_handle);
        drop(old_kv);
        drop(old_file_handle);
        drop(other_handle);
        drop(databases);
        drop(files);
        let databases = Arc::new(
            DatabaseRuntime::new(storage_root.join("databases")).expect("stopped Storage reopen"),
        );
        let files = Arc::new(
            ScopedFileSystemRuntime::new(storage_root.join("filesystems"), local)
                .expect("stopped filesystem reopen"),
        );
        assert!(
            databases.open(&old_scope, old_database.id).is_err(),
            "durable retirement survives interruption"
        );
        assert!(files.open_resource(&old_scope, old_files.id).await.is_err());
        assert!(
            databases
                .open(&other_scope, other_database.id)
                .expect("unaffected database reopen")
                .is_some()
        );
        assert!(
            files
                .open_resource(&other_scope, other_files.id)
                .await
                .is_ok()
        );
        let authorizer = LocalReplicationAuthorizer::with_management_cleanup(
            client.clone(),
            local,
            StorageCopyRetirement::new(databases.clone(), files.clone()),
            CancellationToken::new(),
        );
        fault.store(2, Ordering::SeqCst);
        let delayed = authorizer.clone();
        let cleanup_task = tokio::spawn(async move { delayed.local_owner(local).await });
        entered.notified().await;
        assert!(
            devices
                .transfer_owned(device.id, &next, &old)
                .await
                .expect("exact original owner returns during delayed HTTPS cleanup")
        );
        release.notify_one();
        assert!(cleanup_task.await.expect("cleanup task").is_err());
        assert!(
            policies
                .local_cleanup_pending(device.id)
                .await
                .expect("stale ack cannot clear fence")
        );
        assert_eq!(
            policies
                .pending_cleanup_owner(device.id)
                .await
                .expect("same durable retirement obligation"),
            Some(old.clone())
        );
        assert!(
            client.acknowledge_owner_cleanup(&exact).await.is_err(),
            "stale exact retry denied"
        );
        drop(authorizer);
        drop(databases);
        drop(files);
        let storage_runtime = storage_server::build_http_runtime(
            storage_server::RouterState::new("https://storage.example")
                .with_management_client(client.clone())
                .with_idp_client(storage_server::IdpClient::local(|_| async {
                    Err(storage_server::IdpIntrospectionError::InvalidToken)
                })),
            &storage_root,
            "/storage",
            local,
            CancellationToken::new(),
        )
        .expect(
            "independent HTTP Storage reuses Management endpoint and local retirement evidence",
        );
        let response = storage_runtime
            .router()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/storage/databases")
                    .header("authorization", "Bearer rejected-user-token")
                    .body(axum::body::Body::empty())
                    .expect("current runtime policy request"),
            )
            .await
            .expect("runtime cleanup policy adapter");
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "cleanup does not authorize the requesting User"
        );
        storage_runtime
            .shutdown()
            .await
            .expect("stop independent Storage runtime");
        assert!(
            !storage_root.join("endpoint.key").exists(),
            "independent Storage never creates a transport identity"
        );
        let databases = Arc::new(
            DatabaseRuntime::new(storage_root.join("databases")).expect("completed Storage reopen"),
        );
        let files = Arc::new(
            ScopedFileSystemRuntime::new(storage_root.join("filesystems"), local)
                .expect("completed filesystem reopen"),
        );
        let authorizer = LocalReplicationAuthorizer::with_management_cleanup(
            client.clone(),
            local,
            StorageCopyRetirement::new(databases.clone(), files.clone()),
            CancellationToken::new(),
        );
        assert!(
            authorizer.local_owner(local).await.is_err(),
            "successful cleanup does not approve new owner"
        );
        assert!(
            !policies
                .local_cleanup_pending(device.id)
                .await
                .expect("current ack completes")
        );
        let policy: SelectedResourcesResponse = client
            .selected_resources(&local.to_string())
            .await
            .expect("current completed policy");
        assert_eq!(policy.owner_subject, old);
        assert!(policy.pending_cleanup.is_none());
        assert!(!policy.admin_allowed);
        assert!(policy.resources.is_empty());
        assert!(databases.open(&old_scope, old_database.id).is_err());
        assert!(files.open_resource(&old_scope, old_files.id).await.is_err());
        assert!(
            databases
                .open(&other_scope, other_database.id)
                .expect("unaffected after activation")
                .is_some()
        );
        let current_ack = OwnerCopyCleanup {
            new_owner_subject: old.clone(),
            ..exact.clone()
        };
        client
            .acknowledge_owner_cleanup(&current_ack)
            .await
            .expect("lost response exact retry");
        let mut wrong_endpoint = current_ack.clone();
        wrong_endpoint.endpoint_id = iroh::SecretKey::generate().public().to_string();
        assert!(
            client
                .acknowledge_owner_cleanup(&wrong_endpoint)
                .await
                .is_err()
        );
        management_task.abort();
        let _ = management_task.await;
        idp_task.abort();
        let _ = idp_task.await;
        drop(idp_runtime);
        server.endpoint().close().await;
        drop(authorizer);
        drop(client);
        drop(databases);
        drop(files);
        drop(policies);
        drop(devices);
        drop(management);
        drop(users);
        drop(idp_engine);
        let management =
            Arc::new(Database::open(&management_path).expect("stopped offline Management reopen"));
        let policies = DbSelectionPolicyRepo::new(management);
        assert!(
            !policies
                .local_cleanup_pending(device.id)
                .await
                .expect("completed cleanup persisted")
        );
        assert!(
            !policies
                .get(device.id, &old, Id::nil())
                .await
                .expect("offline restricted policy")
                .expect("retained policy")
                .admin_allowed
        );
        let databases = DatabaseRuntime::new(storage_root.join("databases"))
            .expect("stopped returned-owner database reopen");
        let files = ScopedFileSystemRuntime::new(storage_root.join("filesystems"), local)
            .expect("stopped returned-owner filesystem reopen");
        assert!(databases.open(&old_scope, old_database.id).is_err());
        assert!(files.open_resource(&old_scope, old_files.id).await.is_err());
    })
    .await
    .expect("bounded independent HTTPS transfer");
}
