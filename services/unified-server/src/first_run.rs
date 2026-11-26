use std::{
    fs::{File, read_dir},
    io,
    net::Ipv4Addr,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use crate::{
    node_create::NodeCreate,
    node_runtime::NodeRuntime,
    node_setup::{ADMISSION, Choice, NodeSetup},
    setup_store::SetupStore,
};
use axum::{
    Router,
    extract::Request,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
};
use db::EmbeddedDatabase;
use reqwest::{Client, Url};
use server_common::{LocalTls, random_capability, setup_files};
use setup::join::JoinHost;
use tokio::{
    net::TcpListener,
    sync::watch,
    task::{JoinError, JoinSet},
    time::{sleep, timeout},
};
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;

const ROOT_MARKER: &str = "unified-setup.lock";
const DATABASE: &str = "unified.redb";
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
        let result = async {
            let database = EmbeddedDatabase::open(&root).await.map_err(io::Error::other)?;
            let store = SetupStore::open(&database).await?;
            let node = NodeSetup::open(&root, &ui_origin, &pm_origin, &public_origin, store.clone(), shutdown.clone())?;
            let mut selection = node.choice();
            published.send_replace(Some(node.router(Router::new())));
            let choice = tokio::select! {
                () = shutdown.cancelled() => return Ok(()),
                result = jobs.join_next() => return flatten(result),
                value = selection.wait_for(Option::is_some) => value.map_err(io::Error::other)?.expect("retained choice"),
            };
            match choice {
                Choice::Create => {
                    let (create, commands) = NodeCreate::open(node.clone(), store).await?;
                    published.send_replace(Some(node.router(create.router()).merge(create.owner_routes())));
                    jobs.spawn(create.run(NodeRuntime {
                        root: root.clone(), base: public_origin, ui: ui_origin, pm: pm_origin, client, database,
                    }, commands, shutdown.clone()));
                }
                Choice::Join => {
                    let join = Arc::new(JoinHost::open_for_proxy_with_journal(&ui_origin, node.admission(), Arc::new(store))?);
                    published.send_replace(Some(node.router(Router::new()).merge(join.router())));
                    node.joined(join);
                }
            }
            tokio::select! {
                () = shutdown.cancelled() => Ok(()),
                result = jobs.join_next() => flatten(result),
            }
        }.await;
        shutdown.cancel();
        drain(&mut jobs, result).await
    }.await;
    shutdown.cancel();
    drain(&mut jobs, result).await
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
            || ![ROOT_MARKER, ADMISSION, DATABASE, "sync-staging"]
                .iter()
                .any(|allowed| name == *allowed)
        {
            return Err(io::Error::other(
                "unsupported retained state; preserve it and choose a fresh setup data root",
            ));
        }
    }
    if setup_files::present(&root.join(ADMISSION))? && !setup_files::present(&root.join(DATABASE))?
    {
        return Err(io::Error::other("retained Node database is missing"));
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
    let lock = match setup_files::private_options(true).open(&marker) {
        Ok(lock) => lock,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            setup_files::require_private(&marker, false)?;
            setup_files::private_options(false).open(&marker)?
        }
        Err(error) => return Err(error),
    };
    lock.try_lock().map_err(io::Error::other)?;
    validate_root(root)?;
    if !retained {
        lock.sync_all()?;
        setup_files::sync_ancestry(root)?;
    }
    Ok(lock)
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
