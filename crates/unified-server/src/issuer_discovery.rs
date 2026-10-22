use std::io;

#[cfg(not(test))]
use iroh::endpoint::presets;
use iroh::{EndpointId, SecretKey};
use iroh_chain::{EndpointIdStore, Server};

/// Learns only public Installation identity over pairing transport authenticated to the pinned peer.
/// This does not approve the joining Node or trust a peer discovered from an unsigned address/URL.
pub async fn discover_installation_issuer(peer: &str) -> io::Result<String> {
    discover_with_cancellation(peer, &tokio_util::sync::CancellationToken::new()).await
}

pub(crate) async fn discover_with_cancellation(
    peer: &str,
    cancellation: &tokio_util::sync::CancellationToken,
) -> io::Result<String> {
    management_service::hosted_join::NodeJoinRequest::validate_trusted_peer(peer)?;
    let peer_id: EndpointId = peer.parse().map_err(io::Error::other)?;
    let allowed = EndpointIdStore::default();
    allowed.replace([peer_id]);
    #[cfg(not(test))]
    let preset = presets::N0;
    #[cfg(test)]
    let preset = crate::endpoint::TestPreset;
    let server = Server::bind_with_secret_key(preset, SecretKey::generate(), allowed)
        .await
        .map_err(io::Error::other)?;
    let result =
        management_service::issuer_discovery::discover_on(&server, peer, cancellation).await;
    server.endpoint().close().await;
    result
}
