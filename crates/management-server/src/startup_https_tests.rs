use std::{io, net::SocketAddr, path::Path};

use api::IndependentSetupHost;

#[path = "../../api/src/setup_https_fixture.rs"]
mod fixture;

fn prepare(
    root: &Path,
    prefix: &str,
    address: SocketAddr,
) -> io::Result<Option<IndependentSetupHost>> {
    let mut config = crate::AppConfig {
        data_dir: root.join("management").to_string_lossy().into_owned(),
        ..crate::AppConfig::default()
    };
    config.server.prefix = Some(prefix.into());
    config.server.setup_ui_origin = Some("https://localhost:5173".into());
    super::prepare_setup(&config, address)
}

#[test]
fn production_owner_https_admission_uses_isolated_native_roots() {
    fixture::parent("management", prepare);
}

#[tokio::test]
#[ignore = "parent provides isolated native roots and trust command; no real OS changes"]
async fn production_owner_https_child() {
    fixture::child("management", prepare).await;
}
