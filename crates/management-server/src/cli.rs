use std::{io, net::IpAddr, path::Path, sync::Arc};

use clap::Parser;
use cli::{CliArgs, shutdown_signal};
use device_service::HostedControlPlane;
use tokio_util::sync::CancellationToken;

use crate::{AppConfig, build_router, identity_client};

#[derive(Debug, Parser)]
enum ManagementCommand {
    Serve {
        #[arg(long, short = 'p')]
        port: Option<u16>,
        #[arg(long, short = 'H')]
        host: Option<IpAddr>,
    },
    #[cfg(unix)]
    ResetIndependent {
        #[arg(long, required = true)]
        remove_os_trust: bool,
    },
}

pub async fn run() -> io::Result<()> {
    let _ = dotenvy::dotenv();
    let args = CliArgs::<ManagementCommand>::parse();
    let config = AppConfig::try_from(Path::new(&args.config)).map_err(io::Error::other)?;
    #[cfg(unix)]
    if let Some(ManagementCommand::ResetIndependent { remove_os_trust }) = &args.command {
        return crate::reset_independent(&config, *remove_os_trust).await;
    }
    let required = |name: &str| {
        std::env::var(name)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, format!("{name} is required")))
    };
    let control = HostedControlPlane::new_with_services(
        &config.idp_api_base,
        &config.storage_api_base,
        &config.expected_issuer,
    )
    .and_then(|control| {
        control.with_idp_service_client(
            required("SERVER_IDP_OAUTH_CLIENT_ID").map_err(|error| error.to_string())?,
            required("SERVER_IDP_OAUTH_CLIENT_SECRET").map_err(|error| error.to_string())?,
            required("SERVER_IDP_SERVICE_AUDIENCE").map_err(|error| error.to_string())?,
        )
    })
    .map_err(io::Error::other)?;
    let control = match config.idp_permission_evaluator_client_id.as_deref() {
        Some(client) => control
            .with_permission_evaluator(client)
            .map_err(io::Error::other)?,
        None => control,
    };
    let router = build_router(
        identity_client(&config).await?,
        &config.api_public_uri,
        config.server.prefix(),
        Arc::new(control),
    )
    .await
    .map_err(io::Error::other)?;
    let (host, port) = match args.command {
        Some(ManagementCommand::Serve { host, port }) => (
            host.unwrap_or(config.server.host),
            port.unwrap_or(config.server.port),
        ),
        _ => (config.server.host, config.server.port),
    };
    let shutdown = CancellationToken::new();
    let signal = tokio::spawn(shutdown_signal(shutdown.clone()));
    let result = api::serve(router, (host, port).into(), shutdown).await;
    signal.abort();
    result
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use cli::CliArgs;

    use super::ManagementCommand;

    #[test]
    fn policy_consumer_cli_has_no_device_create_or_join() {
        CliArgs::<ManagementCommand>::try_parse_from([
            "management-server",
            "serve",
            "--port",
            "3001",
        ])
        .expect("standalone policy serving");
        for command in ["create", "join"] {
            assert!(
                CliArgs::<ManagementCommand>::try_parse_from(["management-server", command])
                    .is_err()
            );
        }
    }
}
