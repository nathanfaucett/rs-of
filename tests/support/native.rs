use std::{
    collections::HashMap,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
};

use axum::Router;
use db::Database;
use idp_model::{
    contract::{
        ApplicationRegistration, ClientProfile, ClientRegistration, ClientType, GrantType,
        TokenEndpointAuthMethod,
    },
    model::Id,
};
use idp_service::{
    oauth2::NodeSignerBinding,
    replica::{DbClientRepo, DbKeyRepo, DbUserRepo},
    repo::{ClientRepo, KeyService, PrivateKeyKeyringRepo, UserRepo},
};
use iroh::{SecretKey, endpoint::presets};
use iroh_chain::{EndpointIdStore, Server};
use management_service::LocalPermissions;
use model::contract::{
    AuthorizationDetail, StorageAuthorizationAction, StorageAuthorizationDetail, TokenResponse,
};
use storage_service::{DatabaseRuntime, FileSystemId, ScopedFileSystemRuntime};
use tokio::{net::TcpListener, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use storage_server::{IdpClient, RouterState, resource_router};

pub(crate) struct StorageNamespace {
    pub(crate) user_sub: String,
    pub(crate) application_id: Id,
}
impl storage_model::StorageNamespace for StorageNamespace {
    fn user_sub(&self) -> &str {
        &self.user_sub
    }
    fn application_id(&self) -> Id {
        self.application_id
    }
}

pub(crate) struct TestDirectory(pub(crate) std::path::PathBuf);
impl TestDirectory {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("native-api-{}", Id::now_v7()));
        std::fs::create_dir_all(&root).expect("isolated owner stores");
        Self(root)
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).expect("remove isolated owner stores");
    }
}

#[derive(Clone, Default)]
pub(crate) struct Gate {
    pub(crate) reached: Arc<tokio::sync::Notify>,
    pub(crate) resume: Arc<tokio::sync::Notify>,
}
impl Gate {
    pub(crate) async fn pause(&self) {
        self.reached.notify_one();
        self.resume.notified().await;
    }
    pub(crate) async fn reached(&self) {
        self.reached.notified().await;
    }
    pub(crate) fn release(&self) {
        self.resume.notify_one();
    }
}

struct SecretStore(Arc<keyring_core::mock::Store>);
impl keyring_core::api::CredentialStoreApi for SecretStore {
    fn vendor(&self) -> String {
        self.0.vendor()
    }
    fn id(&self) -> String {
        self.0.id()
    }
    fn build(
        &self,
        service: &str,
        user: &str,
        _: Option<&HashMap<&str, &str>>,
    ) -> keyring_core::Result<keyring_core::Entry> {
        keyring_core::api::CredentialStoreApi::build(&*self.0, service, user, None)
    }
    fn search(&self, spec: &HashMap<&str, &str>) -> keyring_core::Result<Vec<keyring_core::Entry>> {
        self.0.search(spec)
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub(crate) fn registration(
    name: &str,
    grants: Vec<GrantType>,
    scopes: Vec<String>,
    audiences: Vec<String>,
    method: TokenEndpointAuthMethod,
) -> ClientRegistration {
    ClientRegistration {
        application: ApplicationRegistration {
            name: Some(name.into()),
            uri: format!("https://{name}.test"),
            description: None,
        },
        client_id: Some(name.into()),
        client_secret: Some(format!("{name}-secret")),
        client_id_issued_at: None,
        client_secret_expires_at: None,
        client_name: name.into(),
        client_uri: None,
        logo_uri: None,
        contacts: vec![],
        terms_of_service_uri: None,
        policy_uri: None,
        client_type: ClientType::Confidential,
        profile: ClientProfile::Web,
        redirect_uris: vec![],
        allowed_grant_types: grants,
        response_types: vec![],
        allowed_scopes: scopes,
        allowed_audiences: audiences,
        token_endpoint_auth_method: method,
        software_statement: None,
        software_id: None,
        software_version: None,
    }
}

pub(crate) struct Listener {
    pub(crate) base: String,
    pub(crate) cancellation: CancellationToken,
    pub(crate) task: Option<JoinHandle<()>>,
}
impl Listener {
    pub(crate) async fn start(router: Router) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("isolated local listener");
        let base = format!(
            "http://{}",
            listener.local_addr().expect("listener address")
        );
        let cancellation = CancellationToken::new();
        let stop = cancellation.clone();
        let task = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(stop.cancelled_owned())
                .await
                .expect("serve native local APIs");
        });
        Self {
            base,
            cancellation,
            task: Some(task),
        }
    }
    pub(crate) async fn stop(&mut self) {
        self.cancellation.cancel();
        if let Some(task) = self.task.take() {
            task.await.expect("local listener joins");
        }
    }
}

pub(crate) struct Fixture {
    pub(crate) control: Arc<device_service::HostedControlPlane>,
    pub(crate) issuer: String,
    pub(crate) keys: Arc<KeyService<DbKeyRepo, PrivateKeyKeyringRepo>>,
    pub(crate) idp: Arc<Database>,
    pub(crate) devices: Arc<Database>,
    pub(crate) clients: DbClientRepo,
    pub(crate) device: device_service::DeviceEndpointIdentity,
    pub(crate) owner: Id,
    pub(crate) scope: StorageNamespace,
    pub(crate) resource: FileSystemId,
    pub(crate) files: Arc<ScopedFileSystemRuntime<iroh::EndpointId>>,
    pub(crate) databases: Arc<DatabaseRuntime>,
    pub(crate) client: IdpClient,
    pub(crate) http: reqwest::Client,
    pub(crate) identity: Server,
    pub(crate) idp_listener: Listener,
    pub(crate) storage_listener: Listener,
    pub(crate) _directory: TestDirectory,
}
impl Fixture {
    pub(crate) async fn new() -> Self {
        Self::with_catalog_gate(None).await
    }
    pub(crate) async fn with_catalog_gate(catalog_gate: Option<Gate>) -> Self {
        let directory = TestDirectory::new();
        let secret = SecretKey::generate();
        let identity = Server::bind_with_secret_key(
            presets::Minimal,
            secret.clone(),
            EndpointIdStore::default(),
        )
        .await
        .expect("Node endpoint");
        let mut config = idp_server::AppConfig {
            data_dir: directory.0.join("idp").to_string_lossy().into_owned(),
            ..Default::default()
        };
        config.oauth2.issuer = format!("https://installation-{}.test/idp", Id::now_v7());
        config.service_audience = Some("idp-services".into());
        let binding = NodeSignerBinding {
            member_id: Id::now_v7(),
            key_id: Id::now_v7(),
            endpoint_id: identity.endpoint().id().to_string(),
        };
        config.oauth2.local_signer = Some(binding.clone());
        static STORE: OnceLock<Arc<keyring_core::CredentialStore>> = OnceLock::new();
        let store = STORE.get_or_init(|| {
            Arc::new(SecretStore(
                keyring_core::mock::Store::new().expect("test secret adapter"),
            ))
        });
        let private = PrivateKeyKeyringRepo::new_with_store(&config.oauth2.issuer, store.clone());
        let provisioner = idp_server::OwnerProvisioner::open_with_private_keys(&config, private)
            .await
            .expect("native owner provisioning");
        let owner = Id::now_v7();
        let service_client = Id::now_v7();
        assert!(provisioner.prepare_node_signer(&binding).is_err());
        provisioner
            .prepare_fresh_setup_secret_inventory(owner, &[service_client])
            .await
            .expect("fresh exact User and service Client inventory");
        provisioner
            .prepare_setup_client_slots(&[service_client])
            .await
            .expect("complete inventory before private production");
        assert!(provisioner.prepare_setup_client_slots(&[]).await.is_err());
        provisioner
            .ensure_initial_user(owner, Id::now_v7(), "administrator", "valid-password")
            .await
            .expect("real local User");
        provisioner
            .ensure_initial_user_root(owner)
            .await
            .expect("captured initial User signing key");
        provisioner
            .ensure_initial_node(&binding, owner, "API Node")
            .await
            .expect("independent locally trusted signer");
        let mut service_registration = registration(
            "storage-service",
            vec![GrantType::ClientCredentials],
            vec!["idp.token.validate".into()],
            vec![
                "idp-services".into(),
                "storage:filesystems".into(),
                "storage:databases".into(),
            ],
            TokenEndpointAuthMethod::ClientSecretPost,
        );
        service_registration.client_id = Some(service_client.to_string());
        provisioner
            .ensure_setup_infrastructure_client(service_registration.clone())
            .await
            .expect("captured least-privilege service OAuth");
        let public = provisioner
            .verify_node_signer(&binding)
            .await
            .expect("initial signer");
        drop(provisioner);
        let provisioner = idp_server::OwnerProvisioner::open(&config)
            .await
            .expect("existing native owner reopen");
        provisioner
            .prepare_setup_client_slots(&[service_client])
            .await
            .expect("retained exact inventory");
        assert!(
            provisioner
                .prepare_fresh_setup_secret_inventory(owner, &[service_client])
                .await
                .is_err()
        );
        assert_eq!(
            provisioner
                .verify_node_signer(&binding)
                .await
                .expect("retained signer"),
            public
        );
        provisioner
            .ensure_setup_infrastructure_client(service_registration)
            .await
            .expect("exact captured Client retry");
        drop(provisioner);
        let idp = Arc::new(
            Database::open(directory.0.join("idp/idp.redb")).expect("reopen native IdP owner"),
        );
        let keys = Arc::new(KeyService::new(
            DbKeyRepo::new(idp.client()),
            PrivateKeyKeyringRepo::new(&config.oauth2.issuer).expect("retained local private keys"),
            config.key_namespace.clone(),
        ));

        let clients = DbClientRepo::new(idp.client(), Arc::clone(&keys));
        let human = clients
            .create_client(registration(
                "human",
                vec![GrantType::Password, GrantType::TokenExchange],
                vec!["openid".into()],
                vec!["storage:filesystems".into(), "storage:databases".into()],
                TokenEndpointAuthMethod::ClientSecretBasic,
            ))
            .await
            .expect("native human Application client");
        let devices = Arc::new(
            Database::open(directory.0.join("devices.redb")).expect("separate Device store"),
        );
        device_service::replica::up(&devices)
            .await
            .expect("native policy schema");
        let device_repo = Arc::new(device_service::replica::DbDeviceRepo::new(
            devices.clone().client(),
        ));
        device_repo
            .enroll_approved_exact(
                binding.member_id,
                owner,
                "API Node",
                &binding.endpoint_id,
                1,
            )
            .await
            .expect("Device-owned initial endpoint");
        let runtime = idp_server::build_runtime(
            &config,
            idp.client(),
            Some(device_repo.clone()),
            Arc::new(
                device_service::device_identity_from_server(&identity, secret)
                    .expect("injected endpoint identity"),
            ),
            identity.clone(),
            Some(LocalPermissions::new(idp.client())),
        )
        .await
        .expect("integrated owner-local IdP runtime");
        let runtime = Arc::new(runtime);
        let inspect = Arc::clone(&runtime);
        let active = Arc::clone(&runtime);
        let control = Arc::new(
            device_service::HostedControlPlane::new_with_services(
                "http://127.0.0.1:1",
                "http://127.0.0.1:1",
                &config.oauth2.issuer,
            )
            .expect("owner services")
            .with_local_devices(device_repo)
            .with_local_idp(
                move |token| {
                    let idp = Arc::clone(&inspect);
                    async move { idp.authorize_management_bearer(&token).await }
                },
                move |user| {
                    let idp = Arc::clone(&active);
                    async move { idp.active_user(user).await }
                },
            ),
        );
        let device = control
            .get_storage_endpoint_identity(&identity.endpoint().id().to_string())
            .await
            .expect("current Device owner");
        let idp_listener = Listener::start(runtime.router()).await;
        let client = if let Some(gate) = catalog_gate {
            let inspect = Arc::clone(&runtime);
            let calls = Arc::new(AtomicUsize::new(0));
            IdpClient::local(move |token| {
                let runtime = Arc::clone(&inspect);
                let gate = gate.clone();
                let calls = Arc::clone(&calls);
                async move {
                    if calls.fetch_add(1, Ordering::SeqCst) == 1 {
                        gate.pause().await;
                    }
                    runtime
                        .authorize_storage_bearer(&token)
                        .await
                        .map_err(storage_introspection_error)
                }
            })
        } else {
            IdpClient::local(move |token| {
                let idp = Arc::clone(&runtime);
                async move {
                    idp.authorize_storage_bearer(&token)
                        .await
                        .map_err(storage_introspection_error)
                }
            })
        };
        let files = Arc::new(
            ScopedFileSystemRuntime::new(directory.0.join("files"), identity.endpoint().id())
                .expect("native filesystems"),
        );
        let databases = Arc::new(
            DatabaseRuntime::new(directory.0.join("databases")).expect("native databases"),
        );
        let scope = StorageNamespace {
            user_sub: owner.to_string(),
            application_id: human.application_id,
        };
        let resource = files
            .create_resource(&scope, None)
            .await
            .expect("real resource")
            .id;
        let router = resource_router(
            RouterState::new("https://storage.test").with_idp_client(client.clone()),
            Some(databases.clone()),
            Some(files.clone()),
        );
        let storage_listener = Listener::start(router).await;
        Self {
            control,
            issuer: config.oauth2.issuer,
            keys,
            idp,
            devices,
            clients,
            device,
            owner,
            scope,
            resource,
            files,
            databases,
            client,
            http: reqwest::Client::new(),
            identity,
            idp_listener,
            storage_listener,
            _directory: directory,
        }
    }
    pub(crate) async fn token(
        &self,
        resource: &str,
        actions: Vec<StorageAuthorizationAction>,
    ) -> String {
        self.token_for_client(resource, actions, "human").await
    }

    pub(crate) async fn token_for_client(
        &self,
        resource: &str,
        actions: Vec<StorageAuthorizationAction>,
        client_id: &str,
    ) -> String {
        let url = format!("{}/oauth2/token", self.idp_listener.base);
        let response = self
            .http
            .post(&url)
            .basic_auth(client_id, Some(format!("{client_id}-secret")))
            .form(&[
                ("grant_type", "password"),
                ("client_id", client_id),
                ("username", "administrator"),
                ("password", "valid-password"),
                ("scope", "openid"),
            ])
            .send()
            .await
            .expect("local password token request");
        let status = response.status();
        let body = response.text().await.expect("token body");
        assert!(status.is_success(), "native User token: {status} {body}");
        let subject: TokenResponse = serde_json::from_str(&body).expect("User token response");
        let details =
            serde_json::to_string(&[AuthorizationDetail::Storage(StorageAuthorizationDetail {
                actions,
            })])
            .expect("exact actions");
        let response = self
            .http
            .post(url)
            .basic_auth(client_id, Some(format!("{client_id}-secret")))
            .form(&[
                (
                    "grant_type",
                    "urn:ietf:params:oauth:grant-type:token-exchange",
                ),
                ("subject_token", &subject.access_token.0),
                (
                    "subject_token_type",
                    "urn:ietf:params:oauth:token-type:access_token",
                ),
                ("resource", resource),
                ("authorization_details", &details),
            ])
            .send()
            .await
            .expect("native local exchange");
        let status = response.status();
        let body = response.text().await.expect("exchange body");
        assert!(status.is_success(), "native storage token: {status} {body}");
        serde_json::from_str::<TokenResponse>(&body)
            .expect("Storage token response")
            .access_token
            .0
    }
    pub(crate) async fn file(&self) -> Arc<file_system::FileSystem<iroh::EndpointId>> {
        self.files
            .open_resource(&self.scope, self.resource)
            .await
            .expect("public local resource observation")
    }
    pub(crate) async fn revoke_user(&self) {
        DbUserRepo::new(self.idp.client(), Default::default())
            .delete_user_by_id(self.owner)
            .await
            .expect("learn native User revocation");
    }
    pub(crate) async fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        token: &str,
    ) -> reqwest::Response {
        self.http
            .request(method, format!("{}{path}", self.storage_listener.base))
            .bearer_auth(token)
            .json(&serde_json::json!({"name": "non-unique name"}))
            .send()
            .await
            .expect("native Storage API request")
    }
    pub(crate) async fn stop(mut self) {
        self.storage_listener.stop().await;
        self.idp_listener.stop().await;
        self.identity.endpoint().close().await;
    }
}

pub(crate) fn storage_introspection_error(
    error: idp_model::contract::ErrorResponse,
) -> storage_server::IdpIntrospectionError {
    use idp_model::contract::ErrorCode;
    use storage_server::IdpIntrospectionError;

    match error.error {
        ErrorCode::ServerError | ErrorCode::TemporarilyUnavailable => {
            IdpIntrospectionError::ServiceUnavailable
        }
        _ => IdpIntrospectionError::InvalidToken,
    }
}
