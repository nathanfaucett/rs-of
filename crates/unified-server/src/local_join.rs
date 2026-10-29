use std::{fs::File, io, path::Path, sync::Arc};

use idp_model::{contract::ClientRegistration, model::Id};
use idp_service::oauth2::NodeSignerBinding;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use idp_server::OwnerProvisioner;
use idp_server::local_provisioning::{
    absolute, create_private_directory, lock, provisioning_stage, require_absent,
    require_retained_directory,
};

use crate::endpoint_secret::EndpointSecret;
use crate::local_provisioning::{Intent, intent};
use crate::{RuntimeOwners, SecurityProtocol, UnifiedConfig, UnifiedEndpoint, UnifiedRuntime};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JoinIntent {
    version: u8,
    configuration: Intent,
    member: Id,
    key: Id,
    inventory_user: Id,
    login_client: Id,
    endpoint_attempted: bool,
    #[serde(deserialize_with = "Deserialize::deserialize")]
    endpoint_secret: Option<EndpointSecret>,
    endpoint: Option<String>,
    inventory_attempted: bool,
    storage_attempted: bool,
    storage_initialized: bool,
    verified: bool,
    complete: bool,
}

/// Restricted local joining through the existing authenticated, purpose-bound owner protocol.
pub struct LocalJoin {
    config: crate::UnifiedConfig,
    store: crate::host_store::HostStore,
    _operation: File,
}

impl LocalJoin {
    pub fn create(config: UnifiedConfig, peer: &str, name: &str) -> io::Result<Self> {
        idp_server::local_provisioning::require_unix_bootstrap()?;
        let config = crate::local_provisioning::normalized_config(config)?;
        peer.parse::<iroh::EndpointId>().map_err(io::Error::other)?;
        if name.trim().is_empty() || config.idp.oauth2.local_signer.is_some() {
            return Err(invalid(
                "fresh join requires a name and no configured Node signer",
            ));
        }
        let configuration = intent(&config)?;
        for root in [
            config.endpoint_data_dir.as_path(),
            Path::new(&config.idp.data_dir),
            Path::new(&config.device.data_dir),
            Path::new(&config.storage.data_dir),
        ] {
            require_absent(root)?;
        }
        let directory = absolute(&config.local_provisioning_directory())?;
        require_absent(&directory)?;
        let journal = JoinIntent {
            version: 1,
            configuration,
            member: Id::now_v7(),
            key: Id::now_v7(),
            inventory_user: Id::now_v7(),
            login_client: Id::now_v7(),
            endpoint_attempted: false,
            endpoint_secret: None,
            endpoint: None,
            inventory_attempted: false,
            storage_attempted: false,
            storage_initialized: false,
            verified: false,
            complete: false,
        };
        create_private_directory(directory.parent().expect("absolute join parent"), true)?;
        create_private_directory(&directory, false)?;
        let operation = lock(&directory, true)?;
        let store = crate::host_store::HostStore::create_for_config(&config)?;
        if store.get::<serde_json::Value>("create")?.is_some()
            || store.get::<serde_json::Value>("join")?.is_some()
        {
            return Err(invalid("conflicting retained host operation"));
        }
        let local = Self {
            config,
            store,
            _operation: operation,
        };
        local.store.update(|records| {
            if records.contains_key("create") || records.contains_key("join") {
                return Err(invalid("conflicting retained host operation"));
            }
            let canonical = match records.get("operation") {
                Some(value) => serde_json::from_value::<String>(value.clone())?,
                None => idp_server::hosted_setup::JoinIntent {
                    operation_nonce: Id::now_v7().simple().to_string(),
                    node: name.into(),
                    peer: peer.into(),
                    issuer: local.config.idp.oauth2.issuer.clone(),
                }
                .canonical()
                .map_err(|_| invalid("invalid Join operation"))?,
            };
            let operation = decode_operation(&canonical, &local.config)?;
            if operation.peer != peer || operation.node != name {
                return Err(invalid("Join choices conflict with canonical operation"));
            }
            records.insert("operation".into(), serde_json::json!(canonical));
            records.insert("join".into(), serde_json::to_value(&journal)?);
            Ok(())
        })?;
        Ok(local)
    }

    pub fn open(config: UnifiedConfig) -> io::Result<Self> {
        let config = crate::local_provisioning::normalized_config(config)?;
        let directory = absolute(&config.local_provisioning_directory())?;
        let operation = lock(&directory, false)?;
        require_absent(&directory.join("intent.json"))?;
        require_absent(&directory.join("join.json"))
            .map_err(|_| invalid("unsupported legacy Join journal; explicit reset required"))?;
        let store = crate::host_store::HostStore::for_config(&config)?;
        if store.get::<serde_json::Value>("create")?.is_some() {
            return Err(invalid("conflicting Create state"));
        }
        let local = Self {
            config,
            store,
            _operation: operation,
        };
        local.read()?;
        Ok(local)
    }

    /// Prepares this retained operation's endpoint; every retry is existing-only.
    pub async fn prepare_endpoint(&mut self) -> io::Result<UnifiedEndpoint> {
        let mut journal = self.read()?;
        if journal.endpoint_attempted {
            return UnifiedEndpoint::open(&self.config.endpoint_data_dir, &self.config.idp).await;
        }
        idp_server::local_provisioning::require_absent(&self.config.endpoint_data_dir)?;
        let secret = iroh::SecretKey::generate();
        let reference = EndpointSecret::fresh();
        journal.endpoint = Some(secret.public().to_string());
        journal.endpoint_secret = Some(reference.clone());
        journal.endpoint_attempted = true;
        self.write(&journal)?;
        UnifiedEndpoint::create_prepared(
            &self.config.endpoint_data_dir,
            &self.config.idp,
            &reference,
            secret,
        )
        .await
    }

    pub fn reset_references(&self) -> io::Result<serde_json::Value> {
        let journal = self.read()?;
        Ok(serde_json::json!({
            "member_id": journal.member,
            "signer_key_id": journal.key,
            "owner_id": journal.inventory_user,
            "clients": [journal.login_client],
            "inventory_attempted": journal.inventory_attempted,
            "endpoint_secret": journal.endpoint_secret,
        }))
    }

    pub fn trusted_peer(&self) -> io::Result<String> {
        Ok(self.operation()?.peer)
    }

    pub fn is_complete(&self) -> io::Result<bool> {
        Ok(self.read()?.complete)
    }

    pub fn validate_retry(&self, peer: &str, name: &str) -> io::Result<()> {
        self.read()?;
        let operation = self.operation()?;
        if operation.peer != peer || operation.node != name {
            return Err(invalid(
                "Join retry differs from retained peer or Node name",
            ));
        }
        Ok(())
    }

    pub fn login_client(&self) -> io::Result<ClientRegistration> {
        Ok(crate::local_provisioning::login_registration(
            &self.config,
            self.read()?.login_client,
        ))
    }

    /// Does not initialize an administrator or manufacture policy; imports approved public state.
    pub async fn join(&mut self, tls: &crate::LocalTls) -> io::Result<()> {
        crate::LocalTls::api_base(self.config.listen_addr)?;
        let cancellation = CancellationToken::new();
        let runtime =
            provisioning_stage(|| self.prepare_runtime(true, false, &cancellation)).await?;
        runtime.verify_local_bootstrap(Some(tls)).await?;
        let mut journal = self.read()?;
        if journal.complete {
            return Ok(());
        }
        if self.store.get::<serde_json::Value>("admission")?.is_some() {
            return Err(invalid(
                "hosted Join completes only with atomic admission retirement",
            ));
        }
        journal.verified = true;
        journal.complete = true;
        self.write(&journal)
    }

    /// HTTP join for server deployments behind a terminating proxy.
    pub async fn join_http(&mut self) -> io::Result<()> {
        let cancellation = CancellationToken::new();
        let runtime =
            provisioning_stage(|| self.prepare_runtime(false, false, &cancellation)).await?;
        runtime.verify_local_bootstrap(None).await?;
        let mut journal = self.read()?;
        if journal.complete {
            return Ok(());
        }
        if self.store.get::<serde_json::Value>("admission")?.is_some() {
            return Err(invalid(
                "hosted Join completes only with atomic admission retirement",
            ));
        }
        journal.verified = true;
        journal.complete = true;
        self.write(&journal)
    }

    /// Prepares local owners without binding or publishing a second HTTPS listener.
    /// The host owns authorization, exact intent, publication and explicit completion.
    pub async fn prepare_hosted_runtime(
        &mut self,
        https: bool,
        cancellation: &CancellationToken,
    ) -> io::Result<UnifiedRuntime> {
        provisioning_stage(|| self.prepare_runtime(https, true, cancellation)).await
    }

    /// Acknowledges verified publication; hosted completion belongs to atomic admission retirement.
    pub async fn complete_hosted(
        &mut self,
        runtime: &UnifiedRuntime,
        cancellation: &CancellationToken,
    ) -> io::Result<()> {
        check_cancellation(cancellation)?;
        let journal = self.read()?;
        if !journal.storage_initialized
            || runtime.cancellation_token().is_cancelled()
            || runtime.endpoint_id().to_string() != self.binding()?.endpoint_id
            || !runtime.node_join_public_ready().await?
        {
            return Err(invalid(
                "hosted Join requires current approved local owner readiness",
            ));
        }
        check_cancellation(cancellation)?;
        let mut journal = self.read()?;
        if journal.complete {
            return Ok(());
        }
        journal.verified = true;
        journal.complete = self.store.get::<serde_json::Value>("admission")?.is_none();
        self.write(&journal)
    }

    async fn prepare_runtime(
        &mut self,
        https: bool,
        hosted: bool,
        cancellation: &CancellationToken,
    ) -> io::Result<UnifiedRuntime> {
        if https {
            crate::LocalTls::api_base(self.config.listen_addr)?;
        }
        check_cancellation(cancellation)?;
        let journal = self.read()?;
        if journal.complete {
            let runtime = Box::pin(self.build(https, hosted)).await?;

            if let Err(error) = check_cancellation(cancellation) {
                runtime.shutdown().await?;
                return Err(error);
            }
            return Ok(runtime);
        }
        if journal.storage_attempted {
            if !journal.storage_initialized {
                return Err(invalid(
                    "unacknowledged Storage initialization requires recovery",
                ));
            }
            self.config.storage.require_provisioned_storage()?;
        }
        let endpoint = self.prepare_endpoint().await?;
        let endpoint = Arc::new(endpoint);
        let result = async {
        let owners = tokio::select! {
            biased;
            () = cancellation.cancelled() => Err(cancelled()),
            result = provisioning_stage(|| self.join_on_endpoint(Arc::clone(&endpoint))) => result,
        }?;
        check_cancellation(cancellation)?;
        let listener = if hosted {
            None
        } else {
            Some(tokio::net::TcpListener::bind(self.config.listen_addr).await?)
        };
        let mut journal = self.read()?;
        if !journal.storage_attempted {
            require_absent(Path::new(&self.config.storage.data_dir))?;
            journal.storage_attempted = true;
            self.write(&journal)?;
            create_private_directory(Path::new(&self.config.storage.data_dir), false)?;
        }
        let config = self.bound_config()?;
        let address = match &listener {
            Some(listener) => listener.local_addr()?,
            None => config.listen_addr,
        };
        let base = if https {
            crate::LocalTls::api_base(address)?
        } else {
            format!("http://{address}")
        };
        #[cfg(test)]
        let runtime_started = std::time::Instant::now();
        let runtime = Box::pin(UnifiedRuntime::build_with_owners(config, listener, base, owners)).await?;
        #[cfg(test)]
        eprintln!("Node Join stage=runtime elapsed_ms={}", runtime_started.elapsed().as_millis());
        let acknowledged = self.read().and_then(|mut journal| {
            journal.storage_initialized = true;
            self.write(&journal)
        });
        if let Err(error) = acknowledged {
            runtime.shutdown().await?;
            return Err(error);
        }

        if let Err(error) = check_cancellation(cancellation) {
            runtime.shutdown().await?;
            return Err(error);
        }
        Ok(runtime)
        }.await;
        if result.is_err() {
            endpoint.close().await;
        }
        result
    }

    async fn join_on_endpoint(
        &mut self,
        endpoint: Arc<UnifiedEndpoint>,
    ) -> io::Result<RuntimeOwners> {
        let mut journal = self.read()?;
        let operation = self.operation()?;
        let endpoint_id = endpoint.endpoint_id().to_string();
        if journal
            .endpoint
            .as_ref()
            .is_some_and(|retained| retained != &endpoint_id)
        {
            return Err(invalid("joining endpoint changed; exact recovery required"));
        }
        journal.endpoint = Some(endpoint_id);
        self.write(&journal)?;
        let fresh_inventory = !journal.inventory_attempted;
        if fresh_inventory {
            require_absent(Path::new(&self.config.idp.data_dir))?;
            require_absent(Path::new(&self.config.device.data_dir))?;
            journal.inventory_attempted = true;
            self.write(&journal)?;
            create_private_directory(Path::new(&self.config.idp.data_dir), false)?;
            create_private_directory(Path::new(&self.config.device.data_dir), false)?;
        } else {
            self.config.idp.require_provisioned_identity()?;
            self.config.device.require_provisioned_device()?;
        }
        let binding = self.binding()?;
        device_service::node_join_owner::check_bootstrap_endpoint(
            endpoint.endpoint_id(),
            &binding,
        )?;
        let owner = OwnerProvisioner::open(&self.config.idp).await?;
        let device_directory = Path::new(&self.config.device.data_dir);
        let devices = device_server::open_local_device_database(device_directory).await?;
        let idp = owner.node_join_database()?;
        let protocol = SecurityProtocol::new(
            endpoint.server().clone(),
            Arc::clone(&idp),
            devices,
            idp,
            self.config.idp.oauth2.issuer.clone(),
            Some(binding),
            device_directory.join("node-bootstrap-staging"),
            CancellationToken::new(),
        );
        let clients = [journal.login_client];
        if fresh_inventory {
            provisioning_stage(|| {
                owner.prepare_fresh_setup_secret_inventory(journal.inventory_user, &clients)
            })
            .await?;
        }
        provisioning_stage(|| owner.prepare_setup_client_slots(&clients)).await?;
        #[cfg(test)]
        let started = std::time::Instant::now();
        provisioning_stage(|| {
            protocol.join_from_trusted_peer(&owner, &operation.node, &operation.peer)
        })
        .await
        .inspect_err(|_error| {
            #[cfg(test)]
            eprintln!("Node Join stage=public_bootstrap error={_error}");
        })?;
        #[cfg(test)]
        eprintln!(
            "Node Join stage=public_bootstrap elapsed_ms={}",
            started.elapsed().as_millis()
        );
        #[cfg(test)]
        let started = std::time::Instant::now();
        let login = self.login_client()?;
        provisioning_stage(|| owner.ensure_setup_public_login_client(login.clone())).await?;
        #[cfg(test)]
        eprintln!(
            "Node Join stage=login_client elapsed_ms={}",
            started.elapsed().as_millis()
        );
        let (idp, devices) = protocol.into_databases();
        Ok(RuntimeOwners {
            endpoint,
            idp,
            devices,
        })
    }

    pub async fn build_runtime(&self) -> io::Result<UnifiedRuntime> {
        self.build(false, false).await
    }
    pub async fn build_https_runtime(&self) -> io::Result<UnifiedRuntime> {
        self.build(true, false).await
    }

    /// Reopens completed locally trusted owners without taking the host's listener or contacting a peer.
    pub async fn reopen_hosted_runtime(&self, https: bool) -> io::Result<UnifiedRuntime> {
        self.build(https, true).await
    }

    async fn build(&self, https: bool, hosted: bool) -> io::Result<UnifiedRuntime> {
        if !self.read()?.complete {
            return Err(invalid("join is incomplete; explicitly continue join"));
        }
        for root in [
            Path::new(&self.config.idp.data_dir),
            Path::new(&self.config.device.data_dir),
            &self.config.endpoint_data_dir,
        ] {
            require_retained_directory(root)?;
        }
        self.config.idp.require_provisioned_identity()?;
        self.config.device.require_provisioned_device()?;
        self.config.storage.require_provisioned_storage()?;
        if UnifiedEndpoint::persisted_endpoint_id(&self.config.endpoint_data_dir)?.to_string()
            != self.binding()?.endpoint_id
        {
            return Err(invalid("joining endpoint changed; exact recovery required"));
        }
        let config = self.bound_config()?;
        let runtime = if hosted {
            Box::pin(UnifiedRuntime::build_hosted_https(config)).await?
        } else if https {
            Box::pin(UnifiedRuntime::build_https(config)).await?
        } else {
            Box::pin(UnifiedRuntime::build(config)).await?
        };
        let verified = async {
            #[cfg(test)]
            eprintln!("Node Join reopen stage=signer");
            let owner = runtime.join_owner();
            let binding = self.binding()?;
            provisioning_stage(|| owner.verify_node_signer(&binding)).await?;
            #[cfg(test)]
            eprintln!("Node Join reopen stage=login_client");
            let login = self.login_client()?;
            provisioning_stage(|| owner.verify_public_login_client(login.clone())).await?;
            #[cfg(test)]
            eprintln!("Node Join reopen stage=readiness");
            if !runtime.node_join_public_ready().await? {
                return Err(invalid("joined Node is not currently ready"));
            }
            Ok::<_, io::Error>(())
        }
        .await;
        if let Err(error) = verified {
            runtime.shutdown().await?;
            return Err(error);
        }
        Ok(runtime)
    }

    fn binding(&self) -> io::Result<NodeSignerBinding> {
        let journal = self.read()?;
        self.config.idp.retained_node_signer_binding(
            journal.member,
            journal.key,
            journal.endpoint.as_deref(),
        )
    }
    fn bound_config(&self) -> io::Result<UnifiedConfig> {
        let mut config = self.config.clone();
        config.idp.oauth2.local_signer = Some(self.binding()?);
        Ok(config)
    }
    fn operation(&self) -> io::Result<idp_server::hosted_setup::JoinIntent> {
        let canonical: String = self
            .store
            .get("operation")?
            .ok_or_else(|| invalid("missing Join binding"))?;
        decode_operation(&canonical, &self.config)
    }

    fn read(&self) -> io::Result<JoinIntent> {
        self.operation()?;
        let journal: JoinIntent = self
            .store
            .get("join")?
            .ok_or_else(|| invalid("missing retained Join state"))?;
        let mut ids = vec![
            journal.member,
            journal.key,
            journal.inventory_user,
            journal.login_client,
        ];
        let count = ids.len();
        ids.sort();
        ids.dedup();
        if journal.version != 1
            || journal.configuration != intent(&self.config)?
            || ids.len() != count
            || ids.iter().any(Id::is_nil)
            || journal.endpoint.is_some() != journal.endpoint_attempted
            || journal.endpoint_secret.is_some() != journal.endpoint_attempted
            || journal.inventory_attempted && journal.endpoint.is_none()
            || journal.storage_attempted && !journal.inventory_attempted
            || journal.storage_initialized && !journal.storage_attempted
            || journal.verified && !journal.storage_initialized
            || journal.complete && !journal.verified
        {
            return Err(invalid(
                "invalid or conflicting join intent; exact recovery required",
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

    fn write(&self, journal: &JoinIntent) -> io::Result<()> {
        self.store.put("join", journal)
    }
}

fn decode_operation(
    canonical: &str,
    config: &UnifiedConfig,
) -> io::Result<idp_server::hosted_setup::JoinIntent> {
    match idp_server::hosted_setup::SetupIntent::decode(canonical)
        .map_err(|_| invalid("invalid Join binding"))?
    {
        idp_server::hosted_setup::SetupIntent::Join(operation)
            if operation.issuer == config.idp.oauth2.issuer =>
        {
            Ok(operation)
        }
        _ => Err(invalid("Join operation conflicts with host configuration")),
    }
}

fn check_cancellation(cancellation: &CancellationToken) -> io::Result<()> {
    if cancellation.is_cancelled() {
        Err(cancelled())
    } else {
        Ok(())
    }
}

fn cancelled() -> io::Error {
    io::Error::new(
        io::ErrorKind::Interrupted,
        "Join cancelled; retained owner state requires exact continuation",
    )
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

pub(crate) fn endpoint_reference(
    store: &crate::host_store::HostStore,
) -> io::Result<(EndpointSecret, iroh::EndpointId)> {
    let journal: JoinIntent = store
        .get("join")?
        .ok_or_else(|| invalid("missing Join endpoint metadata"))?;
    if journal.version != 1 || !journal.endpoint_attempted {
        return Err(invalid("invalid Join endpoint owner metadata"));
    }
    Ok((
        journal
            .endpoint_secret
            .ok_or_else(|| invalid("missing endpoint provider reference"))?,
        journal
            .endpoint
            .ok_or_else(|| invalid("missing endpoint public identity"))?
            .parse()
            .map_err(io::Error::other)?,
    ))
}
