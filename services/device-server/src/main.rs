#[tokio::main]
async fn main() -> std::io::Result<()> {
    device_server::cli::run::run().await
}
