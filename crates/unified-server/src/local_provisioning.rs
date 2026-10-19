use std::{
    fs::{self, File, OpenOptions},
    future::Future,
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    pin::Pin,
    sync::Arc,
};

use idp_model::{
    contract::{
        ApplicationRegistration, ClientProfile, ClientRegistration, ClientType, GrantType,
        IdentityAction, JwkPublic, TokenEndpointAuthMethod,
    },
    model::Id,
};
use idp_server::OwnerProvisioner;
use idp_service::{encrypt_password, oauth2::NodeSignerBinding};
use serde::{Deserialize, Serialize};

use crate::{UnifiedConfig, UnifiedEndpoint, UnifiedRuntime};

const INITIAL_ACTIONS: [IdentityAction; 25] = [
    IdentityAction::InfrastructureClientsRead,
    IdentityAction::InfrastructureClientsCreate,
    IdentityAction::InfrastructureClientsUpdate,
    IdentityAction::InfrastructureClientsDelete,
    IdentityAction::ApplicationsRead,
    IdentityAction::ApplicationsCreate,
    IdentityAction::ApplicationsUpdate,
    IdentityAction::ApplicationsDelete,
    IdentityAction::UsersRead,
    IdentityAction::UsersUpdate,
    IdentityAction::UsersDelete,
    IdentityAction::UsersResetPassword,
    IdentityAction::ConsentsRead,
    IdentityAction::ConsentsRevoke,
    IdentityAction::KeysRead,
    IdentityAction::KeysRotate,
    IdentityAction::KeysRevoke,
    IdentityAction::DevicesRead,
    IdentityAction::DevicePairingRead,
    IdentityAction::DevicePairingUpdate,
    IdentityAction::ReplicaSignersEnroll,
    IdentityAction::ReplicaSignersRotate,
    IdentityAction::ReplicaSignersRevoke,
    IdentityAction::IdpSecuritySync,
    IdentityAction::ManagementSecuritySync,
];

#[derive(Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Intent {
    issuer: String,
    key_namespace: String,
    roots: [PathBuf; 4],
    password_policy: serde_json::Value,
    applications: [String; 3],
    audiences: [String; 2],
}

#[derive(Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Journal {
    version: u8,
    intent: Intent,
    installation: Id,
    user: Id,
    credential: Id,
    login_client: Id,
    member: Id,
    key: Id,
    role: Id,
    permissions: Vec<Id>,
    administrator: String,
    node: String,
    password_verifier: String,
    endpoint_attempted: bool,
    endpoint: Option<String>,
    inventory_attempted: bool,
    node_attempted: bool,
    public_signer: Option<JwkPublic>,
    management_attempted: bool,
    management_complete: bool,
    storage_attempted: bool,
    storage_initialized: bool,
    complete: bool,
}

/// Restricted operator-local setup. Keep this guard alive until the runtime is shut down.
/// Paths require stable operator-controlled ancestry; this is not a hostile-path race defense.
/// Only Unix owner-only files are supported by this local command.
pub struct LocalProvisioner {
    config: UnifiedConfig,
    directory: PathBuf,
    _operation: File,
}

impl LocalProvisioner {
    /// CLI existing-only startup decision; a present local journal is authoritative.
    pub async fn open_runtime(config: UnifiedConfig) -> io::Result<(UnifiedRuntime, Option<Self>)> {
        Self::open_runtime_for_transport(config, false).await
    }

    pub async fn open_runtime_https(
        config: UnifiedConfig,
    ) -> io::Result<(UnifiedRuntime, Option<Self>)> {
        Self::open_runtime_for_transport(config, true).await
    }

    async fn open_runtime_for_transport(
        config: UnifiedConfig,
        https: bool,
    ) -> io::Result<(UnifiedRuntime, Option<Self>)> {
        match fs::symlink_metadata(config.local_provisioning_directory()) {
            Ok(_) => {
                let local = Self::open(config)?;
                let runtime = local.build_runtime_for_transport(https).await?;
                Ok((runtime, Some(local)))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                crate::runtime::validate_config(&config)?;
                let binding = config.idp.oauth2.local_signer.as_ref().ok_or_else(|| {
                    invalid("existing host requires an explicit retained Node signer binding")
                })?;
                for root in [
                    Path::new(&config.idp.data_dir),
                    Path::new(&config.management.data_dir),
                    Path::new(&config.storage.data_dir),
                    &config.endpoint_data_dir,
                ] {
                    require_retained_directory(root)?;
                }
                require_file(&Path::new(&config.idp.data_dir).join("idp.redb"), false)?;
                require_file(
                    &Path::new(&config.management.data_dir).join("management.redb"),
                    false,
                )?;
                require_file(&config.endpoint_data_dir.join("endpoint.key"), true)?;
                require_storage(&config)?;
                if UnifiedEndpoint::persisted_endpoint_id(&config.endpoint_data_dir)?.to_string()
                    != binding.endpoint_id
                {
                    return Err(invalid(
                        "existing endpoint conflicts with retained Node signer binding",
                    ));
                }

                let runtime = if https {
                    UnifiedRuntime::build_https(config).await?
                } else {
                    UnifiedRuntime::build(config).await?
                };
                match runtime.node_public_ready().await {
                    Ok(true) => Ok((runtime, None)),
                    result => {
                        runtime.shutdown().await?;
                        result?;
                        Err(invalid("existing host is not currently locally ready"))
                    }
                }
            }
            Err(error) => Err(error),
        }
    }

    /// Publishes a new exact intent, without creating endpoint or owner private material.
    /// All four context/endpoint roots and the adjacent provisioning directory must be absent.
    pub fn create(
        config: UnifiedConfig,
        administrator: &str,
        node: &str,
        password_file: &Path,
    ) -> io::Result<Self> {
        require_unix_bootstrap()?;
        let password = read_password(password_file)?;
        Self::create_with_password(config, administrator, node, &password)
    }

    pub fn create_with_password(
        config: UnifiedConfig,
        administrator: &str,
        node: &str,
        password: &str,
    ) -> io::Result<Self> {
        require_unix_bootstrap()?;
        validate_password(password)?;
        let config = normalized_config(config)?;
        if administrator.trim().is_empty()
            || node.trim().is_empty()
            || config.idp.oauth2.local_signer.is_some()
        {
            return Err(invalid(
                "fresh setup requires names and no preconfigured Node signer",
            ));
        }
        let intent = intent(&config)?;
        for root in &intent.roots {
            require_absent(root)?;
        }
        let directory = config.local_provisioning_directory();
        require_absent(&directory)?;
        let journal = Journal {
            version: 1,
            intent,
            installation: Id::now_v7(),
            user: Id::now_v7(),
            credential: Id::now_v7(),
            login_client: Id::now_v7(),
            member: Id::now_v7(),
            key: Id::now_v7(),
            role: Id::now_v7(),
            permissions: INITIAL_ACTIONS.iter().map(|_| Id::now_v7()).collect(),
            administrator: administrator.to_owned(),
            node: node.to_owned(),
            password_verifier: encrypt_password(&config.idp.password, password)
                .map_err(io::Error::other)?,
            endpoint_attempted: false,
            endpoint: None,
            inventory_attempted: false,
            node_attempted: false,
            public_signer: None,
            management_attempted: false,
            management_complete: false,
            storage_attempted: false,
            storage_initialized: false,
            complete: false,
        };
        create_private_directory(
            directory.parent().expect("absolute provisioning parent"),
            true,
        )?;
        create_private_directory(&directory, false)?;
        let operation = lock(&directory, true)?;
        let host = Self {
            config,
            directory,
            _operation: operation,
        };
        host.write(&journal)?;
        Ok(host)
    }

    /// Existing-only opening. Missing/invalid intent is never fresh authorization.
    pub fn open(config: UnifiedConfig) -> io::Result<Self> {
        let config = normalized_config(config)?;
        let directory = config.local_provisioning_directory();
        require_directory(&directory)?;
        let operation = lock(&directory, false)?;
        require_absent(&directory.join("join.json"))?;
        let host = Self {
            config,
            directory,
            _operation: operation,
        };
        host.read()?;
        Ok(host)
    }

    /// Continues only the retained exact local intent, using protected password input.
    pub async fn provision(&mut self, password_file: &Path) -> io::Result<()> {
        let password = read_password(password_file)?;
        self.provision_with_tls(&password, None).await
    }

    pub async fn provision_https(
        &mut self,
        password_file: &Path,
        tls: &crate::LocalTls,
    ) -> io::Result<()> {
        require_unix_bootstrap()?;
        crate::LocalTls::api_base(self.config.listen_addr)?;
        let password = read_password(password_file)?;
        self.provision_https_with_password(&password, tls).await
    }

    pub async fn provision_https_with_password(
        &mut self,
        password: &str,
        tls: &crate::LocalTls,
    ) -> io::Result<()> {
        require_unix_bootstrap()?;
        crate::LocalTls::api_base(self.config.listen_addr)?;
        self.provision_with_tls(password, Some(tls)).await
    }

    /// Public registration only; passwords, tokens and service secrets are never exposed.
    pub fn login_client(&self) -> io::Result<ClientRegistration> {
        let journal = self.read()?;
        Ok(login_registration(&self.config, journal.login_client))
    }

    pub fn is_complete(&self) -> io::Result<bool> {
        Ok(self.read()?.complete)
    }

    pub async fn verify_https(&self, tls: &crate::LocalTls) -> io::Result<()> {
        self.build_runtime_for_transport(true)
            .await?
            .verify_local_bootstrap(Some(tls))
            .await
    }

    async fn provision_with_tls(
        &mut self,
        password: &str,
        tls: Option<&crate::LocalTls>,
    ) -> io::Result<()> {
        let runtime = self
            .prepare_runtime(
                password,
                tls.is_some(),
                false,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await?;
        runtime.verify_local_bootstrap(tls).await?;
        self.mark_complete()
    }

    pub(crate) fn validate_retry(
        &self,
        administrator: &str,
        node: &str,
        password: &str,
    ) -> io::Result<()> {
        let journal = self.read()?;
        if journal.administrator != administrator || journal.node != node {
            return Err(invalid(
                "human Create intent conflicts with retained owner state",
            ));
        }
        verify(password, &journal.password_verifier)
    }

    pub(crate) async fn prepare_hosted_runtime(
        &mut self,
        password: &str,
        cancellation: &tokio_util::sync::CancellationToken,
    ) -> io::Result<UnifiedRuntime> {
        Box::pin(self.prepare_runtime(password, true, true, cancellation)).await
    }

    pub(crate) fn mark_complete(&self) -> io::Result<()> {
        let mut journal = self.read()?;
        journal.complete = true;
        self.write(&journal)
    }

    async fn prepare_runtime(
        &mut self,
        password: &str,
        https: bool,
        hosted: bool,
        cancellation: &tokio_util::sync::CancellationToken,
    ) -> io::Result<UnifiedRuntime> {
        let started = std::time::Instant::now();
        validate_password(password)?;
        check_cancellation(cancellation)?;
        let mut journal = self.read()?;
        verify(password, &journal.password_verifier)?;
        if journal.complete {
            return Box::pin(self.build_runtime_for_host(https, hosted)).await;
        }
        if journal.management_attempted {
            require_directory(Path::new(&self.config.management.data_dir))?;
            require_file(
                &Path::new(&self.config.management.data_dir).join("management.redb"),
                false,
            )?;
            if !journal.management_complete {
                return Err(invalid(
                    "unacknowledged Management initialization requires explicit recovery",
                ));
            }
        }
        if journal.storage_attempted {
            if !journal.storage_initialized {
                return Err(invalid(
                    "unacknowledged Storage initialization requires explicit recovery",
                ));
            }
            require_directory(Path::new(&self.config.storage.data_dir))?;
            require_storage(&self.config)?;
        }
        let endpoint = if journal.endpoint_attempted {
            UnifiedEndpoint::open(&self.config.endpoint_data_dir, &self.config.idp).await?
        } else {
            for root in &journal.intent.roots {
                require_absent(root)?;
            }
            journal.endpoint_attempted = true;
            self.write(&journal)?;
            UnifiedEndpoint::open_fresh(&self.config.endpoint_data_dir, &self.config.idp).await?
        };
        let endpoint = Arc::new(endpoint);
        let result = async {
            let endpoint_id = endpoint.endpoint_id().to_string();
            if journal
                .endpoint
                .as_ref()
                .is_some_and(|retained| retained != &endpoint_id)
            {
                return Err(invalid(
                    "endpoint identity changed; exact recovery required",
                ));
            }
            journal.endpoint = Some(endpoint_id);
            self.write(&journal)?;
            log::info!(
                "Node Create stage=endpoint elapsed_ms={}",
                started.elapsed().as_millis()
            );
            let started = std::time::Instant::now();
            check_cancellation(cancellation)?;
            let fresh_inventory = !journal.inventory_attempted;
            if fresh_inventory {
                require_absent(Path::new(&self.config.idp.data_dir))?;
                journal.inventory_attempted = true;
                self.write(&journal)?;
                create_private_directory(Path::new(&self.config.idp.data_dir), false)?;
            } else {
                require_file(
                    &Path::new(&self.config.idp.data_dir).join("idp.redb"),
                    false,
                )?;
            }
            let owner = OwnerProvisioner::open(&self.config.idp).await?;
            let clients = [journal.login_client];
            if fresh_inventory {
                provisioning_stage(|| {
                    owner.prepare_fresh_setup_secret_inventory(journal.user, &clients)
                })
                .await?;
            }
            provisioning_stage(|| owner.prepare_setup_client_slots(&clients)).await?;
            provisioning_stage(|| {
                owner.ensure_initial_user(
                    journal.user,
                    journal.credential,
                    &journal.administrator,
                    password,
                )
            })
            .await?;
            provisioning_stage(|| owner.ensure_initial_user_root(journal.user)).await?;
            let binding = binding(&journal)?;
            let public = if journal.node_attempted {
                owner.verify_prepared_node_signer(&binding)?
            } else {
                journal.node_attempted = true;
                self.write(&journal)?;
                owner.prepare_node_signer(&binding)?
            };
            if journal
                .public_signer
                .as_ref()
                .is_some_and(|retained| retained != &public)
            {
                return Err(invalid(
                    "prepared Node signer changed; exact recovery required",
                ));
            }
            journal.public_signer = Some(public);
            self.write(&journal)?;
            provisioning_stage(|| owner.ensure_initial_node(&binding, journal.user, &journal.node))
                .await?;
            if Some(provisioning_stage(|| owner.verify_node_signer(&binding)).await?)
                != journal.public_signer
            {
                return Err(invalid(
                    "approved Node signer differs from retained preparation",
                ));
            }
            provisioning_stage(|| {
                owner.ensure_setup_public_login_client(login_registration(
                    &self.config,
                    journal.login_client,
                ))
            })
            .await?;
            let idp_engine = owner.node_join_database();
            log::info!(
                "Node Create stage=idp elapsed_ms={}",
                started.elapsed().as_millis()
            );
            let started = std::time::Instant::now();
            check_cancellation(cancellation)?;
            if !journal.management_attempted {
                require_absent(Path::new(&self.config.management.data_dir))?;
                journal.management_attempted = true;
                self.write(&journal)?;
                create_private_directory(Path::new(&self.config.management.data_dir), false)?;
            }
            let management_engine =
                management_server::open_database(&self.config.management).await?;
            if !journal.management_complete {
                let permissions = journal
                    .permissions
                    .iter()
                    .copied()
                    .zip(INITIAL_ACTIONS)
                    .collect::<Vec<_>>();
                provisioning_stage(|| {
                    management_server::provision_initial_administrator(
                        Arc::clone(&management_engine),
                        journal.user,
                        journal.role,
                        &permissions,
                    )
                })
                .await?;
                let endpoint_id = binding.endpoint_id.clone();
                let node_name = journal.node.clone();
                provisioning_stage(|| {
                    management_server::provision_initial_device(
                        Arc::clone(&management_engine),
                        journal.member,
                        journal.user,
                        &endpoint_id,
                        &node_name,
                    )
                })
                .await?;
                journal.management_complete = true;
                self.write(&journal)?;
            }
            log::info!(
                "Node Create stage=management elapsed_ms={}",
                started.elapsed().as_millis()
            );
            let started = std::time::Instant::now();
            check_cancellation(cancellation)?;
            let listener = if hosted {
                None
            } else {
                Some(tokio::net::TcpListener::bind(self.config.listen_addr).await?)
            };
            let api_base = match &listener {
                Some(listener) if !https => format!("http://{}", listener.local_addr()?),
                Some(listener) => crate::LocalTls::api_base(listener.local_addr()?)?,
                None => crate::LocalTls::api_base(self.config.listen_addr)?,
            };
            if !journal.storage_attempted {
                require_absent(Path::new(&self.config.storage.data_dir))?;
                journal.storage_attempted = true;
                self.write(&journal)?;
                create_private_directory(Path::new(&self.config.storage.data_dir), false)?;
            }
            let mut config = self.config.clone();
            config.idp.oauth2.local_signer = Some(binding);
            let runtime = Box::pin(UnifiedRuntime::build_with_owners(
                config,
                listener,
                api_base,
                crate::runtime::RuntimeOwners {
                    endpoint: Arc::clone(&endpoint),
                    idp: idp_engine,
                    management: management_engine,
                },
            ))
            .await?;
            log::info!(
                "Node Create stage=runtime elapsed_ms={}",
                started.elapsed().as_millis()
            );
            journal.storage_initialized = true;
            if let Err(error) = self.write(&journal) {
                runtime.shutdown().await?;
                return Err(error);
            }
            if let Err(error) = check_cancellation(cancellation) {
                runtime.shutdown().await?;
                return Err(error);
            }
            Ok(runtime)
        }
        .await;
        if result.is_err() {
            endpoint.close().await;
        }
        result
    }

    /// Requires explicit completion and current local evidence; never provisions at startup.
    pub async fn build_runtime(&self) -> io::Result<UnifiedRuntime> {
        self.build_runtime_for_transport(false).await
    }

    async fn build_runtime_for_transport(&self, https: bool) -> io::Result<UnifiedRuntime> {
        self.build_runtime_for_host(https, false).await
    }

    pub(crate) async fn reopen_hosted_runtime(&self) -> io::Result<UnifiedRuntime> {
        self.build_runtime_for_host(true, true).await
    }

    async fn build_runtime_for_host(
        &self,
        https: bool,
        hosted: bool,
    ) -> io::Result<UnifiedRuntime> {
        let journal = self.read()?;
        if !journal.complete {
            return Err(invalid(
                "local provisioning is incomplete; continue the retained create command",
            ));
        }
        for root in &journal.intent.roots {
            require_directory(root)?;
        }
        require_file(
            &Path::new(&self.config.idp.data_dir).join("idp.redb"),
            false,
        )?;
        require_file(
            &Path::new(&self.config.management.data_dir).join("management.redb"),
            false,
        )?;
        require_file(&self.config.endpoint_data_dir.join("endpoint.key"), true)?;
        require_storage(&self.config)?;
        let binding = binding(&journal)?;
        if UnifiedEndpoint::persisted_endpoint_id(&self.config.endpoint_data_dir)?.to_string()
            != binding.endpoint_id
        {
            return Err(invalid(
                "endpoint identity changed; exact recovery required",
            ));
        }
        let mut config = self.config.clone();
        config.idp.oauth2.local_signer = Some(binding.clone());
        let owners = crate::runtime::RuntimeOwners::open(&config).await?;
        let result = async {
            let owner = OwnerProvisioner::from_database(&config.idp, Arc::clone(&owners.idp))?;
            if Some(owner.verify_node_signer(&binding).await?) != journal.public_signer {
                return Err(invalid(
                    "current Node signing capability does not match completed intent",
                ));
            }
            owner
                .verify_public_login_client(login_registration(&self.config, journal.login_client))
                .await?;
            let listener = if hosted {
                None
            } else {
                Some(tokio::net::TcpListener::bind(config.listen_addr).await?)
            };
            let address = match &listener {
                Some(listener) => listener.local_addr()?,
                None => config.listen_addr,
            };
            let api_base = if https {
                crate::LocalTls::api_base(address)?
            } else {
                format!("http://{address}")
            };
            Box::pin(UnifiedRuntime::build_with_owners(
                config,
                listener,
                api_base,
                crate::runtime::RuntimeOwners {
                    endpoint: Arc::clone(&owners.endpoint),
                    idp: Arc::clone(&owners.idp),
                    management: Arc::clone(&owners.management),
                },
            ))
            .await
        }
        .await;
        let mut runtime = match result {
            Ok(runtime) => runtime,
            Err(error) => {
                owners.endpoint.close().await;
                return Err(error);
            }
        };
        drop(owners);
        if journal.endpoint.as_deref() != Some(&runtime.endpoint_id().to_string()) {
            runtime.shutdown().await?;
            return Err(invalid(
                "endpoint identity changed; exact recovery required",
            ));
        }
        let readiness = if hosted {
            runtime.start_background_tasks().await
        } else {
            match runtime.node_public_ready().await {
                Ok(true) => Ok(()),
                Ok(false) => Err(invalid("completed intent is not current local readiness")),
                Err(error) => Err(error),
            }
        };
        match readiness {
            Ok(()) => Ok(runtime),
            Err(error) => {
                runtime.shutdown().await?;
                Err(error)
            }
        }
    }

    pub(crate) fn reset_references(&self) -> io::Result<serde_json::Value> {
        let journal = self.read()?;
        Ok(serde_json::json!({
            "member_id": journal.member,
            "signer_key_id": journal.key,
            "owner_id": journal.user,
            "clients": [journal.login_client],
            "inventory_attempted": journal.inventory_attempted,
        }))
    }

    fn read(&self) -> io::Result<Journal> {
        let bytes = read_private(&self.directory.join("intent.json"))?;
        let journal: Journal = serde_json::from_slice(&bytes)?;
        let retained: serde_json::Value = serde_json::from_slice(&bytes)?;
        if serde_json::to_value(&journal)? != retained
            || journal.version != 1
            || journal.intent != intent(&self.config)?
            || journal.administrator.trim().is_empty()
            || journal.node.trim().is_empty()
            || journal.permissions.len() != INITIAL_ACTIONS.len()
            || journal.endpoint.is_some() && !journal.endpoint_attempted
            || journal.inventory_attempted && journal.endpoint.is_none()
            || journal.node_attempted && !journal.inventory_attempted
            || journal.public_signer.is_some() && !journal.node_attempted
            || journal.management_attempted && journal.public_signer.is_none()
            || journal.management_complete && !journal.management_attempted
            || journal.storage_attempted && !journal.management_complete
            || journal.storage_initialized && !journal.storage_attempted
            || journal.complete && !journal.storage_initialized
        {
            return Err(invalid(
                "invalid or conflicting local intent; exact recovery required",
            ));
        }
        let mut identities = vec![
            journal.installation,
            journal.user,
            journal.credential,
            journal.member,
            journal.key,
            journal.role,
        ];
        identities.extend(&journal.permissions);
        identities.push(journal.login_client);
        let count = identities.len();
        identities.sort();
        identities.dedup();
        if identities.len() != count || identities.iter().any(Id::is_nil) {
            return Err(invalid(
                "local intent identities must be distinct and non-nil",
            ));
        }

        if let Some(configured) = &self.config.idp.oauth2.local_signer
            && !same_binding(configured, &binding(&journal)?)
        {
            return Err(invalid(
                "configured Node signer conflicts with local intent",
            ));
        }
        Ok(journal)
    }

    fn write(&self, journal: &Journal) -> io::Result<()> {
        let path = self.directory.join("intent.json");
        let temporary = self.directory.join(format!(".{}.tmp", Id::now_v7()));
        let mut file = private_options(true).open(&temporary)?;
        file.write_all(&serde_json::to_vec(journal)?)?;
        file.sync_all()?;
        fs::rename(&temporary, &path)?;
        sync_ancestry(&self.directory)?;
        if self.read()? != *journal {
            return Err(invalid("durable local intent readback changed"));
        }
        Ok(())
    }
}

// Construct large owner futures outside the caller's poll frame as well as storing them on the heap.
#[inline(never)]
pub(crate) fn provisioning_stage<'a, T: 'a, F>(
    make: impl FnOnce() -> F,
) -> Pin<Box<dyn Future<Output = io::Result<T>> + Send + 'a>>
where
    F: Future<Output = io::Result<T>> + Send + 'a,
{
    Box::pin(make())
}

fn check_cancellation(cancellation: &tokio_util::sync::CancellationToken) -> io::Result<()> {
    if cancellation.is_cancelled() {
        Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "Create cancelled; retained owner state requires exact continuation",
        ))
    } else {
        Ok(())
    }
}

pub(crate) fn intent(config: &UnifiedConfig) -> io::Result<Intent> {
    crate::runtime::validate_config(config)?;
    crate::LocalTls::validate_ip(config.listen_addr.ip())?;
    if !config.listen_addr.ip().is_loopback() {
        return Err(invalid("local host must use a loopback listener"));
    }
    let roots = [
        PathBuf::from(&config.idp.data_dir),
        PathBuf::from(&config.management.data_dir),
        PathBuf::from(&config.storage.data_dir),
        config.endpoint_data_dir.clone(),
    ];
    let directory = config.local_provisioning_directory();
    let all = roots
        .iter()
        .chain(std::iter::once(&directory))
        .collect::<Vec<_>>();
    for (index, root) in all.iter().enumerate() {
        reject_symlinks(root)?;
        if all[..index]
            .iter()
            .any(|other| root.starts_with(other) || other.starts_with(root))
        {
            return Err(invalid(
                "context, endpoint and provisioning roots must not overlap",
            ));
        }
    }
    if config.idp.oauth2.issuer.trim().is_empty() || config.idp.key_namespace.trim().is_empty() {
        return Err(invalid("Canonical Issuer and key namespace are required"));
    }
    Ok(Intent {
        issuer: config.idp.oauth2.issuer.clone(),
        key_namespace: config.idp.key_namespace.clone(),
        roots,
        password_policy: serde_json::to_value(&config.idp.password)?,
        applications: [
            config.idp.api_public_uri.clone(),
            config.management.api_public_uri.clone(),
            config.storage.api_public_uri.clone(),
        ],
        audiences: [
            config.idp_audience().to_owned(),
            config.management.storage_audience.clone(),
        ],
    })
}

pub(crate) fn login_registration(config: &UnifiedConfig, client_id: Id) -> ClientRegistration {
    ClientRegistration {
        application: ApplicationRegistration {
            name: Some("Local Node operator".into()),
            uri: format!(
                "{}/operator",
                config.idp.api_public_uri.trim_end_matches('/')
            ),
            description: None,
        },
        client_id: Some(client_id.to_string()),
        client_secret: None,
        client_id_issued_at: None,
        client_secret_expires_at: None,
        client_name: "Local Node operator".into(),
        client_uri: None,
        logo_uri: None,
        contacts: Vec::new(),
        terms_of_service_uri: None,
        policy_uri: None,
        client_type: ClientType::Public,
        profile: ClientProfile::Native,
        redirect_uris: Vec::new(),
        allowed_grant_types: vec![
            GrantType::Password,
            GrantType::RefreshToken,
            GrantType::TokenExchange,
        ],
        response_types: Vec::new(),
        allowed_scopes: vec!["openid".into(), "profile".into(), "email".into()],
        allowed_audiences: vec![
            config.idp_audience().to_owned(),
            management_service::MANAGEMENT_APPLICATION_URI.into(),
            config.management.storage_audience.clone(),
            "storage:databases".into(),
            "storage:filesystems".into(),
        ],
        token_endpoint_auth_method: TokenEndpointAuthMethod::None,
        software_statement: None,
        software_id: None,
        software_version: None,
    }
}

fn binding(journal: &Journal) -> io::Result<NodeSignerBinding> {
    let endpoint = journal
        .endpoint
        .clone()
        .ok_or_else(|| invalid("retained endpoint identity is required"))?;
    endpoint
        .parse::<iroh::EndpointId>()
        .map_err(|_| invalid("invalid retained endpoint identity"))?;
    Ok(NodeSignerBinding {
        member_id: journal.member,
        key_id: journal.key,
        endpoint_id: endpoint,
    })
}

fn same_binding(left: &NodeSignerBinding, right: &NodeSignerBinding) -> bool {
    left.member_id == right.member_id
        && left.key_id == right.key_id
        && left.endpoint_id == right.endpoint_id
}

pub(crate) fn normalized_config(mut config: UnifiedConfig) -> io::Result<UnifiedConfig> {
    config.endpoint_data_dir = absolute(&config.endpoint_data_dir)?;
    config.idp.data_dir = absolute(Path::new(&config.idp.data_dir))?
        .to_string_lossy()
        .into_owned();
    config.management.data_dir = absolute(Path::new(&config.management.data_dir))?
        .to_string_lossy()
        .into_owned();
    config.storage.data_dir = absolute(Path::new(&config.storage.data_dir))?
        .to_string_lossy()
        .into_owned();
    intent(&config)?;
    Ok(config)
}

pub(crate) fn absolute(path: &Path) -> io::Result<PathBuf> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(invalid("empty paths and parent components are not allowed"));
    }
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    Ok(path
        .components()
        .filter(|part| !matches!(part, Component::CurDir))
        .collect())
}

fn reject_symlinks(path: &Path) -> io::Result<()> {
    for ancestor in absolute(path)?.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(invalid("symlink ancestry is not allowed"));
            }
            Ok(_) => (),
            Err(error) if error.kind() == io::ErrorKind::NotFound => (),
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

pub(crate) fn require_absent(path: &Path) -> io::Result<()> {
    reject_symlinks(path)?;
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
        Ok(_) => Err(invalid(
            "fresh setup requires absent roots; existing state requires exact continuation or recovery",
        )),
    }
}

pub(crate) fn require_retained_directory(path: &Path) -> io::Result<fs::Metadata> {
    reject_symlinks(path)?;
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() {
        return Err(invalid("managed root must be a regular directory"));
    }
    Ok(metadata)
}

fn require_directory(path: &Path) -> io::Result<()> {
    private_permissions(&require_retained_directory(path)?)
}

pub(crate) fn require_file(path: &Path, private: bool) -> io::Result<File> {
    reject_symlinks(path)?;
    if !fs::symlink_metadata(path)?.is_file() {
        return Err(invalid("input or retained state must be a regular file"));
    }
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(invalid("input or retained state must be a regular file"));
    }
    if private {
        private_permissions(&metadata)?;
    }
    Ok(file)
}

fn private_permissions(metadata: &fs::Metadata) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 == 0 {
            return Ok(());
        }
        Err(invalid(
            "operator files/directories must deny group and other access",
        ))
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "local provisioning requires Unix owner-only permissions; no Windows ACL qualification",
        ))
    }
}

pub(crate) fn require_unix_bootstrap() -> io::Result<()> {
    #[cfg(unix)]
    {
        Ok(())
    }
    #[cfg(not(unix))]
    {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Node bootstrap supports Unix owner-only permissions only",
        ))
    }
}

pub(crate) fn create_private_directory(path: &Path, recursive: bool) -> io::Result<()> {
    require_unix_bootstrap()?;
    reject_symlinks(path)?;
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.recursive(recursive).create(path)?;
    sync_ancestry(path)
}

pub(crate) fn require_storage(config: &UnifiedConfig) -> io::Result<()> {
    let root = Path::new(&config.storage.data_dir);
    require_retained_directory(root)?;
    for name in ["databases", "filesystems"] {
        let path = root.join(name);
        reject_symlinks(&path)?;
        if !fs::symlink_metadata(path)?.is_dir() {
            return Err(invalid(
                "acknowledged Storage runtime roots require exact recovery",
            ));
        }
    }
    Ok(())
}

pub(crate) fn private_options(create_new: bool) -> OpenOptions {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(create_new);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
}

pub(crate) fn lock(directory: &Path, fresh: bool) -> io::Result<File> {
    require_directory(directory)?;
    let path = directory.join("operation.lock");
    if !fresh {
        require_file(&path, true)?;
    }
    let file = private_options(fresh).open(path)?;
    file.try_lock().map_err(|error| match error {
        fs::TryLockError::WouldBlock => io::Error::new(
            io::ErrorKind::WouldBlock,
            "another local provisioning/runtime operation owns this host",
        ),
        fs::TryLockError::Error(error) => error,
    })?;
    file.sync_all()?;
    sync_ancestry(directory)?;
    Ok(file)
}

pub(crate) fn read_private(path: &Path) -> io::Result<Vec<u8>> {
    let mut file = require_file(path, true)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

pub(crate) fn read_password(path: &Path) -> io::Result<String> {
    let mut password = String::from_utf8(read_private(path)?)
        .map_err(|_| invalid("password input must be UTF-8"))?;
    if password.ends_with('\n') {
        password.pop();
        if password.ends_with('\r') {
            password.pop();
        }
    }
    validate_password(&password)?;
    Ok(password)
}

pub(crate) fn validate_password(password: &str) -> io::Result<()> {
    if password.trim().is_empty() || password.contains(['\r', '\n']) {
        return Err(invalid("password input must contain one nonempty line"));
    }
    Ok(())
}

fn verify(secret: &str, verifier: &str) -> io::Result<()> {
    if !argon2::verify_encoded(verifier, secret.as_bytes())
        .map_err(|_| invalid("invalid retained secret verifier"))?
    {
        return Err(invalid(
            "input credential differs from retained local intent",
        ));
    }
    Ok(())
}

pub(crate) fn sync_ancestry(directory: &Path) -> io::Result<()> {
    for ancestor in directory.ancestors() {
        File::open(ancestor)?.sync_all()?;
    }
    Ok(())
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
