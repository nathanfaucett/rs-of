#![recursion_limit = "256"]

use std::io;

use unified_server::cli;

#[tokio::main]
async fn main() -> io::Result<()> {
    cli::run().await
}
