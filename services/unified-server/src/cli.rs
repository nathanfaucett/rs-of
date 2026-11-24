use std::{
    env, fs, io,
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
};

use clap::{Parser, Subcommand};
use cli::shutdown_signal::shutdown_signal;
use tokio::spawn;
use tokio_util::sync::CancellationToken;

use crate::config::{SeedConfig, UnifiedConfig};
#[cfg(all(unix, feature = "independent-contracts"))]
use crate::first_run::serve as serve_first_run;
use crate::runtime::serve;

#[derive(Parser)]
#[command(
    version,
    about = "Local development server. Do not expose seeded credentials to untrusted networks."
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Local {
        #[arg(long, default_value = ".data")]
        data_dir: PathBuf,
        #[arg(long, default_value_t = 1355)]
        port: u16,
        #[arg(long, default_value = "https://of.localhost:1355")]
        origin: String,
        #[arg(long, default_value = "https://password-manager.localhost:1355")]
        password_manager_origin: Option<String>,
        #[arg(
            long,
            help = "JSON file with additional users: {\"users\":[{\"username\":\"test\",\"password\":\"secret\"}]}"
        )]
        seed: Option<PathBuf>,
    },
    #[cfg(all(unix, feature = "independent-contracts"))]
    /// Create a new Installation through local Portless HTTPS, without seed data.
    Setup {
        #[arg(long, default_value = ".data/setup")]
        data_dir: PathBuf,
        #[arg(long, default_value = "https://of.localhost:1355")]
        origin: String,
    },
    #[cfg(feature = "completions")]
    Completions { shell: clap_complete::Shell },
}

/// Runs the local unified server command.
///
/// # Errors
/// Returns an error for seed input, configuration, startup, or serving failure.
pub async fn run() -> io::Result<()> {
    let args = Args::parse();
    let (data_dir, port, origin, password_manager_origin, seed) = match args.command {
        Command::Local {
            data_dir,
            port,
            origin,
            password_manager_origin,
            seed,
        } => (data_dir, port, origin, password_manager_origin, seed),
        #[cfg(all(unix, feature = "independent-contracts"))]
        Command::Setup { data_dir, origin } => {
            let public_origin = env::var("PORTLESS_URL")
                .map_err(|_error| io::Error::other("setup requires the Portless HTTPS URL"))?;
            let port = env::var("PORT")
                .ok()
                .and_then(|port| port.parse::<u16>().ok())
                .filter(|port| *port != 0)
                .ok_or_else(|| io::Error::other("setup requires a nonzero Portless PORT"))?;
            if env::var("PUBLIC_BASE_URL").is_ok_and(|url| url != public_origin) {
                return Err(io::Error::other("setup public URL must match PORTLESS_URL"));
            }
            return with_shutdown(|shutdown| {
                serve_first_run(data_dir, origin, public_origin, port, shutdown)
            })
            .await;
        }
        #[cfg(feature = "completions")]
        Command::Completions { shell } => {
            use clap::CommandFactory;
            clap_complete::generate(
                shell,
                &mut Args::command(),
                "unified-server",
                &mut io::stdout(),
            );
            return Ok(());
        }
    };
    let seed = seed
        .map(|path| -> io::Result<SeedConfig> {
            serde_json::from_slice(&fs::read(path)?).map_err(io::Error::other)
        })
        .transpose()?;
    let public_base_url = server_common::public_base_url();
    let port = env::var("PORT")
        .ok()
        .and_then(|port| port.parse::<u16>().ok())
        .unwrap_or(port);
    let config = UnifiedConfig {
        listen_addr: SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
        public_base_url,
        data_dir,
        origin,
        password_manager_origin,
        seed,
    };
    with_shutdown(|shutdown| serve(config, shutdown)).await
}

async fn with_shutdown<F: Future<Output = io::Result<()>>>(
    run: impl FnOnce(CancellationToken) -> F,
) -> io::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let cancellation = CancellationToken::new();
    let signal = spawn(shutdown_signal(cancellation.clone()));
    let result = run(cancellation).await;
    signal.abort();
    result
}
