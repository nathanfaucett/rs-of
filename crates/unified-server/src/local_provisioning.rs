use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use idp_model::{
    contract::{ClientRegistration, JwkPublic},
    model::Id,
};
use idp_server::OwnerProvisioner;
use idp_server::local_provisioning::{
    LocalPasswordVerifier, absolute, create_private_directory, lock, private_options,
    provisioning_stage, read_password, read_private, reject_symlinks, require_absent,
    require_directory, require_file, require_retained_directory, require_unix_bootstrap,
    sync_ancestry, validate_password,
};
use management_server::InitialAdministratorGrant;
use serde::{Deserialize, Serialize};

use crate::{RuntimeOwners, UnifiedConfig, UnifiedEndpoint, UnifiedRuntime};

#[derive(Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Intent {
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
    #[serde(flatten)]
    administrator_grant: InitialAdministratorGrant,
    administrator: String,
    node: String,
    password_verifier: LocalPasswordVerifier,
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
                crate::config::validate_config(&config)?;
                let binding = config.idp.oauth2.local_signer.as_ref().ok_or_else(|| {
                    invalid("existing host requires an explicit retained Node signer binding")
                })?;
                for root in [
                    Path::new(&config.idp.data_dir),
                    Path::new(&config.device.data_dir),
                    Path::new(&config.storage.data_dir),
                    &config.endpoint_data_dir,
                ] {
                    require_retained_directory(root)?;
                }
                config.idp.require_provisioned_identity()?;
                config.device.require_provisioned_device()?;
                require_file(&config.endpoint_data_dir.join("endpoint.key"), true)?;
                config.storage.require_provisioned_storage()?;
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
            administrator_grant: InitialAdministratorGrant::fresh(),
            administrator: administrator.to_owned(),
            node: node.to_owned(),
            password_verifier: LocalPasswordVerifier::capture(&config.idp.password, password)?,
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

    /// Continues only the retained exact local intent over HTTP.
    pub async fn provision_http(&mut self, password: &str) -> io::Result<()> {
        self.provision_with_tls(password, None).await
    }

    /// Verifies local bootstrap readiness without TLS (HTTP).
    pub async fn verify_http(&self) -> io::Result<()> {
        self.build_runtime_for_transport(false)
            .await?
            .verify_local_bootstrap(None)
            .await
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

    pub fn validate_retry(
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
        journal.password_verifier.verify(password)
    }

    pub async fn prepare_hosted_runtime(
        &mut self,
        password: &str,
        https: bool,
        cancellation: &tokio_util::sync::CancellationToken,
    ) -> io::Result<UnifiedRuntime> {
        Box::pin(self.prepare_runtime(password, https, true, cancellation)).await
    }

    pub fn mark_complete(&self) -> io::Result<()> {
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
        journal.password_verifier.verify(password)?;
        if journal.complete {
            return Box::pin(self.build_runtime_for_host(https, hosted)).await;
        }
        if journal.management_attempted {
            require_directory(Path::new(&self.config.device.data_dir))?;
            self.config.device.require_provisioned_device()?;
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
            self.config.storage.require_provisioned_storage()?;
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
                self.config.idp.require_provisioned_identity()?;
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
            let binding = self.config.idp.retained_node_signer_binding(
                journal.member,
                journal.key,
                journal.endpoint.as_deref(),
            )?;
            let attempted = journal.node_attempted;
            if !attempted {
                journal.node_attempted = true;
                self.write(&journal)?;
            }
            let public = owner.prepare_retained_node_signer(
                &binding,
                attempted,
                journal.public_signer.as_ref(),
            )?;
            journal.public_signer = Some(public);
            self.write(&journal)?;
            provisioning_stage(|| owner.ensure_initial_node(&binding, journal.user, &journal.node))
                .await?;
            provisioning_stage(|| {
                owner.verify_retained_node_signer(&binding, journal.public_signer.as_ref())
            })
            .await?;
            provisioning_stage(|| {
                owner.ensure_setup_public_login_client(login_registration(
                    &self.config,
                    journal.login_client,
                ))
            })
            .await?;
            let idp_engine = owner.node_join_database()?;
            log::info!(
                "Node Create stage=idp elapsed_ms={}",
                started.elapsed().as_millis()
            );
            let started = std::time::Instant::now();
            check_cancellation(cancellation)?;
            if !journal.management_attempted {
                require_absent(Path::new(&self.config.device.data_dir))?;
                journal.management_attempted = true;
                self.write(&journal)?;
                create_private_directory(Path::new(&self.config.device.data_dir), false)?;
            }
            let management_engine =
                device_server::open_device_database(&self.config.device).await?;
            if !journal.management_complete {
                provisioning_stage(|| {
                    journal
                        .administrator_grant
                        .provision(Arc::clone(&idp_engine), journal.user)
                })
                .await?;
                let endpoint_id = binding.endpoint_id.clone();
                let node_name = journal.node.clone();
                provisioning_stage(|| {
                    device_server::provision_initial_device(
                        management_engine.client(),
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
                None if !https => format!("http://{}", self.config.listen_addr),
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
                RuntimeOwners {
                    endpoint: Arc::clone(&endpoint),
                    idp: idp_engine,
                    devices: management_engine,
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

    pub async fn reopen_hosted_runtime(&self, https: bool) -> io::Result<UnifiedRuntime> {
        self.build_runtime_for_host(https, true).await
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
        self.config.idp.require_provisioned_identity()?;
        self.config.device.require_provisioned_device()?;
        require_file(&self.config.endpoint_data_dir.join("endpoint.key"), true)?;
        self.config.storage.require_provisioned_storage()?;
        let binding = self.config.idp.retained_node_signer_binding(
            journal.member,
            journal.key,
            journal.endpoint.as_deref(),
        )?;
        if UnifiedEndpoint::persisted_endpoint_id(&self.config.endpoint_data_dir)?.to_string()
            != binding.endpoint_id
        {
            return Err(invalid(
                "endpoint identity changed; exact recovery required",
            ));
        }
        let mut config = self.config.clone();
        config.idp.oauth2.local_signer = Some(binding.clone());
        let owners = RuntimeOwners::open(&config).await?;
        let result = async {
            let owner = OwnerProvisioner::from_database(&config.idp, Arc::clone(&owners.idp))?;
            owner
                .verify_retained_node_signer(&binding, journal.public_signer.as_ref())
                .await?;
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
                RuntimeOwners {
                    endpoint: Arc::clone(&owners.endpoint),
                    idp: Arc::clone(&owners.idp),
                    devices: Arc::clone(&owners.devices),
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

    pub fn reset_references(&self) -> io::Result<serde_json::Value> {
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
        ];
        journal.administrator_grant.validate()?;
        identities.extend(journal.administrator_grant.identities());
        identities.push(journal.login_client);
        let count = identities.len();
        identities.sort();
        identities.dedup();
        if identities.len() != count || identities.iter().any(Id::is_nil) {
            return Err(invalid(
                "local intent identities must be distinct and non-nil",
            ));
        }

        if self.config.idp.oauth2.local_signer.is_some() {
            self.config.idp.retained_node_signer_binding(
                journal.member,
                journal.key,
                journal.endpoint.as_deref(),
            )?;
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

pub fn intent(config: &UnifiedConfig) -> io::Result<Intent> {
    crate::config::validate_config(config)?;
    crate::LocalTls::validate_ip(config.listen_addr.ip())?;
    if !config.listen_addr.ip().is_loopback() {
        return Err(invalid("local host must use a loopback listener"));
    }
    let roots = [
        PathBuf::from(&config.idp.data_dir),
        PathBuf::from(&config.device.data_dir),
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
    config.idp.validate_installation_settings()?;
    Ok(Intent {
        issuer: config.idp.oauth2.issuer.clone(),
        key_namespace: config.idp.key_namespace.clone(),
        roots,
        password_policy: serde_json::to_value(&config.idp.password)?,
        applications: [
            config.idp.api_public_uri.clone(),
            config.device.api_public_uri.clone(),
            config.storage.api_public_uri.clone(),
        ],
        audiences: [
            config.idp_audience().to_owned(),
            config.device.storage_audience.clone(),
        ],
    })
}

pub fn login_registration(config: &UnifiedConfig, client_id: Id) -> ClientRegistration {
    config.idp.operator_login_registration(
        client_id,
        vec![
            config.idp_audience().to_owned(),
            management_server::MANAGEMENT_APPLICATION_URI.into(),
            config.device.storage_audience.clone(),
            "storage:databases".into(),
            "storage:filesystems".into(),
        ],
    )
}

pub fn normalized_config(mut config: UnifiedConfig) -> io::Result<UnifiedConfig> {
    config.endpoint_data_dir = absolute(&config.endpoint_data_dir)?;
    config.idp.data_dir = absolute(Path::new(&config.idp.data_dir))?
        .to_string_lossy()
        .into_owned();
    config.device.data_dir = absolute(Path::new(&config.device.data_dir))?
        .to_string_lossy()
        .into_owned();
    config.storage.data_dir = absolute(Path::new(&config.storage.data_dir))?
        .to_string_lossy()
        .into_owned();
    intent(&config)?;
    Ok(config)
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
