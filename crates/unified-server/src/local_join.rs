use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use idp_model::{contract::ClientRegistration, model::Id};

use idp_service::oauth2::NodeSignerBinding;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::{
    SecurityProtocol, UnifiedConfig, UnifiedEndpoint, UnifiedRuntime,
    local_provisioning::{
        Intent, absolute, create_private_directory, intent, lock, private_options,
        provisioning_stage, read_private, require_absent, require_file, require_retained_directory,
        require_storage, sync_ancestry,
    },
};

#[derive(Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct JoinIntent {
    version: u8,
    configuration: Intent,
    peer: String,
    name: String,
    member: Id,
    key: Id,
    inventory_user: Id,
    login_client: Id,
    endpoint_attempted: bool,
    endpoint: Option<String>,
    inventory_attempted: bool,
    storage_attempted: bool,
    storage_initialized: bool,
    complete: bool,
}

/// Restricted local joining through the existing authenticated, purpose-bound owner protocol.
pub struct LocalJoin {
    config: UnifiedConfig,
    directory: PathBuf,
    journal: JoinIntent,
    _operation: File,
}

impl LocalJoin {
    pub fn create(config: UnifiedConfig, peer: &str, name: &str) -> io::Result<Self> {
        crate::local_provisioning::require_unix_bootstrap()?;
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
            Path::new(&config.management.data_dir),
            Path::new(&config.storage.data_dir),
        ] {
            require_absent(root)?;
        }
        let directory = absolute(&config.local_provisioning_directory())?;
        require_absent(&directory)?;
        let journal = JoinIntent {
            version: 1,
            configuration,
            peer: peer.into(),
            name: name.into(),
            member: Id::now_v7(),
            key: Id::now_v7(),
            inventory_user: Id::now_v7(),
            login_client: Id::now_v7(),
            endpoint_attempted: false,
            endpoint: None,
            inventory_attempted: false,
            storage_attempted: false,
            storage_initialized: false,
            complete: false,
        };
        create_private_directory(directory.parent().expect("absolute join parent"), true)?;
        create_private_directory(&directory, false)?;
        let operation = lock(&directory, true)?;
        let local = Self {
            config,
            directory,
            journal,
            _operation: operation,
        };
        local.write()?;
        Ok(local)
    }

    pub fn open(config: UnifiedConfig) -> io::Result<Self> {
        let config = crate::local_provisioning::normalized_config(config)?;
        let directory = absolute(&config.local_provisioning_directory())?;
        let operation = lock(&directory, false)?;
        require_absent(&directory.join("intent.json"))?;
        let bytes = read_private(&directory.join("join.json"))?;
        let journal: JoinIntent = serde_json::from_slice(&bytes)?;
        journal
            .peer
            .parse::<iroh::EndpointId>()
            .map_err(io::Error::other)?;
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
            || journal.configuration != intent(&config)?
            || journal.name.trim().is_empty()
            || ids.len() != count
            || ids.iter().any(Id::is_nil)
            || journal.endpoint.is_some() && !journal.endpoint_attempted
            || journal.inventory_attempted && journal.endpoint.is_none()
            || journal.storage_attempted && !journal.inventory_attempted
            || journal.storage_initialized && !journal.storage_attempted
            || journal.complete && !journal.storage_initialized
            || serde_json::to_value(&journal)?
                != serde_json::from_slice::<serde_json::Value>(&bytes)?
        {
            return Err(invalid(
                "invalid or conflicting join intent; exact recovery required",
            ));
        }
        if let Some(configured) = &config.idp.oauth2.local_signer
            && (configured.member_id != journal.member
                || configured.key_id != journal.key
                || Some(&configured.endpoint_id) != journal.endpoint.as_ref())
        {
            return Err(invalid(
                "configured signer conflicts with retained join binding",
            ));
        }
        Ok(Self {
            config,
            directory,
            journal,
            _operation: operation,
        })
    }

    pub(crate) fn trusted_peer(&self) -> &str {
        &self.journal.peer
    }

    pub(crate) fn is_complete(&self) -> io::Result<bool> {
        self.check_retained()?;
        Ok(self.journal.complete)
    }

    pub(crate) fn validate_retry(&self, peer: &str, name: &str) -> io::Result<()> {
        self.check_retained()?;
        if self.journal.peer != peer || self.journal.name != name {
            return Err(invalid(
                "Join retry differs from retained peer or Node name",
            ));
        }
        Ok(())
    }

    pub fn login_client(&self) -> ClientRegistration {
        crate::local_provisioning::login_registration(&self.config, self.journal.login_client)
    }

    /// Does not initialize an administrator or manufacture policy; imports approved public state.
    pub async fn join(&mut self, tls: &crate::LocalTls) -> io::Result<()> {
        crate::LocalTls::api_base(self.config.listen_addr)?;
        self.check_retained()?;
        let cancellation = CancellationToken::new();
        let runtime = provisioning_stage(|| self.prepare_runtime(false, &cancellation)).await?;
        runtime.verify_local_bootstrap(Some(tls)).await?;
        if self.journal.complete {
            return Ok(());
        }
        self.journal.complete = true;
        self.write()
    }

    /// Prepares local owners without binding or publishing a second HTTPS listener.
    /// The host owns authorization, exact intent, publication and explicit completion.
    pub async fn prepare_hosted_runtime(
        &mut self,
        cancellation: &CancellationToken,
    ) -> io::Result<UnifiedRuntime> {
        provisioning_stage(|| self.prepare_runtime(true, cancellation)).await
    }

    /// Records local completion after host publication/verification; does not retire setup admission.
    pub async fn complete_hosted(
        &mut self,
        runtime: &UnifiedRuntime,
        cancellation: &CancellationToken,
    ) -> io::Result<()> {
        check_cancellation(cancellation)?;
        self.check_retained()?;
        if !self.journal.storage_initialized
            || runtime.cancellation_token().is_cancelled()
            || runtime.endpoint_id().to_string() != self.binding()?.endpoint_id
            || !runtime.node_join_public_ready().await?
        {
            return Err(invalid(
                "hosted Join requires current approved local owner readiness",
            ));
        }
        check_cancellation(cancellation)?;
        self.journal.complete = true;
        self.write()
    }

    async fn prepare_runtime(
        &mut self,
        hosted: bool,
        cancellation: &CancellationToken,
    ) -> io::Result<UnifiedRuntime> {
        crate::LocalTls::api_base(self.config.listen_addr)?;
        check_cancellation(cancellation)?;
        self.check_retained()?;
        if self.journal.complete {
            let runtime = Box::pin(self.build(true, hosted)).await?;

            if let Err(error) = check_cancellation(cancellation) {
                runtime.shutdown().await?;
                return Err(error);
            }
            return Ok(runtime);
        }
        if self.journal.storage_attempted {
            if !self.journal.storage_initialized {
                return Err(invalid(
                    "unacknowledged Storage initialization requires recovery",
                ));
            }
            require_storage(&self.config)?;
        }
        let endpoint = if self.journal.endpoint_attempted {
            UnifiedEndpoint::open(&self.config.endpoint_data_dir, &self.config.idp).await?
        } else {
            self.journal.endpoint_attempted = true;
            self.write()?;
            UnifiedEndpoint::open_fresh(&self.config.endpoint_data_dir, &self.config.idp).await?
        };
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
        if !self.journal.storage_attempted {
            require_absent(Path::new(&self.config.storage.data_dir))?;
            self.journal.storage_attempted = true;
            self.write()?;
            create_private_directory(Path::new(&self.config.storage.data_dir), false)?;
        }
        let config = self.bound_config()?;
        let address = match &listener {
            Some(listener) => listener.local_addr()?,
            None => config.listen_addr,
        };
        let base = crate::LocalTls::api_base(address)?;
        #[cfg(test)]
        let runtime_started = std::time::Instant::now();
        let runtime = Box::pin(UnifiedRuntime::build_with_owners(config, listener, base, owners)).await?;
        #[cfg(test)]
        eprintln!("Node Join stage=runtime elapsed_ms={}", runtime_started.elapsed().as_millis());
        self.journal.storage_initialized = true;
        if let Err(error) = self.write() {
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
    ) -> io::Result<crate::runtime::RuntimeOwners> {
        let endpoint_id = endpoint.endpoint_id().to_string();
        if self
            .journal
            .endpoint
            .as_ref()
            .is_some_and(|retained| retained != &endpoint_id)
        {
            return Err(invalid("joining endpoint changed; exact recovery required"));
        }
        self.journal.endpoint = Some(endpoint_id);
        self.write()?;
        let fresh_inventory = !self.journal.inventory_attempted;
        if fresh_inventory {
            require_absent(Path::new(&self.config.idp.data_dir))?;
            require_absent(Path::new(&self.config.management.data_dir))?;
            self.journal.inventory_attempted = true;
            self.write()?;
            create_private_directory(Path::new(&self.config.idp.data_dir), false)?;
            create_private_directory(Path::new(&self.config.management.data_dir), false)?;
        } else {
            require_file(
                &Path::new(&self.config.idp.data_dir).join("idp.redb"),
                false,
            )?;
            require_file(
                &Path::new(&self.config.management.data_dir).join("management.redb"),
                false,
            )?;
        }
        let binding = self.binding()?;
        let (protocol, owner) = SecurityProtocol::open_node_bootstrap(
            &endpoint,
            &self.config.idp,
            Path::new(&self.config.management.data_dir),
            binding,
        )
        .await?;
        let clients = [self.journal.login_client];
        if fresh_inventory {
            provisioning_stage(|| {
                owner.prepare_fresh_setup_secret_inventory(self.journal.inventory_user, &clients)
            })
            .await?;
        }
        provisioning_stage(|| owner.prepare_setup_client_slots(&clients)).await?;
        #[cfg(test)]
        let started = std::time::Instant::now();
        provisioning_stage(|| {
            protocol.join_from_trusted_peer(&owner, &self.journal.name, &self.journal.peer)
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
        provisioning_stage(|| owner.ensure_setup_public_login_client(self.login_client())).await?;
        #[cfg(test)]
        eprintln!(
            "Node Join stage=login_client elapsed_ms={}",
            started.elapsed().as_millis()
        );
        Ok(protocol.into_runtime_owners(endpoint))
    }

    pub async fn build_runtime(&self) -> io::Result<UnifiedRuntime> {
        self.build(false, false).await
    }
    pub async fn build_https_runtime(&self) -> io::Result<UnifiedRuntime> {
        self.build(true, false).await
    }

    /// Reopens completed locally trusted owners without taking the host's listener or contacting a peer.
    pub async fn reopen_hosted_runtime(&self) -> io::Result<UnifiedRuntime> {
        self.build(true, true).await
    }

    async fn build(&self, https: bool, hosted: bool) -> io::Result<UnifiedRuntime> {
        self.check_retained()?;
        if !self.journal.complete {
            return Err(invalid("join is incomplete; explicitly continue join"));
        }
        for root in [
            Path::new(&self.config.idp.data_dir),
            Path::new(&self.config.management.data_dir),
            &self.config.endpoint_data_dir,
        ] {
            require_retained_directory(root)?;
        }
        require_file(
            &Path::new(&self.config.idp.data_dir).join("idp.redb"),
            false,
        )?;
        require_file(
            &Path::new(&self.config.management.data_dir).join("management.redb"),
            false,
        )?;
        require_storage(&self.config)?;
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
            provisioning_stage(|| owner.verify_public_login_client(self.login_client())).await?;
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
        Ok(NodeSignerBinding {
            member_id: self.journal.member,
            key_id: self.journal.key,
            endpoint_id: self
                .journal
                .endpoint
                .clone()
                .ok_or_else(|| invalid("missing joining endpoint"))?,
        })
    }
    fn bound_config(&self) -> io::Result<UnifiedConfig> {
        let mut config = self.config.clone();
        config.idp.oauth2.local_signer = Some(self.binding()?);
        Ok(config)
    }
    fn check_retained(&self) -> io::Result<()> {
        if read_private(&self.directory.join("join.json"))? != serde_json::to_vec(&self.journal)? {
            return Err(invalid("retained Join intent changed while guarded"));
        }
        Ok(())
    }

    fn write(&self) -> io::Result<()> {
        let bytes = serde_json::to_vec(&self.journal)?;
        let temporary = self.directory.join(format!(".{}.tmp", Id::now_v7()));
        let mut file = private_options(true).open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(temporary, self.directory.join("join.json"))?;
        sync_ancestry(&self.directory)?;
        if read_private(&self.directory.join("join.json"))? != bytes {
            return Err(invalid("join intent durable readback mismatch"));
        }
        Ok(())
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
