use std::sync::Arc;

use db::Database;
use iroh::{SecretKey, endpoint::presets::Minimal};
use iroh_chain::{EndpointIdStore, Server};
use tokio_util::sync::CancellationToken;

use crate::{security_policy::SecurityContext, security_protocol::SecurityProtocol};

#[tokio::test]
async fn owner_rejects_unreviewed_public_schema() {
    let idp = Arc::new(Database::in_memory());
    idp_model::replica::up(&idp).await.expect("identity schema");
    let devices = Arc::new(Database::in_memory());
    crate::replica::up(&devices).await.expect("Device schema");
    let server =
        Server::bind_with_secret_key(Minimal, SecretKey::generate(), EndpointIdStore::default())
            .await
            .expect("local endpoint");
    let protocol = SecurityProtocol::new(
        server.clone(),
        idp.clone(),
        devices.clone(),
        idp,
        "https://installation.example".into(),
        None,
        std::env::temp_dir(),
        CancellationToken::new(),
    );
    protocol
        .validate_local_schema(SecurityContext::Device)
        .await
        .expect("reviewed owner schemas");
    devices
        .client()
        .translate_and_execute(
            "ALTER TABLE devices ADD COLUMN unreviewed TEXT",
            &db::SqlTranslator,
        )
        .await
        .expect("unreviewed column");
    let error = protocol
        .validate_local_schema(SecurityContext::Device)
        .await
        .expect_err("unknown fields must not synchronize");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    server.endpoint().close().await;
}

#[test]
fn bootstrap_requires_the_exact_shared_endpoint_before_opening_owners() {
    let endpoint = SecretKey::generate().public();
    let mut binding = idp_service::oauth2::NodeSignerBinding {
        member_id: idp_model::model::Id::now_v7(),
        key_id: idp_model::model::Id::now_v7(),
        endpoint_id: endpoint.to_string(),
    };
    crate::node_join_owner::check_bootstrap_endpoint(endpoint, &binding)
        .expect("exact endpoint binding");
    binding.endpoint_id = SecretKey::generate().public().to_string();
    let error = crate::node_join_owner::check_bootstrap_endpoint(endpoint, &binding)
        .expect_err("another endpoint cannot use this signer binding");
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
}

struct RetainedAdmission {
    peer: iroh::EndpointId,
    state: Arc<std::sync::atomic::AtomicU8>,
}
impl crate::join_admission::RetainedJoinAdmission for RetainedAdmission {
    fn pinned_join_peer(
        &self,
        _: iroh::EndpointId,
        approved: bool,
    ) -> std::io::Result<Option<iroh::EndpointId>> {
        let state = self.state.load(std::sync::atomic::Ordering::SeqCst);
        if state == 3 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "malformed retained state",
            ));
        }
        Ok((state > 0 && (!approved || state == 2)).then_some(self.peer))
    }
}

async fn admitted(client: &iroh::Endpoint, server: &iroh::Endpoint, alpn: &[u8]) -> bool {
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        let (outgoing, incoming) = tokio::join!(client.connect(server.addr(), alpn), async {
            server.accept().await.expect("incoming probe").await
        });
        let (Ok(outgoing), Ok(incoming)) = (outgoing, incoming) else {
            return false;
        };
        let Ok(mut send) = outgoing.open_uni().await else {
            return false;
        };
        if send.write_all(b"admitted").await.is_err() || send.finish().is_err() {
            return false;
        }
        let Ok(mut recv) = incoming.accept_uni().await else {
            return false;
        };
        let admitted = recv
            .read_to_end(8)
            .await
            .is_ok_and(|bytes| bytes == b"admitted");
        outgoing.close(0u32.into(), b"probe complete");
        admitted
    })
    .await
    .unwrap_or(false)
}

#[tokio::test]
async fn retained_join_admission_is_purpose_bound_and_rechecks_learned_state() {
    use iroh_chain::DATA_ALPN;
    use std::sync::atomic::{AtomicU8, Ordering};
    let peer_secret = SecretKey::generate();
    let state = Arc::new(AtomicU8::new(1));
    let allowed = EndpointIdStore::default();
    let joining = crate::join_admission::bind_node_join_endpoint(
        Minimal,
        SecretKey::generate(),
        allowed.clone(),
        RetainedAdmission {
            peer: peer_secret.public(),
            state: state.clone(),
        },
    )
    .await
    .expect("Device admission owner");
    let peers = EndpointIdStore::default();
    peers.add(joining.endpoint().id());
    let peer = Server::bind_with_secret_key(Minimal, peer_secret, peers)
        .await
        .expect("trusted peer endpoint");
    let bootstrap = crate::security_protocol::BOOTSTRAP_ALPN;
    for endpoint in [joining.endpoint(), peer.endpoint()] {
        endpoint.set_alpns(vec![
            iroh_chain::PAIRING_ALPN.to_vec(),
            bootstrap.to_vec(),
            DATA_ALPN.to_vec(),
        ]);
    }
    assert!(
        admitted(
            joining.endpoint(),
            peer.endpoint(),
            iroh_chain::PAIRING_ALPN
        )
        .await
    );
    assert!(!admitted(joining.endpoint(), peer.endpoint(), bootstrap).await);
    state.store(2, Ordering::SeqCst);
    assert!(admitted(joining.endpoint(), peer.endpoint(), bootstrap).await);
    assert!(!admitted(joining.endpoint(), peer.endpoint(), DATA_ALPN).await);
    assert!(
        !joining.is_allowed(peer.endpoint().id()).await,
        "purpose-bound admission never grants ordinary trust"
    );
    for learned in [0, 3] {
        state.store(learned, Ordering::SeqCst);
        assert!(
            !admitted(joining.endpoint(), peer.endpoint(), bootstrap).await,
            "missing or malformed current evidence denies"
        );
    }
    allowed.add(peer.endpoint().id());
    assert!(
        admitted(joining.endpoint(), peer.endpoint(), DATA_ALPN).await,
        "ordinary membership remains with the existing allowlist owner"
    );
    joining.endpoint().close().await;
    peer.endpoint().close().await;
}
