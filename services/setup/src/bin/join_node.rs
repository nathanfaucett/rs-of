use std::io;

#[cfg(unix)]
#[tokio::main(flavor = "current_thread")]
async fn main() -> io::Result<()> {
    use std::{env, net::Ipv4Addr, path::PathBuf};

    use setup::join::JoinHost;

    let [operation, root, port, origin]: [String; 4] = env::args()
        .skip(1)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_error| {
            io::Error::other(
                "usage: join-node <init|serve> <new-or-retained-root> <port> <HTTPS-app-origin>",
            )
        })?;
    let root = PathBuf::from(root);
    let address = (Ipv4Addr::LOCALHOST, port.parse().map_err(io::Error::other)?).into();
    match operation.as_str() {
        "init" => {
            JoinHost::initialize(&root, address, &origin)?;
            println!(
                "Setup code file: {}",
                root.join("admission/state").display()
            );
            println!(
                "Review and trust the join helper CA: {}",
                root.join("tls/ca.pem").display()
            );
            Ok(())
        }
        "serve" => JoinHost::open(&root, address, &origin)?.serve().await,
        _ => Err(io::Error::other("expected init or serve")),
    }
}

#[cfg(not(unix))]
fn main() -> io::Result<()> {
    Err(io::Error::other(
        "join-node requires Unix owner-only storage",
    ))
}
