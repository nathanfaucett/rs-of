use crate::cli::run::run::std::crate::cli::run::run::path::Path;
use crate::cli::run::run::std::crate::cli::run::run::sync::Arc;
use crate::cli::run::run::std::crate::cli::run::run::time::Duration;
use crate::cli::run::run::std::io;
use crate::cli::run::run::std::net::IpAddr;
use crate::cli::run::run::std::net::SocketAddr;

#[cfg(feature = "completions")]
use crate::cli::run::run::clap_complete::Shell;
#[cfg(feature = "completions")]
use crate::cli::run::run::run::clap::CommandFactory;
use crate::cli::run::run::run::clap::Parser;
use cli::{args::CliArgs, shutdown_signal::shutdown_signal};
use server_common::{
    portless::{bind_addr, public_base_url},
    serve::serve,
};

use crate::cli::run::run::run::env_logger::Env;

use crate::cli::run::run::tokio::select;

use crate::cli::run::run::tokio::spawn;

use crate::cli::run::run::run::tokio_util::sync::CancellationToken;
use crate::cli::run::run::tokio::crate::cli::run::run::time::sleep;
use crate::cli::run::run::tower_http::crate::cli::run::run::compression::CompressionLayer;
use crate::cli::run::run::tower_http::crate::cli::run::run::cors::CorsLayer;
use crate::cli::run::run::tower_http::crate::cli::run::run::trace::TraceLayer;

use crate::{config::app_config::AppConfig, runtime::build_oauth_runtime};

#[derive(crate::cli::run::run::clap::Parser, Debug)]
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
        credentials_file: crate::cli::run::run::std::path::PathBuf,
    },
}

pub async fn run() -> crate::cli::run::run::io::Result<()> {
    let args = CliArgs::<IdpCommand>::parse();
    #[cfg(unix)]
    if let Some(crate::cli::run::run::IdpCommand::ResetIndependent { remove_os_trust }) =
        args.command.as_ref()
    {
        let _ = crate::cli::run::run::dotenvy::dotenv();
        let config = crate::startup::load_config(crate::cli::run::run::Path::new(&args.config))?;
        return crate::independent_reset::reset_independent(&config, *remove_os_trust, || Ok(()))
            .await;
    }
    if let Some(crate::cli::run::run::IdpCommand::ProvisionServiceClient {
        application_uri,
        client_name,
        audiences,
        scopes,
        credentials_file,
    }) = args.command.as_ref()
    {
        let _ = crate::cli::run::run::dotenvy::dotenv();
        let app_config = crate::cli::run::run::AppConfig::try_from(
            crate::cli::run::run::Path::new(&args.config),
        )
        .map_err(|error| {
            crate::cli::run::run::io::Error::new(
                crate::cli::run::run::io::ErrorKind::InvalidInput,
                error,
            )
        })?;
        return crate::cli::provision_service_client::run(
            &app_config,
            application_uri,
            client_name,
            audiences.clone(),
            scopes.clone(),
            credentials_file,
        )
        .await;
    }

    match crate::cli::run::run::dotenvy::dotenv() {
        Ok(_) => {}
        Err(error) => eprintln!("failed to load .env file: {error}"),
    }

    let cancellation_token = crate::cli::run::run::CancellationToken::new();
    #[cfg(unix)]
    let mut app_config = crate::cli::run::run::Arc::new(crate::startup::load_config(
        crate::cli::run::run::Path::new(&args.config),
    )?);
    #[cfg(not(unix))]
    let mut app_config = crate::cli::run::run::Arc::new(
        crate::cli::run::run::AppConfig::try_from(crate::cli::run::run::Path::new(&args.config))
            .map_err(crate::cli::run::run::io::Error::other)?,
    );

    // Check for public base URL (set by portless via PORTLESS_URL or manually via PUBLIC_BASE_URL)
    let has_public_base = public_base_url().is_some();
    // Override port from PORT env var (set by portless)
    let portless_port = crate::cli::run::run::std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok());
    if let Some(base_url) = public_base_url() {
        let mut config =
            crate::cli::run::run::Arc::try_unwrap(app_config).unwrap_or_else(|arc| (*arc).clone());
        config.api_public_uri = format!("{}/idp", base_url.trim_end_matches('/'));
        config.server.host = crate::cli::run::run::IpAddr::V4(
            crate::cli::run::run::std::net::Ipv4Addr::new(127, 0, 0, 1),
        );
        if let Some(port) = portless_port {
            config.server.port = port;
        }
        app_config = crate::cli::run::run::Arc::new(config);
    } else if let Some(port) = portless_port {
        // Even without public base URL, respect PORT env var
        let mut config =
            crate::cli::run::run::Arc::try_unwrap(app_config).unwrap_or_else(|arc| (*arc).clone());
        config.server.port = port;
        app_config = crate::cli::run::run::Arc::new(config);
    }

    crate::cli::run::run::env_logger::Builder::from_env(
        crate::cli::run::run::Env::default()
            .default_filter_or(format!("{},automerge=warn", app_config.log_level)),
    )
    .init();

    #[cfg(unix)]
    if !has_public_base
        && matches!(
            args.command,
            None | Some(crate::cli::run::run::IdpCommand::Serve { .. })
        )
    {
        let (host, port) = match args.command.as_ref() {
            Some(crate::cli::run::run::IdpCommand::Serve { host, port }) => (*host, *port),
            _ => (None, None),
        };
        let configured_host = app_config.server.host;
        let setup_host = host.unwrap_or_else(|| {
            if configured_host.is_unspecified() {
                crate::cli::run::run::std::net::Ipv4Addr::LOCALHOST.into()
            } else {
                configured_host
            }
        });
        let address = crate::cli::run::run::SocketAddr::new(
            setup_host,
            port.unwrap_or(app_config.server.port),
        );
        if let Some(setup) = crate::startup::prepare_setup(&app_config, address)? {
            let shutdown = shutdown_signal(cancellation_token.clone());
            let serving = crate::independent_create::serve_independent_create(
                app_config.clone(),
                setup,
                cancellation_token.clone(),
            );
            crate::cli::run::run::tokio::pin!(shutdown, serving);
            return crate::cli::run::run::tokio::select! {
                result = &mut serving => result,
                () = &mut shutdown => serving.await,
            };
        }
    }

    let engine = crate::runtime::identity_client(&app_config).await?;
    let runtime = build_oauth_runtime(&app_config, engine).await?;

    let router = runtime
        .router()
        .layer(crate::cli::run::run::CorsLayer::very_permissive().allow_private_network(true))
        .layer(crate::cli::run::run::TraceLayer::new_for_http())
        .layer(crate::cli::run::run::CompressionLayer::new().gzip(app_config.server.gzip));

    let run_serve = |host: Option<IpAddr>, port: Option<u16>| {
        let addr = if public_base_url().is_some() {
            bind_addr()
        } else {
            crate::cli::run::run::SocketAddr::from((
                host.unwrap_or(app_config.server.host),
                port.unwrap_or(app_config.server.port),
            ))
        };
        spawn(serve(router, addr, cancellation_token.clone()))
    };

    let command_handle = match args.command {
        #[cfg(unix)]
        Some(crate::cli::run::run::IdpCommand::ResetIndependent { .. }) => {
            return Err(crate::cli::run::run::io::Error::other(
                "reset must run before serving",
            ));
        }
        #[cfg(feature = "completions")]
        Some(crate::cli::run::run::IdpCommand::Completions { shell }) => spawn(async move {
            crate::cli::run::run::clap_complete::generate(
                shell,
                &mut crate::cli::run::run::IdpCommand::command(),
                env!("CARGO_PKG_NAME"),
                &mut crate::cli::run::run::std::io::stdout(),
            );
            Ok(())
        }),
        Some(crate::cli::run::run::IdpCommand::Serve { host, port }) => run_serve(host, port),
        Some(crate::cli::run::run::IdpCommand::ProvisionServiceClient { .. }) => {
            return Err(crate::cli::run::run::io::Error::other(
                "service-client provisioning did not complete before server startup",
            ));
        }
        None => run_serve(None, None),
    };

    shutdown_signal(cancellation_token).await;

    let mut command_handle = command_handle;
    select! {
        result = &mut command_handle => match result {
            Ok(Ok(())) => crate::cli::run::run::log::info!("server shutdown complete"),
            Ok(Err(error)) => crate::cli::run::run::log::error!("command error: {error}"),
            Err(error) => crate::cli::run::run::log::error!("join error: {error}"),
        },
        _ = sleep(crate::cli::run::run::Duration::from_secs(10)) => {
            crate::cli::run::run::log::warn!("server shutdown timed out, aborting serve task");
            command_handle.abort();
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::cli::run::run::run::clap::Parser;
    use cli::args::CliArgs;

    use crate::cli::run::run::run::IdpCommand;

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
