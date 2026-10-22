use crate::{
    BootstrapConfig, BootstrapPurpose, CredentialBootstrap, LocalJoin, LocalProvisioner, LocalTls,
    UnifiedRuntime,
    bootstrap_config::present,
    local_provisioning::{provisioning_stage, require_absent},
};
use api::SetupAdmission;
use axum::{Router, http::StatusCode};
use idp_server::hosted_setup::{
    CreateRequest, JoinIntent, JoinRequest, LoginClient, SetupIntent, begin_join_attempt,
    join_attempted,
};
use idp_server::{
    HostedSetup, HostedSetupControl, HostedSetupOwners, HostedSetupReady, HostedSetupSettings,
    SetupCommand, SetupOperation, SetupPurpose,
};
use std::{io, sync::Arc};
use tokio_util::sync::CancellationToken;

pub type ApiSetupHost = HostedSetup<RuntimeSetupOwners>;

pub fn setup_worker(
    host: BootstrapConfig,
    tls: impl Into<Option<LocalTls>>,
    admission: Arc<SetupAdmission>,
    shutdown: CancellationToken,
) -> ApiSetupHost {
    HostedSetup::new(
        RuntimeSetupOwners {
            host,
            tls: tls.into(),
            runtime: None,
            local: None,
            joining: None,
            credentials: None,
        },
        admission,
        shutdown,
    )
}

pub struct RuntimeSetupOwners {
    host: BootstrapConfig,
    tls: Option<LocalTls>,
    runtime: Option<UnifiedRuntime>,
    local: Option<LocalProvisioner>,
    joining: Option<LocalJoin>,
    credentials: Option<CredentialBootstrap>,
}

impl RuntimeSetupOwners {
    async fn reopen_create(
        &mut self,
        retired: bool,
        control: &HostedSetupControl,
    ) -> io::Result<()> {
        let started = std::time::Instant::now();
        let config = self.host.config();
        if !present(&config.credential_bootstrap_directory())? {
            return Ok(());
        }
        let owner_root = config.local_provisioning_directory();
        self.credentials = Some(CredentialBootstrap::open(config)?);
        if !present(&owner_root)? {
            return Ok(());
        }
        let config = self
            .credentials
            .as_ref()
            .expect("retained credential references")
            .load()?;
        let local = LocalProvisioner::open(config)?;
        log::info!(
            "Node reopen stage=credentials elapsed_ms={}",
            started.elapsed().as_millis()
        );
        let started = std::time::Instant::now();
        if local.is_complete()? && retired {
            let runtime = local.reopen_hosted_runtime(self.tls.is_some()).await?;
            log::info!(
                "Node reopen stage=runtime elapsed_ms={}",
                started.elapsed().as_millis()
            );
            self.local = Some(local);
            self.publish(runtime, control).await?;
            log::info!(
                "Node reopen ready elapsed_ms={}",
                started.elapsed().as_millis()
            );
        } else {
            self.local = Some(local);
        }
        Ok(())
    }

    async fn provision_create(
        &mut self,
        command: &SetupCommand,
        request: &CreateRequest,
        control: &HostedSetupControl,
    ) -> Result<HostedSetupReady, StatusCode> {
        self.host
            .select_create()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        self.open_credentials()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let credentials = self.credentials.take().expect("retained credentials guard");
        let administrator = request.administrator.clone();
        let node = request.node.clone();
        let password = request.password.clone();
        let local = self.local.take();
        let prepared = tokio::task::spawn_blocking(move || {
            let result = (|| {
                let config = credentials.load()?;
                let local = match local {
                    Some(local) => local,
                    None if present(&config.local_provisioning_directory())? => {
                        LocalProvisioner::open(config)?
                    }
                    None => LocalProvisioner::create_with_password(
                        config,
                        &administrator,
                        &node,
                        &password,
                    )?,
                };
                local.validate_retry(&administrator, &node, &password)?;
                Ok::<_, io::Error>(local)
            })();
            (credentials, result)
        })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        self.credentials = Some(prepared.0);
        self.local = Some(prepared.1.map_err(|_| StatusCode::CONFLICT)?);
        let local = self.local.as_mut().expect("retained owner journal");
        let runtime = Box::pin(local.prepare_hosted_runtime(
            &request.password,
            self.tls.is_some(),
            &command.cancellation,
        ))
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        if let Err(error) = self.publish(runtime, control).await {
            return Err(if error.kind() == io::ErrorKind::Interrupted {
                StatusCode::CONFLICT
            } else {
                StatusCode::SERVICE_UNAVAILABLE
            });
        }
        if command.cancellation.is_cancelled() {
            return Err(StatusCode::CONFLICT);
        }
        self.local
            .as_ref()
            .expect("verified owner journal")
            .mark_complete()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        self.published()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
    }

    async fn reopen_join(
        &mut self,
        retired: bool,
        intent: JoinIntent,
        control: &HostedSetupControl,
    ) -> io::Result<()> {
        #[cfg(test)]
        eprintln!("Node Join reopen stage=intent");
        #[cfg(test)]
        eprintln!("Node Join reopen stage=history");
        let config = self.host.config();
        let credentials_attempted = join_attempted(self.host.directory(), "credentials")?;
        let owner_attempted = join_attempted(self.host.directory(), "owner")?;
        if owner_attempted && !credentials_attempted
            || retired && !(credentials_attempted && owner_attempted)
        {
            return Err(io::Error::other(
                "retired or conflicting Join production history",
            ));
        }
        if !credentials_attempted {
            require_absent(&config.credential_bootstrap_directory())?;
            require_absent(&config.local_provisioning_directory())?;
            return Ok(());
        }
        if !present(&config.credential_bootstrap_directory())? {
            return Err(io::Error::other(
                "attempted Join is missing retained credentials",
            ));
        }
        let owner_root = config.local_provisioning_directory();
        #[cfg(test)]
        eprintln!("Node Join reopen stage=credentials");
        self.credentials = Some(CredentialBootstrap::open(config)?);
        if !owner_attempted {
            require_absent(&owner_root)?;
            return Ok(());
        }
        let config = self
            .credentials
            .as_ref()
            .expect("retained credential references")
            .load()?;
        if !present(&config.local_provisioning_directory())? {
            return Err(io::Error::other(
                "attempted Join is missing retained owner state",
            ));
        }
        #[cfg(test)]
        eprintln!("Node Join reopen stage=owner");
        let joining = LocalJoin::open(config)?;
        let complete = joining.is_complete()?;
        joining.validate_retry(&intent.peer, &intent.node)?;
        if Some(joining.trusted_peer()) != self.host.trusted_peer() {
            return Err(io::Error::other(
                "retained Join peer conflicts with host selection",
            ));
        }
        self.joining = Some(joining);
        if complete && retired {
            #[cfg(test)]
            eprintln!("Node Join reopen stage=runtime");
            let runtime = self
                .joining
                .as_ref()
                .expect("retained Join")
                .reopen_hosted_runtime(self.tls.is_some())
                .await?;
            #[cfg(test)]
            eprintln!("Node Join reopen stage=publish");
            self.publish(runtime, control).await?;
        } else if retired {
            return Err(io::Error::other(
                "retired admission requires completed Join",
            ));
        }
        Ok(())
    }

    fn open_credentials(&mut self) -> io::Result<()> {
        if self.credentials.is_some()
            && self.host.purpose() == BootstrapPurpose::Join
            && !join_attempted(self.host.directory(), "credentials")?
        {
            return Err(io::Error::other(
                "retained Join is missing credential production history",
            ));
        }
        if self.credentials.is_none() {
            let config = self.host.config();
            self.credentials = Some(if self.host.purpose() == BootstrapPurpose::Join {
                if begin_join_attempt(
                    self.host.directory(),
                    "credentials",
                    &config.credential_bootstrap_directory(),
                )? {
                    match &self.tls {
                        Some(tls) => CredentialBootstrap::adopt_host_tls(config, tls)?,
                        None => CredentialBootstrap::create(config)?,
                    }
                } else {
                    CredentialBootstrap::open(config)?
                }
            } else if present(&config.credential_bootstrap_directory())? {
                CredentialBootstrap::open(config)?
            } else {
                match &self.tls {
                    Some(tls) => CredentialBootstrap::adopt_host_tls(config, tls)?,
                    None => CredentialBootstrap::create(config)?,
                }
            });
        }
        Ok(())
    }

    async fn provision_join(
        &mut self,
        command: &SetupCommand,
        request: &JoinRequest,
        control: &HostedSetupControl,
    ) -> Result<HostedSetupReady, StatusCode> {
        let SetupIntent::Join(intent) = SetupIntent::decode(&command.intent)? else {
            return Err(StatusCode::CONFLICT);
        };
        self.host
            .select_join(&intent.peer, &intent.issuer)
            .map_err(|_| StatusCode::CONFLICT)?;
        self.open_credentials()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let directory = self.host.directory().to_path_buf();
        let credentials = self.credentials.take().expect("retained credential guard");
        let joining = self.joining.take();
        let request = request.clone();
        let prepared = tokio::task::spawn_blocking(move || {
            let result = (|| {
                let config = credentials.load()?;
                let joining = match joining {
                    Some(joining) => {
                        if !join_attempted(&directory, "owner")? {
                            return Err(io::Error::other(
                                "retained Join is missing owner production history",
                            ));
                        }
                        joining
                    }
                    None if begin_join_attempt(
                        &directory,
                        "owner",
                        &config.local_provisioning_directory(),
                    )? =>
                    {
                        LocalJoin::create(config, &request.peer, &request.node)?
                    }
                    None => LocalJoin::open(config)?,
                };
                joining.validate_retry(&request.peer, &request.node)?;
                Ok::<_, io::Error>(joining)
            })();
            (credentials, result)
        })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        self.credentials = Some(prepared.0);
        self.joining = Some(prepared.1.map_err(|_| StatusCode::CONFLICT)?);
        let joining = self.joining.as_mut().expect("retained Join");
        let runtime = provisioning_stage(|| {
            joining.prepare_hosted_runtime(self.tls.is_some(), &command.cancellation)
        })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        #[cfg(test)]
        let publish_started = std::time::Instant::now();
        self.publish(runtime, control)
            .await
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        #[cfg(test)]
        eprintln!(
            "Node Join stage=publish elapsed_ms={}",
            publish_started.elapsed().as_millis()
        );
        #[cfg(test)]
        let completion_started = std::time::Instant::now();
        self.joining
            .as_mut()
            .expect("verified Join")
            .complete_hosted(
                self.runtime.as_ref().expect("published runtime"),
                &command.cancellation,
            )
            .await
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        #[cfg(test)]
        eprintln!(
            "Node Join stage=completion elapsed_ms={}",
            completion_started.elapsed().as_millis()
        );
        self.published()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
    }

    async fn publish(
        &mut self,
        mut runtime: UnifiedRuntime,
        control: &HostedSetupControl,
    ) -> io::Result<()> {
        #[cfg(test)]
        runtime.trace_bootstrap_http_stages();
        if let Err(error) = runtime.start_background_tasks().await {
            runtime.shutdown().await?;
            return Err(error);
        }
        let (protocol, owner) = runtime.readiness();
        control
            .serve(
                runtime.router(),
                Arc::new(move || {
                    let protocol = protocol.clone();
                    let owner = owner.clone();
                    Box::pin(async move { protocol.local_node_public_ready(owner.as_ref()).await })
                }),
            )
            .await;
        self.runtime = Some(runtime);
        Ok(())
    }

    async fn unpublish(&mut self) -> io::Result<()> {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown().await?;
        }
        Ok(())
    }

    fn login_client(&self) -> io::Result<LoginClient> {
        let login = match self.joining.as_ref() {
            Some(joining) => joining.login_client(),
            None => self
                .local
                .as_ref()
                .expect("ready owner journal")
                .login_client()?,
        };
        Ok(LoginClient {
            client_id: login
                .client_id
                .ok_or_else(|| io::Error::other("missing public login Client"))?,
            scopes: login.allowed_scopes,
        })
    }
    fn published(&self) -> io::Result<HostedSetupReady> {
        let runtime = self
            .runtime
            .as_ref()
            .ok_or_else(|| io::Error::other("owner runtime not published"))?;
        let (protocol, owner) = runtime.readiness();
        Ok(HostedSetupReady {
            router: runtime.router(),
            login_client: self.login_client()?,
            readiness: Arc::new(move || {
                let protocol = protocol.clone();
                let owner = owner.clone();
                Box::pin(async move { protocol.local_node_public_ready(owner.as_ref()).await })
            }),
        })
    }
}

impl HostedSetupOwners for RuntimeSetupOwners {
    fn settings(&self) -> HostedSetupSettings {
        let config = self.host.config();
        HostedSetupSettings {
            directory: self.host.directory().to_owned(),
            ui_origin: config.idp.ui_public_uri.clone(),
            https: self.tls.is_some(),
            authentication_base: format!(
                "{}://{}/idp",
                if self.tls.is_some() { "https" } else { "http" },
                config.listen_addr
            ),
            contracts: vec![
                (
                    "/idp".into(),
                    idp_server::openapi_document(&config.idp.api_public_uri, ""),
                ),
                (
                    "/management".into(),
                    management_server::openapi_document(&config.management.api_public_uri, ""),
                ),
                (
                    "/storage".into(),
                    storage_server::openapi_document(&config.storage.api_public_uri, "/storage"),
                ),
            ],
        }
    }
    fn purpose(&self) -> SetupPurpose {
        match self.host.purpose() {
            BootstrapPurpose::AwaitingChoice => SetupPurpose::AwaitingChoice,
            BootstrapPurpose::Create => SetupPurpose::Create,
            BootstrapPurpose::Join => SetupPurpose::Join,
        }
    }
    fn trusted_peer(&self) -> Option<&str> {
        self.host.trusted_peer()
    }
    fn issuer(&self) -> String {
        self.host.config().idp.oauth2.issuer
    }
    fn service_router(&self, control: HostedSetupControl) -> Router {
        let config = self.host.config();
        crate::compose_router(
            idp_server::build_setup_router(control, &config.idp.api_public_uri, "/idp"),
            management_server::public_metadata_router(
                &config.management.api_public_uri,
                "/management",
            ),
            storage_server::public_metadata_router(&config.storage.api_public_uri, "/storage"),
        )
    }
    fn verification_client(&self) -> io::Result<reqwest::Client> {
        LocalTls::verification_client()
    }
    async fn discover(&self, peer: &str, cancellation: &CancellationToken) -> io::Result<String> {
        crate::issuer_discovery::discover_with_cancellation(peer, cancellation).await
    }
    async fn reopen(
        &mut self,
        retired: bool,
        intent: Option<JoinIntent>,
        control: HostedSetupControl,
    ) -> io::Result<Option<HostedSetupReady>> {
        if let Some(intent) = intent {
            self.reopen_join(retired, intent, &control).await?;
        } else {
            self.reopen_create(retired, &control).await?;
        }
        if self.runtime.is_some() {
            self.published().map(Some)
        } else {
            Ok(None)
        }
    }
    async fn provision(
        &mut self,
        command: &SetupCommand,
        control: HostedSetupControl,
    ) -> Result<HostedSetupReady, StatusCode> {
        match &command.request {
            SetupOperation::Create(request) => {
                Box::pin(self.provision_create(command, request, &control)).await
            }
            SetupOperation::Join(request) => {
                Box::pin(self.provision_join(command, request, &control)).await
            }
        }
    }
    async fn shutdown(&mut self) -> io::Result<()> {
        self.unpublish().await
    }
}
