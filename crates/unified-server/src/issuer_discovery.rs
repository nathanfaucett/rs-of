use std::{io, time::Duration};

use idp_model::model::Id;
#[cfg(not(test))]
use iroh::endpoint::presets;
use iroh::{EndpointId, SecretKey};
use iroh_chain::{EndpointIdStore, PairingOffer, Server};
use serde::{Deserialize, Serialize};

const PURPOSE: &str = "of.equal-node-issuer-discovery.v1";

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Request {
    purpose: String,
    nonce: Id,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Response {
    purpose: String,
    nonce: Id,
    endpoint: String,
    issuer: String,
}

/// Learns only public Installation identity over pairing transport authenticated to the pinned peer.
/// This does not approve the joining Node or trust a peer discovered from an unsigned address/URL.
pub async fn discover_installation_issuer(peer: &str) -> io::Result<String> {
    discover_with_cancellation(peer, &tokio_util::sync::CancellationToken::new()).await
}

pub(crate) async fn discover_with_cancellation(
    peer: &str,
    cancellation: &tokio_util::sync::CancellationToken,
) -> io::Result<String> {
    idp_server::NodeJoinRequest::validate_trusted_peer(peer)?;
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
    let request = Request {
        purpose: PURPOSE.into(),
        nonce: Id::now_v7(),
    };
    let exchange = async {
        let reply = tokio::time::timeout(Duration::from_secs(30), server.send_pairing_offer(peer_id, &serde_json::to_vec(&request)?))
            .await.map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "trusted peer issuer discovery timed out; enable join acceptance and open its authorized approval interface"))?
            .map_err(io::Error::other)?;
        decode(&request, peer, &reply)
    };
    let result = tokio::select! {
        biased;
        () = cancellation.cancelled() => Err(io::Error::new(io::ErrorKind::Interrupted, "issuer discovery cancelled")),
        result = exchange => result,
    };
    server.endpoint().close().await;
    result
}

pub(crate) fn is_issuer_discovery(offer: &PairingOffer) -> bool {
    serde_json::from_slice::<Request>(&offer.payload)
        .is_ok_and(|request| request.purpose == PURPOSE && !request.nonce.is_nil())
}

pub(crate) async fn answer_issuer_discovery(
    offer: &PairingOffer,
    endpoint: &str,
    issuer: &str,
) -> io::Result<bool> {
    let Ok(request) = serde_json::from_slice::<Request>(&offer.payload) else {
        return Ok(false);
    };
    if request.purpose != PURPOSE || request.nonce.is_nil() {
        return Ok(false);
    }
    crate::bootstrap_config::validate_issuer(issuer)?;
    let response = Response {
        purpose: PURPOSE.into(),
        nonce: request.nonce,
        endpoint: endpoint.into(),
        issuer: issuer.into(),
    };
    offer
        .reply(&serde_json::to_vec(&response)?)
        .await
        .map_err(io::Error::other)?;
    Ok(true)
}
fn decode(request: &Request, peer: &str, reply: &[u8]) -> io::Result<String> {
    let response: Response = serde_json::from_slice(reply)
        .map_err(|_| io::Error::other("invalid purpose-bound issuer response"))?;
    if response.purpose != PURPOSE || response.nonce != request.nonce || response.endpoint != peer {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "issuer response does not bind the requested purpose, nonce and authenticated peer",
        ));
    }
    crate::bootstrap_config::validate_issuer(&response.issuer)?;
    Ok(response.issuer)
}

#[cfg(test)]
mod tests {
    use super::{PURPOSE, Request, Response, decode};
    use idp_model::model::Id;
    #[test]
    fn issuer_response_requires_exact_nonce_peer_and_purpose() {
        let request = Request {
            purpose: PURPOSE.into(),
            nonce: Id::now_v7(),
        };
        let mut response = Response {
            purpose: PURPOSE.into(),
            nonce: request.nonce,
            endpoint: "pinned-peer".into(),
            issuer: "https://installation.test/idp".into(),
        };
        assert_eq!(
            decode(
                &request,
                "pinned-peer",
                &serde_json::to_vec(&response).expect("response")
            )
            .expect("exact authenticated response"),
            "https://installation.test/idp"
        );
        response.nonce = Id::now_v7();
        assert!(
            decode(
                &request,
                "pinned-peer",
                &serde_json::to_vec(&response).expect("response")
            )
            .is_err()
        );
        response.nonce = request.nonce;
        assert!(
            decode(
                &request,
                "wrong-peer",
                &serde_json::to_vec(&response).expect("response")
            )
            .is_err()
        );
        response.purpose = "unrelated-purpose".into();
        assert!(
            decode(
                &request,
                "pinned-peer",
                &serde_json::to_vec(&response).expect("response")
            )
            .is_err()
        );
    }
}
