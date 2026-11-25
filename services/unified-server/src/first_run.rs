use std::{
    fs::{File, read_dir},
    io,
    net::Ipv4Addr,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use axum::{
    Router,
    extract::Request,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::get,
};
use reqwest::{Client, Url};
use server_common::{
    IndependentCreateHost, IndependentSetupHost, LocalTls, ServerConfig, random_capability,
    setup_files,
};
use setup::join::JoinHost;
use tokio::{
    net::TcpListener,
    sync::watch,
    task::{JoinError, JoinSet},
    time::{sleep, timeout},
};
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;
use tower_http::cors::{Any, CorsLayer};

use crate::{
    compose_router,
    node_setup::{ADMISSION, Choice, Completion, NodeSetup, OWNERS},
};

const ROOT_MARKER: &str = "unified-setup.lock";
const HOSTS_MARKER: &str = "unified-setup-hosts";
const PROBE_PATH: &str = "/.well-known/unified-setup-probe";

pub async fn serve(
    root: PathBuf,
    ui_origin: String,
    public_origin: String,
    pm_origin: String,
    port: u16,
    shutdown: CancellationToken,
) -> io::Result<()> {
    let url = Url::parse(&public_origin).map_err(io::Error::other)?;
    server_common::setup_origin(&public_origin)?;
    server_common::setup_origin(&ui_origin)?;
    server_common::setup_origin(&pm_origin)?;
    if url.scheme() != "https"
        || !url
            .host_str()
            .is_some_and(|host| host.ends_with(".localhost"))
        || !ui_origin.starts_with("https://")
        || !pm_origin.starts_with("https://")
    {
        return Err(io::Error::other("setup requires local Portless HTTPS"));
    }
    let root = setup_files::absolute(&root)?;
    validate_root(&root)?;
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await?;
    let client = LocalTls::verification_client()?;
    let proof = random_capability()?;
    let (published, router) = watch::channel::<Option<Router>>(None);
    let probe = proof.clone();
    let probing = router.clone();
    let listening = Router::new()
        .route(
            PROBE_PATH,
            get(move |headers: HeaderMap| async move {
                if probing.borrow().is_none()
                    && headers
                        .get("x-unified-setup-probe")
                        .and_then(|v| v.to_str().ok())
                        == Some(&probe)
                {
                    StatusCode::NO_CONTENT
                } else {
                    StatusCode::NOT_FOUND
                }
            }),
        )
        .fallback(move |request: Request| {
            let router = router.borrow().clone();
            async move {
                match router {
                    Some(router) => router
                        .oneshot(request)
                        .await
                        .expect("infallible unified router"),
                    None => StatusCode::SERVICE_UNAVAILABLE.into_response(),
                }
            }
        });
    let mut jobs = JoinSet::new();
    let stop = shutdown.clone();
    jobs.spawn(async move {
        axum::serve(listener, listening)
            .with_graceful_shutdown(stop.cancelled_owned())
            .await
    });
    let verified = tokio::select! {
        result = verify_proxy(&client, &public_origin, &proof) => result,
        () = shutdown.cancelled() => Err(io::Error::new(io::ErrorKind::Interrupted, "setup stopped")),
    };
    let result = async {
        verified?;
        let _root_lock = open_root(&root)?;
        let mut owners = None;
        let result = async {
            let node = NodeSetup::open(&root, &ui_origin, &pm_origin, &public_origin)?;
            let mut selection = node.choice();
            published.send_replace(Some(node.router(Completion::Starting)));
            let choice = tokio::select! {
                () = shutdown.cancelled() => return Ok(()),
                result = jobs.join_next() => return flatten(result),
                value = selection.wait_for(Option::is_some) => value.map_err(io::Error::other)?.expect("retained choice"),
            };
            match choice {
                Choice::Create => owners = Some(run_owners(
                    &root, (&ui_origin, &public_origin, &pm_origin), &client,
                    &node, &published, &shutdown, &mut jobs,
                )?),
                Choice::Join => {
                    let path = root.join("join");
                    let join = Arc::new(JoinHost::open_for_proxy(&path, &ui_origin)?);
                    println!("Protected Join setup-code retrieval path: {}", path.join("admission/state").display());
                    published.send_replace(Some(join.router().merge(node.router(Completion::Join(join)))));
                }
            }
            tokio::select! {
                () = shutdown.cancelled() => Ok(()),
                result = jobs.join_next() => flatten(result),
            }
        }.await;
        shutdown.cancel();
        let result = drain(&mut jobs, result).await;
        drop(owners);
        result
    }.await;
    shutdown.cancel();
    drain(&mut jobs, result).await
}

fn run_owners(
    root: &Path,
    (ui_origin, public_origin, pm_origin): (&str, &str, &str),
    client: &Client,
    node: &NodeSetup,
    published: &watch::Sender<Option<Router>>,
    shutdown: &CancellationToken,
    jobs: &mut JoinSet<io::Result<()>>,
) -> io::Result<[IndependentCreateHost; 3]> {
    let [idp, device, storage] = OWNERS.map(|owner| {
        IndependentSetupHost::prepare_for_proxy(
            owner,
            &root.join(owner),
            ui_origin,
            public_origin,
            client.clone(),
        )
    });
    let (idp, device, storage) = (idp?, device?, storage?);
    let paths = [
        idp.admission_path(),
        device.admission_path(),
        storage.admission_path(),
    ];
    let (idp, idp_commands) = idp.into_create(shutdown.clone())?;
    let (device, device_commands) = device.into_create(shutdown.clone())?;
    let (storage, storage_commands) = storage.into_create(shutdown.clone())?;
    write_hosts_marker(root)?;
    let (idp_config, device_config, storage_config) =
        owner_configs(root, ui_origin, public_origin)?;
    let (device_ready, mut device_identity) = watch::channel(None);
    let (management_ready, management_router) = watch::channel::<Option<Router>>(None);
    let management_base = format!("{public_origin}/management");
    let management = management_routes(
        &management_base,
        server_common::setup_origin(ui_origin)?,
        management_router.clone(),
    );
    jobs.spawn(idp_server::run_owner_setup(
        Arc::new(idp_config),
        idp.control(),
        idp_commands,
        shutdown.clone(),
    ));
    jobs.spawn(device_server::run_owner_setup(
        Arc::new(device_config),
        device.control(),
        device_commands,
        shutdown.clone(),
        Some(device_ready),
    ));
    jobs.spawn(storage_server::run_owner_setup(
        Arc::new(storage_config),
        storage.control(),
        storage_commands,
        shutdown.clone(),
    ));
    let stop = shutdown.clone();
    jobs.spawn(async move {
        let serving = tokio::select! {
            () = stop.cancelled() => return Ok(()),
            value = device_identity.wait_for(Option::is_some) => value.map_err(io::Error::other)?.clone(),
        };
        let (identity, control) = serving.expect("Device published its serving state");
        let router = management_server::build_router(identity, &management_base, "/management", control).await.map_err(io::Error::other)?;
        management_ready.send_replace(Some(router));
        stop.cancelled().await;
        Ok(())
    });
    let api_origins = [
        server_common::setup_origin(ui_origin)?,
        server_common::setup_origin(pm_origin)?,
    ];
    let completion = Completion::Create {
        owners: [idp.control(), device.control(), storage.control()],
        management: management_router,
    };
    published.send_replace(Some(
        compose_router(
            idp.router_with_api_origins(api_origins.clone()),
            management,
            device.router(),
            storage.router_with_api_origins(api_origins),
        )
        .merge(node.router(completion)),
    ));
    for (owner, path) in OWNERS.into_iter().zip(paths) {
        println!(
            "Protected {owner} setup-code retrieval path: {}",
            path.display()
        );
    }
    Ok([idp, device, storage])
}

fn owner_configs(
    root: &Path,
    ui_origin: &str,
    public_origin: &str,
) -> io::Result<(
    idp_server::AppConfig,
    device_server::AppConfig,
    storage_server::AppConfig,
)> {
    let config = |owner: &str| -> io::Result<(String, ServerConfig, String)> {
        let path = root.join(owner);
        let data_dir = path
            .to_str()
            .ok_or_else(|| io::Error::other("setup data path must be UTF-8"))?;
        let prefix = format!("/{owner}");
        let base = format!("{public_origin}{prefix}");
        let server = ServerConfig {
            prefix: Some(prefix),
            setup_ui_origin: Some(ui_origin.to_owned()),
            ..Default::default()
        };
        Ok((data_dir.into(), server, base))
    };
    let (data_dir, server, api_public_uri) = config("idp")?;
    let mut idp_config = idp_server::AppConfig {
        data_dir,
        server,
        api_public_uri,
        ui_public_uri: ui_origin.to_owned(),
        ..Default::default()
    };
    idp_config
        .oauth2
        .issuer
        .clone_from(&idp_config.api_public_uri);
    let (data_dir, server, api_public_uri) = config("device")?;
    let device_config = device_server::AppConfig {
        data_dir,
        server,
        api_public_uri,
        ..Default::default()
    };
    let (data_dir, server, api_public_uri) = config("storage")?;
    let storage_config = storage_server::AppConfig {
        data_dir,
        server,
        api_public_uri,
        ..Default::default()
    };
    Ok((idp_config, device_config, storage_config))
}

fn management_routes(
    management_base: &str,
    ui_origin: HeaderValue,
    management_router: watch::Receiver<Option<Router>>,
) -> Router {
    let metadata = management_server::public_metadata_router(management_base, "/management")
        .fallback(|| async { StatusCode::SERVICE_UNAVAILABLE });
    Router::new()
        .fallback(move |request: Request| {
            let router = management_router
                .borrow()
                .clone()
                .unwrap_or_else(|| metadata.clone());
            async move {
                if request.uri().path().starts_with("/management/setup/") {
                    return StatusCode::NOT_FOUND.into_response();
                }
                router
                    .oneshot(request)
                    .await
                    .expect("infallible Management router")
            }
        })
        .layer(
            CorsLayer::new()
                .allow_origin(ui_origin)
                .allow_methods(Any)
                .allow_headers(Any)
                .allow_private_network(true),
        )
}

async fn verify_proxy(client: &Client, origin: &str, proof: &str) -> io::Result<()> {
    let request = format!("{origin}{PROBE_PATH}");
    timeout(Duration::from_secs(10), async {
        loop {
            if client
                .get(&request)
                .header("x-unified-setup-probe", proof)
                .send()
                .await
                .is_ok_and(|response| response.status() == StatusCode::NO_CONTENT)
            {
                break;
            }
            sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .map_err(|_elapsed| {
        io::Error::other(
            "cannot verify the local Portless HTTPS route; check Portless and CA trust",
        )
    })
}

fn validate_root(root: &Path) -> io::Result<()> {
    if !setup_files::present(root)? {
        return Ok(());
    }
    let retained = setup_files::present(&root.join(ROOT_MARKER))?;
    for entry in read_dir(root)? {
        let name = entry?.file_name();
        if !retained
            || ![
                ROOT_MARKER,
                HOSTS_MARKER,
                ADMISSION,
                "join",
                "idp",
                "device",
                "storage",
                "idp.setup-host",
                "device.setup-host",
                "storage.setup-host",
            ]
            .iter()
            .any(|allowed| name == *allowed)
        {
            return Err(io::Error::other(
                "unsupported retained state; preserve it and choose a fresh setup data root",
            ));
        }
    }
    if setup_files::present(&root.join(HOSTS_MARKER))? {
        setup_files::require_private(&root.join(HOSTS_MARKER), false)?;
        for owner in OWNERS {
            setup_files::require_private(&root.join(owner).with_extension("setup-host"), true)?;
        }
    }
    Ok(())
}

fn open_root(root: &Path) -> io::Result<File> {
    setup_files::create_private_directory(root, true)?;
    setup_files::require_private(root, true)?;
    let marker = root.join(ROOT_MARKER);
    let retained = setup_files::present(&marker)?;
    if retained {
        setup_files::require_private(&marker, false)?;
    }
    let lock = setup_files::private_options(!retained).open(marker)?;
    lock.try_lock().map_err(io::Error::other)?;
    validate_root(root)?;
    if !retained {
        lock.sync_all()?;
        setup_files::sync_ancestry(root)?;
    }
    Ok(lock)
}

fn write_hosts_marker(root: &Path) -> io::Result<()> {
    let marker = root.join(HOSTS_MARKER);
    if !setup_files::present(&marker)? {
        setup_files::private_options(true)
            .open(marker)?
            .sync_all()?;
        setup_files::sync_ancestry(root)?;
    }
    Ok(())
}

fn flatten(result: Option<Result<io::Result<()>, JoinError>>) -> io::Result<()> {
    result
        .expect("running setup tasks")
        .map_err(io::Error::other)?
}

async fn drain(jobs: &mut JoinSet<io::Result<()>>, mut result: io::Result<()>) -> io::Result<()> {
    while let Some(job) = jobs.join_next().await {
        result = result.and(flatten(Some(job)));
    }
    result
}
