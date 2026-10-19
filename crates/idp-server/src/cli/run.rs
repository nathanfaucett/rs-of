use std::{
    io,
    net::{IpAddr, SocketAddr},
    path::Path,
    sync::Arc,
    time::Duration,
};

use api::serve;
#[cfg(feature = "completions")]
use clap::CommandFactory;
use clap::Parser;
#[cfg(feature = "completions")]
use cli::Shell;
use cli::{CliArgs, shutdown_signal};
use db::Database;
use env_logger::Env;
use iroh_chain::EndpointIdStore;

use tokio::{select, spawn, time::sleep};
use tokio_util::sync::CancellationToken;
use tower_http::{compression::CompressionLayer, cors::CorsLayer, trace::TraceLayer};

use crate::{AppConfig, build_runtime};

#[derive(clap::Parser, Debug)]
enum IdpCommand {
    Serve {
        #[arg(long, short = 'p')]
        port: Option<u16>,
        #[arg(long, short = 'H')]
        host: Option<IpAddr>,
    },
    #[cfg(feature = "completions")]
    Completions { shell: Shell },
    #[command(
        about = "Provision an OAuth service client locally. Stop IdP before running; this command does not enforce shutdown."
    )]
    ProvisionServiceClient {
        #[arg(long)]
        application_uri: String,
        #[arg(long)]
        client_name: String,
        #[arg(long = "audience", required = true)]
        audiences: Vec<String>,
        #[arg(long = "scope", required = true)]
        scopes: Vec<String>,
        #[arg(long)]
        credentials_file: std::path::PathBuf,
    },
}

pub async fn run() -> io::Result<()> {
    let args = CliArgs::<IdpCommand>::parse();
    if let Some(IdpCommand::ProvisionServiceClient {
        application_uri,
        client_name,
        audiences,
        scopes,
        credentials_file,
    }) = args.command.as_ref()
    {
        let _ = dotenvy::dotenv();
        let app_config = AppConfig::try_from(Path::new(&args.config))
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
        return crate::cli::provision_service_client(
            &app_config,
            application_uri,
            client_name,
            audiences.clone(),
            scopes.clone(),
            credentials_file,
        )
        .await;
    }

    match dotenvy::dotenv() {
        Ok(_) => {}
        Err(error) => eprintln!("failed to load .env file: {error}"),
    }

    let cancellation_token = CancellationToken::new();
    #[cfg(unix)]
    let app_config = Arc::new(crate::startup::load_config(Path::new(&args.config))?);
    #[cfg(not(unix))]
    let app_config = Arc::new(match AppConfig::try_from(Path::new(&args.config)) {
        Ok(app_config) => app_config,
        Err(error) => {
            eprintln!("failed to load config {:?}: {error}", args.config);
            AppConfig::default()
        }
    });
    env_logger::Builder::from_env(Env::default().default_filter_or(&app_config.log_level)).init();

    #[cfg(unix)]
    if matches!(args.command, None | Some(IdpCommand::Serve { .. })) {
        let (host, port) = match args.command.as_ref() {
            Some(IdpCommand::Serve { host, port }) => (*host, *port),
            _ => (None, None),
        };
        let configured_host = app_config.server.host;
        let setup_host = host.unwrap_or_else(|| {
            if configured_host.is_unspecified() {
                std::net::Ipv4Addr::LOCALHOST.into()
            } else {
                configured_host
            }
        });
        let address = SocketAddr::new(setup_host, port.unwrap_or(app_config.server.port));
        if let Some(setup) = crate::prepare_setup(&app_config, address)? {
            let shutdown = shutdown_signal(cancellation_token.clone());
            let serving = setup.serve(cancellation_token.clone());
            tokio::pin!(shutdown, serving);
            return tokio::select! {
                result = &mut serving => result,
                () = &mut shutdown => serving.await,
            };
        }
    }

    std::fs::create_dir_all(&app_config.data_dir)?;
    let engine = Arc::new(
        Database::open(Path::new(&app_config.data_dir).join("idp.redb"))
            .map_err(io::Error::other)?,
    );
    idp_model::replica::up(&engine)
        .await
        .map_err(io::Error::other)?;
    let allowed_peers = EndpointIdStore::default();
    let (device_identity, server) =
        management_service::open_device_identity_with_allowlist(allowed_peers.clone()).await?;
    let runtime = Arc::new(
        build_runtime(
            &app_config,
            engine,
            None,
            Arc::new(device_identity),
            server.clone(),
            None,
        )
        .await?,
    );
    let _iroh_router = server.router_with_protocol(
        crate::unavailable_data_protocol::UnavailableDataProtocol,
        crate::bootstrap::BOOTSTRAP_ALPN,
        runtime.bootstrap_protocol(),
    );
    log::info!("Iroh endpoint: {:?}", server.endpoint().addr());

    let router = runtime
        .router()
        .layer(CorsLayer::very_permissive().allow_private_network(true))
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new().gzip(app_config.server.gzip));
    let run_serve = |host: Option<IpAddr>, port: Option<u16>| {
        let addr = SocketAddr::from((
            host.unwrap_or(app_config.server.host),
            port.unwrap_or(app_config.server.port),
        ));
        spawn(serve(router, addr, cancellation_token.clone()))
    };
    let command_handle = match args.command {
        #[cfg(feature = "completions")]
        Some(IdpCommand::Completions { shell }) => spawn(async move {
            clap_complete::generate(
                shell,
                &mut IdpCommand::command(),
                env!("CARGO_PKG_NAME"),
                &mut std::io::stdout(),
            );
            Ok(())
        }),
        Some(IdpCommand::Serve { host, port }) => run_serve(host, port),
        Some(IdpCommand::ProvisionServiceClient { .. }) => {
            return Err(io::Error::other(
                "service-client provisioning did not complete before server startup",
            ));
        }
        None => run_serve(None, None),
    };

    shutdown_signal(cancellation_token).await;

    let mut command_handle = command_handle;
    select! {
        result = &mut command_handle => match result {
            Ok(Ok(())) => log::info!("server shutdown complete"),
            Ok(Err(error)) => log::error!("command error: {error}"),
            Err(error) => log::error!("join error: {error}"),
        },
        _ = sleep(Duration::from_secs(10)) => {
            log::warn!("server shutdown timed out, aborting serve task");
            command_handle.abort();
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use cli::CliArgs;

    #[test]
    fn independent_startup_accepts_no_config_and_explicit_serve() {
        CliArgs::<super::IdpCommand>::try_parse_from(["idp-server"]).expect("default launch");
        CliArgs::<super::IdpCommand>::try_parse_from(["idp-server", "serve", "--port", "3000"])
            .expect("explicit serving launch");
    }
}
