use std::{fs, path::PathBuf, time::Duration};

use device_service::hosted_join::{NodeJoinApprovalExt, NodeJoinRequestExt};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use idp_service::{
    oauth2::node_signer::NodeSignerBinding,
    repo::{
        private_key_keyring_repo::PrivateKeyKeyringRepo, private_key_repo::PrivateKeyRepo,
        raw_keyring_repo::RawKeyringRepo,
    },
};
use iroh::{Endpoint, SecretKey, endpoint::presets::Minimal};
use iroh_chain::{DATA_ALPN, EndpointIdStore, MESH_ALPN, PAIRING_ALPN, Server};
use of_model::id::Id;

use of_model::node_join::NodeJoinApproval;
use of_model::node_join::NodeJoinRequest;

use crate::config::app_config::AppConfig;

use crate::bootstrap::BOOTSTRAP_ALPN;

use crate::provisioning::OwnerProvisioner;

use super::bind_node_join_endpoint;

struct Fixture {
    root: PathBuf,
    config: AppConfig,
    owner: OwnerProvisioner,
    binding: NodeSignerBinding,
    request: NodeJoinRequest,
    joining: Server,
    peer: Server,
    peer_secret: SecretKey,
}

impl Fixture {
    async fn new() -> Self {
        let root = std::env::temp_dir().join(format!("node-join-transport-{}", Id::now_v7()));
        let mut config = AppConfig {
            data_dir: root.to_string_lossy().into_owned(),
            ..Default::default()
        };
        config.oauth2.issuer = format!("https://join-transport-{}.test/idp", Id::now_v7());
        let owner = OwnerProvisioner::open(&config)
            .await
            .expect("OS keyring owner");
        owner
            .prepare_fresh_setup_secret_inventory(Id::now_v7(), &[])
            .await
            .expect("fresh local preparation");
        owner
            .prepare_setup_client_slots(&[])
            .await
            .expect("ready inventory");
        let secret = SecretKey::generate();
        let binding = NodeSignerBinding {
            member_id: Id::now_v7(),
            key_id: Id::now_v7(),
            endpoint_id: secret.public().to_string(),
        };
        let peer_secret = SecretKey::generate();
        let request = owner
            .prepare_node_join(&binding, "Joining Node", peer_secret.public())
            .expect("persist prepared request and independent signer");
        let allowed = EndpointIdStore::default();
        allowed.add(secret.public());
        let joining = bind_node_join_endpoint(&config, Minimal, secret, allowed)
            .await
            .expect("real domain joining endpoint");
        let peer_allowed = EndpointIdStore::default();
        peer_allowed.add(joining.endpoint().id());
        let peer = Server::bind_with_secret_key(Minimal, peer_secret.clone(), peer_allowed)
            .await
            .expect("hooked approving peer");
        Self {
            root,
            config,
            owner,
            binding,
            request,
            joining,
            peer,
            peer_secret,
        }
    }

    fn approval(&self) -> NodeJoinApproval {
        let owner_subject = Id::now_v7();
        let device_id = Id::now_v7();
        NodeJoinApproval {
            device_id,
            request: self.request.clone(),
            owner_subject,
            signature: URL_SAFE_NO_PAD.encode(
                self.peer_secret
                    .sign(
                        &NodeJoinApproval::payload(&self.request, device_id, owner_subject)
                            .expect("approval payload"),
                    )
                    .to_bytes(),
            ),
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        PrivateKeyKeyringRepo::new(&self.config.oauth2.issuer)
            .expect("OS keyring")
            .delete(
                &format!(
                    "{}:node:{}:{}",
                    self.config.key_namespace, self.binding.member_id, self.binding.key_id
                ),
                &bip32::DerivationPath::default(),
            )
            .expect("remove fixture private signer");
        fs::remove_dir_all(&self.root).expect("remove isolated public owner state");
    }
}

async fn handshake_admitted(client: &Endpoint, server: &Endpoint, alpn: &[u8]) -> bool {
    tokio::time::timeout(Duration::from_secs(15), async {
        let (outgoing, incoming) = tokio::join!(client.connect(server.addr(), alpn), async {
            server.accept().await.expect("incoming connection").await
        },);
        let Ok(incoming) = incoming else {
            return false;
        };
        let outgoing = outgoing.expect("outgoing handshake");
        let mut send = outgoing.open_uni().await.expect("authenticated stream");
        send.write_all(b"admitted").await.expect("write probe");
        send.finish().expect("finish probe");
        let mut recv = incoming.accept_uni().await.expect("admitted stream");
        assert_eq!(recv.read_to_end(8).await.expect("read probe"), b"admitted");
        outgoing.close(0u32.into(), b"probe complete");
        true
    })
    .await
    .expect("bounded hook handshake")
}

fn join_alpns(endpoint: &Endpoint) {
    endpoint.set_alpns(vec![
        PAIRING_ALPN.to_vec(),
        BOOTSTRAP_ALPN.to_vec(),
        DATA_ALPN.to_vec(),
        MESH_ALPN.to_vec(),
        b"other/1".to_vec(),
    ]);
}

#[tokio::test]
#[ignore = "requires an unlocked OS keyring; run explicitly in isolation"]
async fn prepared_join_can_send_pairing_without_global_pairing_or_peer_trust() {
    let fixture = Fixture::new().await;
    assert!(!fixture.joining.pairing_enabled());
    assert!(
        !fixture
            .joining
            .is_allowed(fixture.peer.endpoint().id())
            .await
    );
    fixture.peer.set_pairing_enabled(true);
    let mut offers = fixture.peer.subscribe_pairing_offers();
    let router = fixture
        .peer
        .router(crate::bootstrap::handler::BootstrapProtocolHandler);
    let approval = fixture.approval();
    let expected = fixture.request.clone();
    let response = tokio::spawn(async move {
        let offer = offers.recv().await.expect("real pairing offer");
        assert_eq!(
            serde_json::from_slice::<NodeJoinRequest>(&offer.payload).expect("request"),
            expected
        );
        offer
            .reply(&serde_json::to_vec(&approval).expect("public approval"))
            .await
            .expect("reply");
        offer
    });
    let received = tokio::time::timeout(
        Duration::from_secs(15),
        fixture
            .request
            .send(&fixture.joining, fixture.peer.endpoint().addr()),
    )
    .await
    .expect("bounded real pairing")
    .expect("prepared join pairing must be admitted before approval");
    let _offer = response.await.expect("offer exchange");
    received
        .verify(&fixture.request)
        .expect("verified matching approval");
    assert!(
        fixture
            .owner
            .node_join_approval()
            .expect("approval state")
            .is_none()
    );
    for alpn in [BOOTSTRAP_ALPN, DATA_ALPN, MESH_ALPN, b"other/1"] {
        assert!(
            fixture
                .joining
                .endpoint()
                .connect(fixture.peer.endpoint().addr(), alpn)
                .await
                .is_err(),
            "preapproval must not admit {alpn:?}"
        );
    }
    let other = Server::bind(Minimal, EndpointIdStore::default())
        .await
        .expect("hooked other peer");
    assert!(
        fixture
            .joining
            .endpoint()
            .connect(other.endpoint().addr(), PAIRING_ALPN)
            .await
            .is_err()
    );
    assert!(!fixture.joining.pairing_enabled());
    assert!(
        !fixture
            .joining
            .is_allowed(fixture.peer.endpoint().id())
            .await
    );
    other.endpoint().close().await;
    fixture.joining.endpoint().close().await;
    router.shutdown().await.expect("stop approving peer");
}

#[tokio::test]
#[ignore = "requires an unlocked OS keyring; run explicitly in isolation"]
async fn incoming_join_admission_is_pinned_and_bootstrap_requires_matching_verified_approval() {
    let fixture = Fixture::new().await;
    join_alpns(fixture.joining.endpoint());
    join_alpns(fixture.peer.endpoint());
    assert!(
        handshake_admitted(
            fixture.peer.endpoint(),
            fixture.joining.endpoint(),
            PAIRING_ALPN
        )
        .await
    );
    for alpn in [BOOTSTRAP_ALPN, DATA_ALPN, MESH_ALPN, b"other/1"] {
        assert!(
            !handshake_admitted(fixture.peer.endpoint(), fixture.joining.endpoint(), alpn).await,
            "incoming preapproval must not admit {alpn:?}"
        );
    }
    let allowed = EndpointIdStore::default();
    allowed.add(fixture.joining.endpoint().id());
    let other = Server::bind(Minimal, allowed)
        .await
        .expect("hooked unpinned peer");
    assert!(!handshake_admitted(other.endpoint(), fixture.joining.endpoint(), PAIRING_ALPN).await);
    let approval_path = fixture.root.join("node-join-approval.json");
    let approval = fixture.approval();
    let mut forged = approval.clone();
    forged.signature = URL_SAFE_NO_PAD.encode(
        SecretKey::generate()
            .sign(
                &NodeJoinApproval::payload(
                    &fixture.request,
                    forged.device_id,
                    forged.owner_subject,
                )
                .expect("payload"),
            )
            .to_bytes(),
    );
    let mut mismatch = approval.clone();
    mismatch.request.name = "Another Node".into();
    mismatch.signature = URL_SAFE_NO_PAD.encode(
        fixture
            .peer_secret
            .sign(
                &NodeJoinApproval::payload(
                    &mismatch.request,
                    mismatch.device_id,
                    mismatch.owner_subject,
                )
                .expect("payload"),
            )
            .to_bytes(),
    );
    for invalid in [
        b"malformed".to_vec(),
        serde_json::to_vec(&forged).expect("forged approval"),
        serde_json::to_vec(&mismatch).expect("mismatched approval"),
    ] {
        fs::write(&approval_path, invalid).expect("inject invalid public approval");
        assert!(
            fixture
                .joining
                .endpoint()
                .connect(fixture.peer.endpoint().addr(), BOOTSTRAP_ALPN)
                .await
                .is_err()
        );
        assert!(
            !handshake_admitted(
                fixture.peer.endpoint(),
                fixture.joining.endpoint(),
                BOOTSTRAP_ALPN
            )
            .await
        );
    }
    fs::remove_file(&approval_path).expect("remove invalid approval");
    fixture
        .owner
        .record_node_join_approval(&approval)
        .expect("persist verified exact approval");
    assert!(
        handshake_admitted(
            fixture.joining.endpoint(),
            fixture.peer.endpoint(),
            BOOTSTRAP_ALPN
        )
        .await
    );
    assert!(
        handshake_admitted(
            fixture.peer.endpoint(),
            fixture.joining.endpoint(),
            BOOTSTRAP_ALPN
        )
        .await
    );
    assert!(
        !handshake_admitted(other.endpoint(), fixture.joining.endpoint(), BOOTSTRAP_ALPN).await
    );
    for alpn in [DATA_ALPN, MESH_ALPN, b"other/1"] {
        assert!(
            fixture
                .joining
                .endpoint()
                .connect(fixture.peer.endpoint().addr(), alpn)
                .await
                .is_err()
        );
        assert!(
            !handshake_admitted(fixture.peer.endpoint(), fixture.joining.endpoint(), alpn).await
        );
    }
    assert!(!fixture.joining.pairing_enabled());
    assert!(
        !fixture
            .joining
            .is_allowed(fixture.peer.endpoint().id())
            .await
    );
    other.endpoint().close().await;
    fixture.joining.endpoint().close().await;
    fixture.peer.endpoint().close().await;
}
