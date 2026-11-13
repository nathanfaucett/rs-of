use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use server_common::{LocalTls, LocalTlsConfig};
use setup::{IndependentCreateCommand, IndependentCreateHost, IndependentSetupHost};
use axum::{Router, extract::Request, http::StatusCode};
use tokio::{sync::mpsc, task::JoinSet};
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;

use crate::{UnifiedConfig, compose_router};

pub struct SetupHost {
    config: UnifiedConfig,
    tls: LocalTls,
    hosts: [IndependentCreateHost; 3],
    commands: [mpsc::Receiver<IndependentCreateCommand>; 3],
    shutdown: CancellationToken,
    management: Arc<std::sync::RwLock<Option<Router>>>,
}

impl SetupHost {
    pub fn prepare(config: UnifiedConfig) -> io::Result<Self> {
        reject_previous_layout(&config)?;
        LocalTls::api_base(config.listen_addr)?;
        let roots = [
            &config.idp.data_dir,
            &config.device.data_dir,
            &config.storage.data_dir,
        ];
        let mut paths = roots
            .iter()
            .map(|root| idp_server::local_provisioning::absolute(Path::new(root)))
            .collect::<io::Result<Vec<_>>>()?;
        paths.push(idp_server::local_provisioning::absolute(
            &config.tls_directory(),
        )?);
        for (i, path) in paths.iter().enumerate() {
            if paths[..i]
                .iter()
                .any(|other| path.starts_with(other) || other.starts_with(path))
            {
                return Err(io::Error::other("owner and TLS roots must not overlap"));
            }
        }
        for (owner, prefix) in [
            ("idp", config.idp.server.prefix()),
            ("device", config.device.server.prefix()),
            ("storage", config.storage.server.prefix()),
            ("management", config.management.server.prefix()),
        ] {
            if prefix != format!("/{owner}") {
                return Err(io::Error::other(
                    "composed owners require their exact owner prefix",
                ));
            }
        }
        let retained = server_common::setup_files::present(&config.tls_directory())?;
        if !retained
            && paths[..3].iter().any(|path| {
                server_common::setup_files::present(path).unwrap_or(true)
                    || server_common::setup_files::present(&path.with_extension("setup-host")).unwrap_or(true)
            })
        {
            return Err(io::Error::other(
                "retained owner state has no local authority; restore exact state or explicitly reset",
            ));
        }
        let tls = if retained {
            LocalTls::open(&config)?
        } else {
            LocalTls::create(&config)?
        };
        let shutdown = CancellationToken::new();
        let mut prepared = Vec::new();
        for (owner, root) in ["idp", "device", "storage"].into_iter().zip(roots) {
            let host = IndependentSetupHost::prepare_with_tls(
                owner,
                Path::new(root),
                config.listen_addr,
                &config.idp.ui_public_uri,
                &format!("/{owner}"),
                tls.clone(),
            )?;
            prepared.push(host.into_create(shutdown.clone())?);
        }
        let [
            (idp, idp_commands),
            (device, device_commands),
            (storage, storage_commands),
        ] = prepared
            .try_into()
            .map_err(|_| io::Error::other("three setup owners required"))?;
        Ok(Self {
            config,
            tls,
            hosts: [idp, device, storage],
            commands: [idp_commands, device_commands, storage_commands],
            shutdown,
            management: Arc::new(std::sync::RwLock::new(None)),
        })
    }

    pub fn tls(&self) -> &LocalTls {
        &self.tls
    }
    pub fn local_addr(&self) -> std::net::SocketAddr {
        self.config.listen_addr
    }
    pub fn admission_path(&self, owner: &str) -> io::Result<PathBuf> {
        self.hosts
            .iter()
            .find(|host| host.control().owner() == owner)
            .map(|host| host.control().directory().join("setup-admission/state"))
            .ok_or_else(|| io::Error::other("unknown setup owner"))
    }
    pub fn router(&self) -> Router {
        let management = self.management.clone();
        let metadata = management_server::public_metadata_router(
            &self.config.management.api_public_uri,
            self.config.management.server.prefix(),
        )
        .fallback(|| async { StatusCode::SERVICE_UNAVAILABLE });
        let management = Router::new().fallback(move |request: Request| {
            let router = management
                .read()
                .ok()
                .and_then(|r| r.clone())
                .unwrap_or_else(|| metadata.clone());
            async move {
                router
                    .oneshot(request)
                    .await
                    .expect("infallible Management router")
            }
        });
        compose_router(
            self.hosts[0].router(),
            management,
            self.hosts[1].router(),
            self.hosts[2].router(),
        )
    }
    pub async fn run(self, cancellation: CancellationToken) -> io::Result<()> {
        let Self {
            config,
            tls: _,
            hosts,
            commands: [idp_commands, device_commands, storage_commands],
            shutdown,
            management,
        } = self;
        let _cancel = shutdown.clone().drop_guard();
        let mut jobs = JoinSet::new();
        jobs.spawn(idp_server::run_owner_setup(
            Arc::new(config.idp),
            hosts[0].control(),
            idp_commands,
            shutdown.clone(),
        ));
        let (serving, mut receiver) = tokio::sync::watch::channel(None);
        jobs.spawn(device_server::run_owner_setup(
            Arc::new(config.device),
            hosts[1].control(),
            device_commands,
            shutdown.clone(),
            Some(serving),
        ));
        let stop = shutdown.clone();
        jobs.spawn(async move {
            let (identity, control) = loop {
                if let Some(serving) = receiver.borrow_and_update().clone() {
                    break serving;
                }
                tokio::select! {
                    () = stop.cancelled() => return Ok(()),
                    changed = receiver.changed() => changed.map_err(io::Error::other)?,
                }
            };
            let router = management_server::build_router(
                identity,
                &config.management.api_public_uri,
                config.management.server.prefix(),
                control,
            )
            .await
            .map_err(io::Error::other)?;
            *management
                .write()
                .map_err(|_| io::Error::other("Management router lock poisoned"))? = Some(router);
            stop.cancelled().await;
            Ok(())
        });
        jobs.spawn(storage_server::run_owner_setup(
            Arc::new(config.storage),
            hosts[2].control(),
            storage_commands,
            shutdown.clone(),
        ));
        let mut result = tokio::select! {
            () = cancellation.cancelled() => Ok(()),
            result = jobs.join_next() => result.expect("owner workers").map_err(io::Error::other).and_then(|r| r),
        };
        shutdown.cancel();
        while let Some(job) = jobs.join_next().await {
            result = result.and(job.map_err(io::Error::other).and_then(|r| r));
        }
        drop(hosts);
        result
    }
    pub async fn serve(self, cancellation: CancellationToken) -> io::Result<()> {
        let listener = tokio::net::TcpListener::bind(self.local_addr()).await?;
        let router = self.router();
        let listener = self.tls.listener(listener);
        for owner in ["idp", "device", "storage"] {
            println!(
                "Protected {owner} setup code: {}",
                self.admission_path(owner)?.display()
            );
        }
        println!("Local setup authority: {}", self.tls.ca_path().display());
        let stop = cancellation.clone();
        let serving = async move {
            axum::serve(listener, router.into_make_service())
                .with_graceful_shutdown(stop.cancelled_owned())
                .await
        };
        let worker = self.run(cancellation.clone());
        tokio::pin!(serving, worker);
        let result = tokio::select! {
            result = &mut serving => { cancellation.cancel(); result.and(worker.await) },
            result = &mut worker => { cancellation.cancel(); result.and(serving.await) },
        };
        result
    }
}

fn reject_previous_layout(config: &UnifiedConfig) -> io::Result<()> {
    let endpoint = &config.endpoint_data_dir;
    let parent = endpoint
        .parent()
        .ok_or_else(|| io::Error::other("endpoint root has no parent"))?;
    if let Some(data) = parent.parent()
        && server_common::setup_files::present(&data.join("reset-state.json"))?
    {
        return Err(io::Error::other(
            "native reset is pending; finish that exact reset before starting owner workers",
        ));
    }
    for path in [
        parent.join("host.redb"),
        parent.join("host.json"),
        parent.join("credentials.json"),
        parent.join("setup-admission"),
        endpoint.with_extension("host"),
        endpoint.with_extension("provisioning"),
        endpoint.with_extension("credentials"),
    ] {
        if server_common::setup_files::present(&path)? {
            return Err(io::Error::other(
                "unsupported previous Unified Host installation; preserve this root and restore the previous binary for recovery/reset, or explicitly choose a new empty data directory; automatic migration is not supported",
            ));
        }
    }
    Ok(())
}
