#[tokio::main]
async fn main() -> std::io::Result<()> {
    device_server::run().await
}
