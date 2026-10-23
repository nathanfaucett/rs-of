#![forbid(unsafe_code)]

use std::{io, net::SocketAddr, path::PathBuf};

use clap::Parser;

#[derive(Parser)]
struct Args {
    #[arg(long)]
    database: PathBuf,
    #[arg(long, default_value = "127.0.0.1:50052")]
    listen: SocketAddr,
    #[arg(long)]
    certificate: Option<PathBuf>,
    #[arg(long)]
    private_key: Option<PathBuf>,
    #[arg(long)]
    client_ca: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let args = Args::parse();
    let (identity, client_ca) = match (&args.certificate, &args.private_key, &args.client_ca) {
        (Some(certificate), Some(key), Some(ca)) => (
            Some((std::fs::read(certificate)?, std::fs::read(key)?)),
            Some(std::fs::read(ca)?),
        ),
        (None, None, None) if args.listen.ip().is_loopback() => (None, None),
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Device SQL requires mTLS; plaintext is allowed only on loopback",
            ));
        }
    };
    let database = db::Database::open(&args.database).map_err(io::Error::other)?;
    device_service::replica::up(&database)
        .await
        .map_err(io::Error::other)?;
    database
        .serve_tcp_with_mtls(args.listen, identity, client_ca, async {
            if let Err(error) = tokio::signal::ctrl_c().await {
                eprintln!("shutdown signal: {error}");
            }
        })
        .await
        .map_err(io::Error::other)
}
