use std::io;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

#[cfg(feature = "completions")]
use clap::CommandFactory;
use clap::Parser;
#[cfg(feature = "completions")]
use clap_complete::Shell;

use cli::{args::CliArgs, shutdown_signal::shutdown_signal};
use server_common::{
    portless::{bind_addr, public_base_url},
    serve::serve,
};

use env_logger::Env;

use tokio::select;
use tokio::spawn;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::{config::app_config::AppConfig, runtime::build_oauth_runtime};
#[cfg(all(unix, feature = "independent"))]
use crate::{independent_create::serve_independent_create, independent_reset::reset_independent};

#[derive(Parser, Debug)]
enum IdpCommand {
    #[cfg(unix)]
    #[command(
        about = "LOCAL destructive reset of the selected independent IdP config. Stop its host first. Deletes local data and exact protected secrets; requires explicit OS CA trust removal with native consent. Does not revoke remote membership or reset the app."
    )]
    ResetIndependent {
        #[arg(
            long,
            required = true,
            help = "Explicitly remove this host's OS CA trust; retain evidence on denial/failure"
        )]
        remove_os_trust: bool,
    },
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
        credentials_file: PathBuf,
    },
}

pub async fn run() -> io::Result<()> {
    let args = CliArgs::<IdpCommand>::parse();
    #[cfg(all(unix, feature = "independent"))]
    if let Some(IdpCommand::ResetIndependent { remove_os_trust }) = args.command.as_ref() {
        let _ = dotenvy::dotenv();
        let config = crate::startup::load_config(Path::new(&args.config))?;
        return reset_independent(&config, *remove_os_trust, || Ok(())).await;
    }
    #[cfg(not(feature = "independent"))]
    if let Some(IdpCommand::ResetIndependent { .. }) = args.command.as_ref() {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "independent reset requires the 'independent' feature",
        ));
    }
    if let Some(IdpCommand::ProvisionServiceClient {
        application_uri,
        client_name,
        audiences,
        scopes,
        credentials_file,
    }) = args.command.as_ref()
    {
        let _ = dotenvy::dotenv();
        let app_config = crate::cli::provision_service_client::run(
            &AppConfig::try_from(Path::new(&args.config))
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?,
            application_uri,
            client_name,
            audiences.clone(),
            scopes.clone(),
            credentials_file,
        )
        .await;
        return app_config;
    }

    match dotenvy::dotenv() {
        Ok(_) => {}
        Err(error) => eprintln!("failed to load .env file: {error}"),
    }

    let cancellation_token = CancellationToken::new();
    #[cfg(unix)]
    let mut app_config = Arc::new(crate::startup::load_config(Path::new(&args.config))?);
    #[cfg(not(unix))]
    let mut app_config =
        Arc::new(AppConfig::try_from(Path::new(&args.config)).map_err(io::Error::other)?);

    // Check for public base URL (set by portless via PORTLESS_URL or manually via PUBLIC_BASE_URL)
    let has_public_base = public_base_url().is_some();
    // Override port from PORT env var (set by portless)
    let portless_port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok());
    if let Some(base_url) = public_base_url() {
        let mut config = Arc::try_unwrap(app_config).unwrap_or_else(|arc| (*arc).clone());
        config.api_public_uri = format!("{}/idp", base_url.trim_end_matches('/'));
        config.server.host = IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1));
        if let Some(port) = portless_port {
            config.server.port = port;
        }
        app_config = Arc::new(config);
    } else if let Some(port) = portless_port {
        // Even without public base URL, respect PORT env var
        let mut config = Arc::try_unwrap(app_config).unwrap_or_else(|arc| (*arc).clone());
        config.server.port = port;
        app_config = Arc::new(config);
    }

    env_logger::Builder::from_env(
        Env::default().default_filter_or(format!("{},automerge=warn", app_config.log_level)),
    )
    .init();

    #[cfg(unix)]
    if !has_public_base && matches!(args.command, None | Some(IdpCommand::Serve { .. })) {
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
        #[cfg(feature = "independent")]
        if let Some(setup) = crate::startup::prepare_setup(&app_config, address)? {
            let shutdown = shutdown_signal(cancellation_token.clone());
            let serving =
                serve_independent_create(app_config.clone(), setup, cancellation_token.clone());
            tokio::pin!(shutdown, serving);
            return select! {
                result = &mut serving => result,
                () = &mut shutdown => serving.await,
            };
        }
        #[cfg(not(feature = "independent"))]
        if crate::startup::prepare_setup(&app_config, address)?.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "independent create requires the 'independent' feature",
            ));
        }
    }

    let engine = crate::runtime::identity_client(&app_config).await?;
    let runtime = build_oauth_runtime(&app_config, engine).await?;

    let router = runtime
        .router()
        .layer(CorsLayer::very_permissive().allow_private_network(true))
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new().gzip(app_config.server.gzip));

    let run_serve = |host: Option<IpAddr>, port: Option<u16>| {
        let addr = if public_base_url().is_some() {
            bind_addr()
        } else {
            SocketAddr::from((
                host.unwrap_or(app_config.server.host),
                port.unwrap_or(app_config.server.port),
            ))
        };
        spawn(serve(router, addr, cancellation_token.clone()))
    };

    let command_handle = match args.command {
        #[cfg(unix)]
        Some(IdpCommand::ResetIndependent { .. }) => {
            return Err(io::Error::other("reset must run before serving"));
        }
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
    use cli::args::CliArgs;

    use crate::cli::run::run::IdpCommand;

    #[cfg(unix)]
    #[test]
    fn local_reset_requires_explicit_os_trust_removal() {
        assert!(
            CliArgs::<IdpCommand>::try_parse_from(["idp-server", "reset-independent"]).is_err()
        );
        CliArgs::<IdpCommand>::try_parse_from([
            "idp-server",
            "reset-independent",
            "--remove-os-trust",
        ])
        .expect("explicit local trust removal");
    }

    #[test]
    fn independent_startup_accepts_no_config_and_explicit_serve() {
        CliArgs::<IdpCommand>::try_parse_from(["idp-server"]).expect("default launch");
        CliArgs::<IdpCommand>::try_parse_from(["idp-server", "serve", "--port", "3000"])
            .expect("explicit serving launch");
    }
}
