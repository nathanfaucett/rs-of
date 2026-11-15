use crate::cli::run::run::std::crate::cli::run::run::path::Path;
use crate::cli::run::run::std::crate::cli::run::run::sync::Arc;
use crate::cli::run::run::std::crate::cli::run::run::time::Duration;
use crate::cli::run::run::std::io;
use crate::cli::run::run::std::net::IpAddr;
use crate::cli::run::run::std::net::SocketAddr;

use crate::cli::run::run::run::clap::Parser;
use cli::{args::CliArgs, shutdown_signal::shutdown_signal};
use server_common::{
    portless::{bind_addr, public_base_url},
    serve::serve,
};

use crate::cli::run::run::run::env_logger::Env;

use crate::cli::run::run::run::tokio_util::sync::CancellationToken;
use crate::cli::run::run::tokio::crate::cli::run::run::time::sleep;
use crate::cli::run::run::tokio::select;
use crate::cli::run::run::tokio::spawn;
use crate::cli::run::run::tower_http::crate::cli::run::run::compression::CompressionLayer;
use crate::cli::run::run::tower_http::crate::cli::run::run::cors::CorsLayer;
use crate::cli::run::run::tower_http::crate::cli::run::run::trace::TraceLayer;
use device_service::hosted_control_plane::HostedControlPlane;

#[cfg(not(unix))]
use crate::config::app_config::AppConfig;
use crate::runtime::build_router;

#[derive(crate::cli::run::run::clap::Parser, Debug)]
enum DeviceCommand {
    #[cfg(unix)]
    #[command(
        about = "LOCAL destructive reset of the selected independent Management config. Stop its host first. Deletes local data and exact protected secrets; requires explicit OS CA trust removal with native consent. Does not revoke remote membership or reset the app."
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
    Completions { shell: clap_complete::Shell },
}

pub async fn run() -> crate::cli::run::run::io::Result<()> {
    match crate::cli::run::run::dotenvy::dotenv() {
        Ok(_) => {}
        Err(error) => eprintln!("failed to load .env file: {error}"),
    }

    let args = CliArgs::<DeviceCommand>::parse();
    let cancellation_token = crate::cli::run::run::CancellationToken::new();
    #[cfg(unix)]
    let mut app_config = crate::cli::run::run::Arc::new(crate::startup::load_config(
        crate::cli::run::run::Path::new(&args.config),
    )?);
    #[cfg(not(unix))]
    let mut app_config = crate::cli::run::run::Arc::new(
        match crate::cli::run::run::AppConfig::try_from(crate::cli::run::run::Path::new(
            &args.config,
        )) {
            Ok(app_config) => app_config,
            Err(error) => {
                eprintln!("failed to load config {:?}: {error}", args.config);
                crate::cli::run::run::AppConfig::default()
            }
        },
    );

    // Check for public base URL (set by portless via PORTLESS_URL or manually via PUBLIC_BASE_URL)
    let has_public_base = public_base_url().is_some();
    let portless_port = crate::cli::run::run::std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok());
    if has_public_base {
        let base_url = public_base_url().expect("public base URL must be set");
        let mut config =
            crate::cli::run::run::Arc::try_unwrap(app_config).unwrap_or_else(|arc| (*arc).clone());
        config.api_public_uri = format!("{}/device", base_url.trim_end_matches('/'));
        config.server.host = crate::cli::run::run::IpAddr::V4(
            crate::cli::run::run::std::net::Ipv4Addr::new(127, 0, 0, 1),
        );
        if let Some(port) = portless_port {
            config.server.port = port;
        }
        app_config = crate::cli::run::run::Arc::new(config);
    } else if let Some(port) = portless_port {
        let mut config =
            crate::cli::run::run::Arc::try_unwrap(app_config).unwrap_or_else(|arc| (*arc).clone());
        config.server.port = port;
        app_config = crate::cli::run::run::Arc::new(config);
    }

    #[cfg(unix)]
    if !has_public_base
        && matches!(
            args.command,
            None | Some(crate::cli::run::run::DeviceCommand::Serve { .. })
        )
    {
        let (host, port) = match args.command.as_ref() {
            Some(crate::cli::run::run::DeviceCommand::Serve { host, port }) => (*host, *port),
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
            crate::cli::run::run::env_logger::Builder::from_env(
                crate::cli::run::run::Env::default()
                    .default_filter_or(format!("{},automerge=warn", app_config.log_level)),
            )
            .init();
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

    if app_config.idp_api_base.trim().is_empty()
        || app_config.storage_api_base.trim().is_empty()
        || app_config.expected_issuer.trim().is_empty()
        || app_config.storage_audience.trim().is_empty()
    {
        return Err(crate::cli::run::run::io::Error::new(
            crate::cli::run::run::io::ErrorKind::InvalidInput,
            "idp_api_base, storage_api_base, expected_issuer and storage_audience are required",
        ));
    }

    let idp_client_id =
        crate::cli::run::run::std::env::var("SERVER_IDP_OAUTH_CLIENT_ID").map_err(|_| {
            crate::cli::run::run::io::Error::new(
                crate::cli::run::run::io::ErrorKind::InvalidInput,
                "SERVER_IDP_OAUTH_CLIENT_ID is required",
            )
        })?;
    let idp_client_secret = crate::cli::run::run::std::env::var("SERVER_IDP_OAUTH_CLIENT_SECRET")
        .map_err(|_| {
        crate::cli::run::run::io::Error::new(
            crate::cli::run::run::io::ErrorKind::InvalidInput,
            "SERVER_IDP_OAUTH_CLIENT_SECRET is required",
        )
    })?;
    let idp_service_audience = crate::cli::run::run::std::env::var("SERVER_IDP_SERVICE_AUDIENCE")
        .map_err(|_| {
        crate::cli::run::run::io::Error::new(
            crate::cli::run::run::io::ErrorKind::InvalidInput,
            "SERVER_IDP_SERVICE_AUDIENCE is required",
        )
    })?;
    let control_plane = crate::cli::run::run::Arc::new(
        crate::cli::run::run::HostedControlPlane::new_with_services(
            &app_config.idp_api_base,
            &app_config.storage_api_base,
            &app_config.expected_issuer,
        )
        .and_then(|control_plane| {
            control_plane.with_idp_service_client(
                idp_client_id,
                idp_client_secret,
                idp_service_audience,
            )
        })
        .and_then(
            |control_plane| match app_config.idp_permission_evaluator_client_id.as_deref() {
                Some(client_id) => control_plane.with_permission_evaluator(client_id),
                None => Ok(control_plane),
            },
        )
        .map_err(|error| {
            crate::cli::run::run::io::Error::new(
                crate::cli::run::run::io::ErrorKind::InvalidInput,
                error,
            )
        })?,
    );

    crate::cli::run::run::env_logger::Builder::from_env(
        crate::cli::run::run::Env::default()
            .default_filter_or(format!("{},automerge=warn", app_config.log_level)),
    )
    .init();

    let engine = crate::database_client::device_client(&app_config).await?;
    let router = build_router(
        crate::database_client::identity_client(&app_config).await?,
        engine,
        &app_config.api_public_uri,
        app_config.server.prefix(),
        &app_config.storage_audience,
        control_plane,
        None,
    )
    .await
    .map_err(crate::cli::run::run::io::Error::other)?
    .layer(crate::cli::run::run::CorsLayer::very_permissive().allow_private_network(true))
    .layer(crate::cli::run::run::TraceLayer::new_for_http())
    .layer(crate::cli::run::run::CompressionLayer::new().gzip(app_config.server.gzip));

    let run_serve = |host: Option<IpAddr>, port: Option<u16>| {
        let addr = if has_public_base {
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
        Some(crate::cli::run::run::DeviceCommand::ResetIndependent { .. }) => {
            return Err(crate::cli::run::run::io::Error::other(
                "reset must run before serving",
            ));
        }
        #[cfg(feature = "completions")]
        Some(crate::cli::run::run::DeviceCommand::Completions { shell }) => {
            spawn(async move { cli::completions::run_completions(shell).await })
        }
        Some(crate::cli::run::run::DeviceCommand::Serve { host, port }) => run_serve(host, port),
        None => run_serve(None, None),
    };

    shutdown_signal(cancellation_token).await;

    let shutdown_timeout = crate::cli::run::run::Duration::from_secs(10);
    let mut command_handle = command_handle;
    select! {
        result = &mut command_handle => match result {
            Ok(Ok(())) => crate::cli::run::run::log::info!("server shutdown complete"),
            Ok(Err(error)) => crate::cli::run::run::log::error!("command error: {error}"),
            Err(error) => crate::cli::run::run::log::error!("join error: {error}"),
        },
        _ = sleep(shutdown_timeout) => {
            crate::cli::run::run::log::warn!("server shutdown timed out after {shutdown_timeout:?}, aborting serve task");
            command_handle.abort();
            sleep(crate::cli::run::run::Duration::from_millis(100)).await;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::cli::run::run::run::clap::Parser;
    use cli::args::CliArgs;

    use crate::cli::run::run::run::DeviceCommand;

    #[cfg(unix)]
    #[test]
    fn local_reset_requires_explicit_os_trust_removal() {
        assert!(
            CliArgs::<DeviceCommand>::try_parse_from(["device-server", "reset-independent"])
                .is_err()
        );
        CliArgs::<DeviceCommand>::try_parse_from([
            "device-server",
            "reset-independent",
            "--remove-os-trust",
        ])
        .expect("explicit local trust removal");
    }

    #[test]
    fn independent_startup_accepts_no_config_and_explicit_serve() {
        CliArgs::<DeviceCommand>::try_parse_from(["device-server"]).expect("default launch");
        CliArgs::<DeviceCommand>::try_parse_from(["device-server", "serve", "--port", "3001"])
            .expect("explicit serving launch");
    }
}
