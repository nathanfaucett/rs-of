use std::{io, path::PathBuf};

use server_common::{bind_addr, public_base_url};
use clap::{Args as ClapArgs, Parser, Subcommand};
use tokio_util::sync::CancellationToken;

use crate::{SetupHost, UnifiedConfig};

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;

#[derive(Parser)]
#[command(
    version,
    about = "Compose the IdP, Management, Device and Storage owners"
)]
struct Args {
    #[arg(long, short = 'c', global = true)]
    config: Option<PathBuf>,
    #[arg(long, global = true, conflicts_with = "config")]
    data_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    Serve {
        #[arg(long)]
        host: Option<std::net::IpAddr>,
        #[arg(long, short = 'p')]
        port: Option<u16>,
    },
    Create {
        #[command(flatten)]
        target: SetupTarget,
        #[arg(long)]
        administrator: String,
        #[arg(long, default_value = "My Node")]
        node: String,
        #[arg(long)]
        password_file: PathBuf,
    },
    Join {
        #[command(flatten)]
        target: SetupTarget,
        #[arg(long)]
        peer: String,
        #[arg(long, default_value = "My Node")]
        node: String,
    },
    #[cfg(feature = "completions")]
    Completions { shell: clap_complete::Shell },
}

#[derive(ClapArgs)]
struct SetupTarget {
    #[arg(long)]
    idp: String,
    #[arg(long)]
    management: String,
    #[arg(long)]
    device: String,
    #[arg(long)]
    storage: String,
    #[arg(long)]
    operation_nonce: String,
    #[arg(long)]
    ca_path: PathBuf,
    #[arg(long)]
    idp_code_file: PathBuf,
    #[arg(long)]
    device_code_file: PathBuf,
    #[arg(long)]
    storage_code_file: PathBuf,
}

async fn submit(
    target: SetupTarget,
    administrator: String,
    node: String,
    password: Option<String>,
    peer: Option<String>,
) -> io::Result<()> {
    server_common::exact_https(&target.management)?;
    let certificate = reqwest::Certificate::from_pem(&std::fs::read(&target.ca_path)?)
        .map_err(io::Error::other)?;
    let client = reqwest::Client::builder()
        .tls_built_in_native_certs(true)
        .add_root_certificate(certificate)
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(io::Error::other)?;
    let issuer = match &peer {
        Some(peer) => {
            server_common::IndependentPeer::discover_with_client(peer, client.clone())
                .await?
                .issuer
        }
        None => target.idp.clone(),
    };
    let operation = server_common::IndependentCreateIntent {
        purpose: if peer.is_some() { "join" } else { "create" }.into(),
        operation_nonce: target.operation_nonce,
        administrator,
        node,
        issuer,
        peer,
        idp: target.idp.clone(),
        device: target.device.clone(),
        storage: target.storage.clone(),
    };
    operation.canonical()?;
    for (i, (base, path)) in [
        (target.idp, target.idp_code_file),
        (target.device, target.device_code_file),
        (target.storage, target.storage_code_file),
    ]
    .into_iter()
    .enumerate()
    {
        let admission = path.parent().ok_or_else(|| {
            io::Error::other("owner capability requires its protected admission path")
        })?;
        let owner = ["idp", "device", "storage"][i];
        let Some(code) =
            server_common::SetupAdmission::protected_code(admission).map_err(io::Error::other)?
        else {
            observe_completed(&client, &operation, owner).await?;
            println!("Owner {owner} already completed the exact operation");
            continue;
        };
        let mut request = serde_json::to_value(&operation)?;
        if i == 0 {
            request["password"] = serde_json::json!(password);
        }
        let response = client
            .post(format!("{base}/setup/owner/{}", operation.purpose))
            .header("Authorization", format!("Setup {}", code.trim()))
            .json(&request)
            .send()
            .await
            .map_err(io::Error::other)?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            observe_completed(&client, &operation, owner).await?;
            continue;
        }
        if response.status() != reqwest::StatusCode::ACCEPTED {
            return Err(io::Error::other(format!(
                "owner {i} refused setup (HTTP {}); retain the exact nonce and operation for an explicit retry",
                response.status().as_u16()
            )));
        }
        println!("Owner {i} admitted the exact operation; observe {base}/setup/owner/status");
    }
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(180);
    loop {
        if client
            .get(format!("{}/health", target.management))
            .send()
            .await
            .is_ok_and(|response| response.status().is_success())
        {
            println!("Management is serving at {}", target.management);
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(io::Error::other(
                "Management did not become available at the supplied base; inspect each owner's status and the server configuration before retrying the exact operation",
            ));
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    Ok(())
}

async fn observe_completed(
    client: &reqwest::Client,
    operation: &server_common::IndependentCreateIntent,
    owner: &str,
) -> io::Result<()> {
    let base = operation.base(owner)?;
    let status: serde_json::Value = client
        .get(format!("{base}/setup/owner/status"))
        .send()
        .await
        .map_err(io::Error::other)?
        .error_for_status()
        .map_err(io::Error::other)?
        .json()
        .await
        .map_err(io::Error::other)?;
    if status["stage"] != "installation"
        || status["state"] != "ready"
        || status["ready"] != true
        || status["owners"] != serde_json::json!([owner])
    {
        return Err(io::Error::other(format!(
            "owner {owner} has no capability and is not currently ready; retain the exact operation for recovery"
        )));
    }
    let public: server_common::IndependentOwnerPublic = client
        .get(format!("{base}/setup/owner/public"))
        .send()
        .await
        .map_err(io::Error::other)?
        .error_for_status()
        .map_err(io::Error::other)?
        .json()
        .await
        .map_err(io::Error::other)?;
    if public.owner != owner || public.operation != *operation {
        return Err(io::Error::other(format!(
            "owner {owner} completion conflicts with the exact operation"
        )));
    }
    Ok(())
}

pub async fn run() -> io::Result<()> {
    let args = Args::parse();
    #[cfg(feature = "completions")]
    if let Some(Command::Completions { shell }) = args.command {
        use clap::CommandFactory;
        clap_complete::generate(
            shell,
            &mut Args::command(),
            "unified-server",
            &mut io::stdout(),
        );
        return Ok(());
    }
    match args.command {
        Some(Command::Create {
            target,
            administrator,
            node,
            password_file,
        }) => {
            let password = idp_server::local_provisioning::read_password(&password_file)?;
            return submit(target, administrator, node, Some(password), None).await;
        }
        Some(Command::Join { target, peer, node }) => {
            return submit(target, String::new(), node, None, Some(peer)).await;
        }
        _ => (),
    }
    let has_public_base = public_base_url().is_some();
    let portless_port = std::env::var("PORT").ok().and_then(|p| p.parse::<u16>().ok());

    let mut config = match args.config {
        Some(path) => UnifiedConfig::try_from(path.as_path()).map_err(io::Error::other)?,
        None => {
            let root = args
                .data_dir
                .unwrap_or_else(|| PathBuf::from("unified-data"));
            let address = if has_public_base {
                bind_addr()
            } else {
                match &args.command {
                    Some(Command::Serve { host, port }) => std::net::SocketAddr::new(
                        host.unwrap_or(std::net::Ipv4Addr::LOCALHOST.into()),
                        port.unwrap_or(3000),
                    ),
                    _ => "127.0.0.1:3000".parse().expect("literal loopback address"),
                }
            };
            UnifiedConfig::local(&root, address)
        }
    };

    if has_public_base {
        let base_url = public_base_url().expect("public base URL must be set");
        let base = base_url.trim_end_matches('/');
        config.idp.api_public_uri = format!("{}/idp", base);
        config.management.api_public_uri = format!("{}/management", base);
        config.device.api_public_uri = format!("{}/device", base);
        config.storage.api_public_uri = format!("{}/storage", base);
        config.listen_addr = bind_addr();
    } else if let Some(port) = portless_port {
        config.listen_addr.set_port(port);
    } else if let Some(Command::Serve { host, port }) = args.command {
        if let Some(host) = host {
            config.listen_addr.set_ip(host);
        }
        if let Some(port) = port {
            config.listen_addr.set_port(port);
        }
    }
    env_logger::Builder::from_env(
        env_logger::Env::default()
            .default_filter_or(format!("{},automerge=warn", config.idp.log_level)),
    )
    .init();
    let host = SetupHost::prepare(config)?;
    let cancellation = CancellationToken::new();
    let signal = tokio::spawn(cli::shutdown_signal(cancellation.clone()));
    let result = host.serve(cancellation).await;
    signal.abort();
    result
}
