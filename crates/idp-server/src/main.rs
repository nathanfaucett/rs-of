#[tokio::main]
async fn main() -> std::io::Result<()> {
    idp_server::cli::run::run().await
}
