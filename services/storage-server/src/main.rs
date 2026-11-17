#[tokio::main]
async fn main() -> std::io::Result<()> {
    storage_server::cli::run().await
}
