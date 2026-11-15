use crate::cli::run::run::std::crate::cli::run::run::path::Path;
use crate::cli::run::run::std::crate::cli::run::run::sync::Arc;
use crate::cli::run::run::std::crate::cli::run::run::time::Duration;
use crate::cli::run::run::std::io;
use crate::cli::run::run::std::net::IpAddr;
use crate::cli::run::run::std::net::SocketAddr;

use server_common::{
    portless::{bind_addr, public_base_url},
    serve::serve,
};

use crate::cli::run::run::run::clap::Parser;
use crate::cli::run::run::run::env_logger::Env;
use cli::{args::CliArgs, shutdown_signal::shutdown_signal};

use crate::cli::run::run::tokio::select;

use crate::cli::run::run::tokio::spawn;

use crate::cli::run::run::run::tokio_util::sync::CancellationToken;
use crate::cli::run::run::tokio::crate::cli::run::run::time::sleep;
use crate::cli::run::run::tower_http::crate::cli::run::run::compression::CompressionLayer;
use crate::cli::run::run::tower_http::crate::cli::run::run::cors::CorsLayer;
use crate::cli::run::run::tower_http::crate::cli::run::run::trace::TraceLayer;

#[cfg(not(unix))]
use crate::config::app_config::AppConfig;
use crate::{
    device_client::DeviceClient,
    idp_client::IdpClient,
    management_client::ManagementClient,
    router::state::RouterState,
    runtime::{build_http_runtime, build_runtime},
};

#[derive(crate::cli::run::run::clap::Parser, Debug)]
enum StorageCommand {
    #[cfg(unix)]
    #[command(
        about = "LOCAL destructive reset of the selected independent Storage config. Stop its host first. Deletes local data and exact protected secrets; requires explicit OS CA trust removal with native consent. Does not revoke remote membership or reset the app."
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
        Err(e) => {
            eprintln!("failed to load .env file: {}", e);
        }
    }

    let args = CliArgs::<StorageCommand>::parse();

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
            Err(e) => {
                eprintln!("failed to load config {:?}: {}", args.config, e);
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
        config.api_public_uri = format!("{}/storage", base_url.trim_end_matches('/'));
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
            None | Some(crate::cli::run::run::StorageCommand::Serve { .. })
        )
    {
        let (host, port) = match args.command.as_ref() {
            Some(crate::cli::run::run::StorageCommand::Serve { host, port }) => (*host, *port),
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

    crate::cli::run::run::env_logger::Builder::from_env(
        crate::cli::run::run::Env::default()
            .default_filter_or(format!("{},automerge=warn", app_config.log_level)),
    )
    .init();

    let mut router_state = if app_config.idp_api_base_uri.trim().is_empty() {
        crate::cli::run::run::RouterState::new(
            &app_config.api_public_uri,
            &app_config.device_endpoint_id.clone().unwrap_or_default(),
        )
    } else {
        let client_id = app_config
            .idp_oauth_client_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                crate::cli::run::run::io::Error::new(
                    crate::cli::run::run::io::ErrorKind::InvalidInput,
                    "STORAGE_IDP_OAUTH_CLIENT_ID is required when STORAGE_IDP_API_BASE_URI is configured",
                )
            })?;
        let client_secret = app_config
            .idp_oauth_client_secret
            .as_deref()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                crate::cli::run::run::io::Error::new(
                    crate::cli::run::run::io::ErrorKind::InvalidInput,
                    "STORAGE_IDP_OAUTH_CLIENT_SECRET is required when STORAGE_IDP_API_BASE_URI is configured",
                )
            })?;
        let issuer = app_config
            .idp_issuer_uri
            .trim()
            .strip_suffix('/')
            .unwrap_or(app_config.idp_issuer_uri.trim());
        if issuer.is_empty() {
            return Err(crate::cli::run::run::io::Error::new(
                crate::cli::run::run::io::ErrorKind::InvalidInput,
                "STORAGE_IDP_ISSUER_URI is required when STORAGE_IDP_API_BASE_URI is configured",
            ));
        }
        let audience = app_config
            .idp_service_audience
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                crate::cli::run::run::io::Error::new(
                    crate::cli::run::run::io::ErrorKind::InvalidInput,
                    "STORAGE_IDP_SERVICE_AUDIENCE is required when STORAGE_IDP_API_BASE_URI is configured",
                )
            })?;
        let idp_client = crate::cli::run::run::IdpClient::new(
            &app_config.idp_api_base_uri,
            client_id,
            client_secret,
            issuer,
            audience,
        )
        .map_err(|error| {
            crate::cli::run::run::io::Error::new(
                crate::cli::run::run::io::ErrorKind::InvalidInput,
                error,
            )
        })?;
        crate::cli::run::run::RouterState::new(
            &app_config.api_public_uri,
            &app_config.device_endpoint_id.clone().unwrap_or_default(),
        )
        .with_idp_client(idp_client)
    };
    if !app_config.device_api_base_uri.trim().is_empty() {
        let client_id = app_config
            .device_oauth_client_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| crate::cli::run::run::io::Error::new(
                crate::cli::run::run::io::ErrorKind::InvalidInput,
                "STORAGE_MANAGEMENT_OAUTH_CLIENT_ID is required when STORAGE_MANAGEMENT_API_BASE_URI is configured",
            ))?;
        let client_secret = app_config
            .device_oauth_client_secret
            .as_deref()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| crate::cli::run::run::io::Error::new(
                crate::cli::run::run::io::ErrorKind::InvalidInput,
                "STORAGE_MANAGEMENT_OAUTH_CLIENT_SECRET is required when STORAGE_MANAGEMENT_API_BASE_URI is configured",
            ))?;
        let audience = app_config
            .device_service_audience
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| crate::cli::run::run::io::Error::new(
                crate::cli::run::run::io::ErrorKind::InvalidInput,
                "STORAGE_MANAGEMENT_SERVICE_AUDIENCE is required when STORAGE_MANAGEMENT_API_BASE_URI is configured",
            ))?;
        let idp_api_base_uri = app_config
            .idp_api_base_uri
            .trim()
            .strip_suffix('/')
            .unwrap_or(app_config.idp_api_base_uri.trim());
        if idp_api_base_uri.is_empty() || app_config.idp_issuer_uri.trim().is_empty() {
            return Err(crate::cli::run::run::io::Error::new(
                crate::cli::run::run::io::ErrorKind::InvalidInput,
                "STORAGE_IDP_API_BASE_URI and STORAGE_IDP_ISSUER_URI are required when STORAGE_MANAGEMENT_API_BASE_URI is configured",
            ));
        }
        let device_client = crate::cli::run::run::DeviceClient::new(
            &app_config.device_api_base_uri,
            idp_api_base_uri,
            client_id,
            client_secret,
            &app_config.idp_issuer_uri,
            audience,
        )
        .map_err(|error| {
            crate::cli::run::run::io::Error::new(
                crate::cli::run::run::io::ErrorKind::InvalidInput,
                error,
            )
        })?;
        router_state = router_state.with_device_client(device_client);
    }

    // Create ManagementClient if configured
    if !app_config.management_api_base_uri.trim().is_empty() {
        let client_id = app_config
            .management_oauth_client_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| crate::cli::run::run::io::Error::new(
                crate::cli::run::run::io::ErrorKind::InvalidInput,
                "STORAGE_MANAGEMENT_OAUTH_CLIENT_ID is required when STORAGE_MANAGEMENT_API_BASE_URI is configured",
            ))?;
        let client_secret = app_config
            .management_oauth_client_secret
            .as_deref()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| crate::cli::run::run::io::Error::new(
                crate::cli::run::run::io::ErrorKind::InvalidInput,
                "STORAGE_MANAGEMENT_OAUTH_CLIENT_SECRET is required when STORAGE_MANAGEMENT_API_BASE_URI is configured",
            ))?;
        let _audience = app_config
            .management_service_audience
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| crate::cli::run::run::io::Error::new(
                crate::cli::run::run::io::ErrorKind::InvalidInput,
                "STORAGE_MANAGEMENT_SERVICE_AUDIENCE is required when STORAGE_MANAGEMENT_API_BASE_URI is configured",
            ))?;
        let idp_api_base_uri = app_config
            .idp_api_base_uri
            .trim()
            .strip_suffix('/')
            .unwrap_or(app_config.idp_api_base_uri.trim());
        if idp_api_base_uri.is_empty() {
            return Err(crate::cli::run::run::io::Error::new(
                crate::cli::run::run::io::ErrorKind::InvalidInput,
                "STORAGE_IDP_API_BASE_URI is required when STORAGE_MANAGEMENT_API_BASE_URI is configured",
            ));
        }
        let management_client = crate::cli::run::run::ManagementClient::new(
            &app_config.management_api_base_uri,
            idp_api_base_uri,
            client_id,
            client_secret,
        )
        .map_err(|error| {
            crate::cli::run::run::io::Error::new(
                crate::cli::run::run::io::ErrorKind::InvalidInput,
                error,
            )
        })?;
        router_state = router_state.with_management_client(management_client);
    }

    let prefix = app_config.server.prefix();
    let mut storage_runtime = if router_state.device_client.is_some() {
        let endpoint = app_config.device_endpoint_id.as_deref()
            .ok_or_else(|| crate::cli::run::run::io::Error::new(crate::cli::run::run::io::ErrorKind::InvalidInput,
                "STORAGE_MANAGEMENT_ENDPOINT_ID must identify the current Management-approved endpoint; Storage does not create transport identity"))?
            .parse().map_err(|_| crate::cli::run::run::io::Error::new(crate::cli::run::run::io::ErrorKind::InvalidInput,
                "invalid STORAGE_MANAGEMENT_ENDPOINT_ID"))?;
        build_http_runtime(
            router_state,
            crate::cli::run::run::Path::new(&app_config.data_dir),
            prefix,
            endpoint,
            cancellation_token.clone(),
        )?
    } else {
        build_runtime(
            router_state,
            crate::cli::run::run::Path::new(&app_config.data_dir),
            prefix,
            prefix,
            None,
            cancellation_token.clone(),
        )?
    };
    let router = storage_runtime
        .router()
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

        storage_runtime.start_background_tasks()?;
        Ok::<_, crate::cli::run::run::io::Error>(spawn(serve(
            router,
            addr,
            cancellation_token.clone(),
        )))
    };

    let command_handle = match args.command {
        #[cfg(unix)]
        Some(crate::cli::run::run::StorageCommand::ResetIndependent { .. }) => {
            return Err(crate::cli::run::run::io::Error::other(
                "reset must run before serving",
            ));
        }
        #[cfg(feature = "completions")]
        Some(crate::cli::run::run::StorageCommand::Completions { shell }) => {
            spawn(async move { cli::completions::run_completions(shell).await })
        }
        Some(crate::cli::run::run::StorageCommand::Serve { host, port }) => run_serve(host, port)?,
        None => run_serve(None, None)?,
    };

    shutdown_signal(cancellation_token).await;
    if let Err(error) = storage_runtime.shutdown().await {
        crate::cli::run::run::log::warn!("Storage replication runtime shutdown failed: {error}");
    }

    let shutdown_timeout = crate::cli::run::run::Duration::from_secs(10);
    let mut command_handle = command_handle;
    select! {
      res = &mut command_handle => {
        match res {
          Ok(Ok(_)) => crate::cli::run::run::log::info!("server shutdown complete"),
          Ok(Err(e)) => crate::cli::run::run::log::error!("command error: {}", e),
          Err(e) => crate::cli::run::run::log::error!("join error: {}", e),
        }
      }
      _ = sleep(shutdown_timeout) => {
        crate::cli::run::run::log::warn!("server shutdown timed out after {:?}, aborting serve task", shutdown_timeout);
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

    use crate::cli::run::run::run::StorageCommand;

    #[cfg(unix)]
    #[test]
    fn local_reset_requires_explicit_os_trust_removal() {
        assert!(
            CliArgs::<StorageCommand>::try_parse_from(["storage-server", "reset-independent"])
                .is_err()
        );
        CliArgs::<StorageCommand>::try_parse_from([
            "storage-server",
            "reset-independent",
            "--remove-os-trust",
        ])
        .expect("explicit local trust removal");
    }

    #[test]
    fn independent_startup_accepts_no_config_and_explicit_serve() {
        CliArgs::<StorageCommand>::try_parse_from(["storage-server"]).expect("default launch");
        CliArgs::<StorageCommand>::try_parse_from(["storage-server", "serve", "--port", "3002"])
            .expect("explicit serving launch");
    }
}
