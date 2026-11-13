use std::{
    io,
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
};

use clap::{Parser, Subcommand};
use tokio_util::sync::CancellationToken;

use crate::{SeedConfig, TlsConfig, UnifiedConfig, serve};

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
        #[arg(long, default_value = "http://localhost:5173")]
        origin: String,
        #[arg(long)]
        no_tls: bool,
        #[arg(
            long,
            help = "JSON file with additional users: {\"users\":[{\"username\":\"test\",\"password\":\"secret\"}]}"
        )]
        seed: Option<PathBuf>,
    },
    #[cfg(feature = "completions")]
    Completions { shell: clap_complete::Shell },
}

pub async fn run() -> io::Result<()> {
    let args = Args::parse();
    let (data_dir, port, origin, no_tls, seed) = match args.command {
        Command::Local {
            data_dir,
            port,
            origin,
            no_tls,
            seed,
        } => (data_dir, port, origin, no_tls, seed),
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
            serde_json::from_slice(&std::fs::read(path)?).map_err(io::Error::other)
        })
        .transpose()?;
    let config = UnifiedConfig {
        listen_addr: SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
        data_dir,
        origin,
        tls: if no_tls {
            TlsConfig::Disabled
        } else {
            TlsConfig::AutoGenerate
        },
        seed,
    };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cancellation = CancellationToken::new();
    let signal = tokio::spawn(cli::shutdown_signal(cancellation.clone()));
    let result = serve(config, cancellation).await;
    signal.abort();
    result
}
