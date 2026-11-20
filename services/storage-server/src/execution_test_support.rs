use std::{path::PathBuf, sync::Arc};

use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};
use device_service::device_repo::DeviceRepo;
use device_service::ofdb::device_repo::DbDeviceRepo;
use device_service::ofdb::selection_policy::DbSelectionPolicyRepo;
use idp_service::{
    password_config::PasswordConfig,
    replica::{DbReplicaSignerRepo, DbUserRepo},
    repo::user_repo::UserRepo,
};
use iroh::{
    Endpoint, EndpointId,
    address_lookup::MemoryLookup,
    endpoint::{Connection, presets},
};
use of_model::contract::replication::SelectedResource;
use of_model::device::selection_policy::SelectionPolicy;
use of_model::id::Id;
use of_model::idp::contract::idp_signer::IdpSignerRecord;
use of_model::idp::contract::idp_signer::ReplicaMembership;
use of_model::idp::contract::jwk_public::JwkPublic;
use of_model::idp::contract::jwk_public_parameters::JwkPublicParameters;
use of_model::idp::contract::jws_algorithm::JwsAlgorithm;
use of_model::idp::contract::key_use::KeyUse;
use ofdb_sql::Database;
use ofdb_sql::SqlTranslator;
use tokio::sync::Notify;

use crate::local_replication_authorizer::{DeviceReplicationPolicy, LocalReplicationAuthorizer};

pub struct TestDirectory(pub PathBuf);
impl TestDirectory {
    pub fn new() -> Self {
        let path = std::env::temp_dir().join(format!("of03-domain-{}", Id::now_v7()));
        std::fs::create_dir_all(&path).expect("isolated Node stores");
        Self(path)
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).expect("remove isolated Node stores");
    }
}

pub struct IrohPair {
    pub endpoints: [Endpoint; 2],
    pub connections: [Connection; 2],
}
impl IrohPair {
    pub async fn new() -> Self {
        let lookup = MemoryLookup::new();
        let left = Endpoint::builder(presets::Minimal)
            .alpns(vec![iroh_chain::DATA_ALPN.to_vec()])
            .address_lookup(lookup.clone())
            .bind()
            .await
            .expect("left endpoint");
        let right = Endpoint::builder(presets::Minimal)
            .alpns(vec![iroh_chain::DATA_ALPN.to_vec()])
            .address_lookup(lookup.clone())
            .bind()
            .await
            .expect("right endpoint");
        lookup.add_endpoint_info(right.addr());
        let (outgoing, incoming) =
            tokio::join!(left.connect(right.id(), iroh_chain::DATA_ALPN), async {
                right
                    .accept()
                    .await
                    .expect("incoming")
                    .accept()
                    .expect("accept")
                    .await
            });
        let connections = [outgoing.expect("connect"), incoming.expect("authenticate")];
        assert_eq!(connections[0].remote_id(), right.id());
        assert_eq!(connections[1].remote_id(), left.id());
        Self {
            endpoints: [left, right],
            connections,
        }
    }
    pub fn ids(&self) -> [EndpointId; 2] {
        [self.endpoints[0].id(), self.endpoints[1].id()]
    }
    pub async fn streams(&self) -> [db::database_transport::DatabaseTransport; 2] {
        let (mut send, recv) = self.connections[0]
            .open_bi()
            .await
            .expect("fresh stream on existing connection");
        send.write_all(&[1]).await.expect("stream marker");
        let (incoming_send, mut incoming_recv) = self.connections[1]
            .accept_bi()
            .await
            .expect("accept stream");
        incoming_recv.read_exact(&mut [0]).await.expect("marker");
        [
            db::database_transport::DatabaseTransport::new(send, recv),
            db::database_transport::DatabaseTransport::new(incoming_send, incoming_recv),
        ]
    }
    pub async fn reconnect(&mut self) {
        self.connections[0].close(0u32.into(), b"reconnect");
        self.connections[1].close(0u32.into(), b"reconnect");
        let (outgoing, incoming) = tokio::join!(
            self.endpoints[0].connect(self.endpoints[1].id(), iroh_chain::DATA_ALPN),
            async {
                self.endpoints[1]
                    .accept()
                    .await
                    .expect("reconnecting peer")
                    .accept()
                    .expect("accept reconnect")
                    .await
            }
        );
        self.connections = [
            outgoing.expect("reconnect"),
            incoming.expect("authenticate reconnect"),
        ];
        assert_eq!(self.connections[0].remote_id(), self.endpoints[1].id());
        assert_eq!(self.connections[1].remote_id(), self.endpoints[0].id());
    }
    pub async fn close(&self) {
        self.connections[0].close(0u32.into(), b"test complete");
        self.connections[1].close(0u32.into(), b"test complete");
        self.endpoints[0].close().await;
        self.endpoints[1].close().await;
    }
}

#[derive(Clone, Default)]
pub struct Gate {
    reached: Arc<Notify>,
    resume: Arc<Notify>,
}
impl Gate {
    pub async fn pause(&self) {
        self.reached.notify_one();
        self.resume.notified().await;
    }
    pub async fn reached(&self) {
        self.reached.notified().await;
    }
    pub fn release(&self) {
        self.resume.notify_one();
    }
}

const INSTALLATION: &str = "https://of03-installation.example";

fn approved_signers(devices: [Id; 2]) -> [IdpSignerRecord; 2] {
    devices.map(|member_id| {
        let entropy = key::master_key::MasterKey::entropy().expect("independent signer entropy");
        let master = key::master_key::MasterKey::from_entropy(entropy.as_ref())
            .expect("independent private signer");
        let (x, y) = master
            .derive_from_index_and_hardened(0, false)
            .expect("signer derivation")
            .to_xy()
            .expect("public signer point");
        let key_id = Id::now_v7();
        IdpSignerRecord {
            member_id,
            key_id,
            issuer: INSTALLATION.into(),
            public_jwk: JwkPublic {
                kid: key_id.to_string(),
                r#use: KeyUse::Signature,
                alg: JwsAlgorithm::ES256K,
                params: JwkPublicParameters::Ec {
                    crv: "secp256k1".into(),
                    x: STANDARD_NO_PAD.encode(x),
                    y: STANDARD_NO_PAD.encode(y),
                },
            },
            approved_at: Some(1),
            revoked_at: None,
            expires_at: None,
        }
    })
}

pub struct DomainNode {
    pub idp: Arc<Database>,
    pub management: Arc<Database>,
    pub root: PathBuf,
    pub owner: Id,
    pub devices: [Id; 2],
}
impl DomainNode {
    pub async fn pair(root: &std::path::Path, endpoints: [EndpointId; 2]) -> [Self; 2] {
        let left = root.join("left");
        let right = root.join("right");
        let (idp, management) = Self::open_stores(&left);
        db::ofdb::up(&idp.client()).await.expect("IdP schema");
        device_service::ofdb::schema::up(&management)
            .await
            .expect("Device schema");
        let owner = insert_owner(&idp).await;
        let devices = [
            insert_device(&management, owner, endpoints[0]).await,
            insert_device(&management, owner, endpoints[1]).await,
        ];
        let signers = approved_signers(devices);
        for ((device, endpoint), signer) in devices.into_iter().zip(endpoints).zip(signers) {
            DbReplicaSignerRepo::new(idp.client())
                .enroll(
                    &ReplicaMembership {
                        installation_id: INSTALLATION.into(),
                        member_id: device,
                        endpoint_id: endpoint.to_string(),
                        issuer: INSTALLATION.into(),
                        approved_at: 1,
                        revoked_at: None,
                    },
                    &signer,
                )
                .await
                .expect("owner-approved Installation/Device/signer binding");
        }
        let (peer_idp, peer_management) = Self::open_stores(&right);
        for (source, peer) in [(&idp, &peer_idp), (&management, &peer_management)] {
            peer.import_history(source.export_history().await.expect("bootstrap history"))
                .await
                .expect("same Installation state on the peer");
        }
        [
            Self {
                idp,
                management,
                root: left,
                owner,
                devices,
            },
            Self {
                idp: peer_idp,
                management: peer_management,
                root: right,
                owner,
                devices,
            },
        ]
    }

    fn open_stores(root: &std::path::Path) -> (Arc<Database>, Arc<Database>) {
        std::fs::create_dir_all(root).expect("Node root");
        (
            Arc::new(Database::open(root.join("idp.redb")).expect("IdP store")),
            Arc::new(Database::open(root.join("management.redb")).expect("Management store")),
        )
    }

    pub async fn select(&self, resource: &SelectedResource) {
        for device_id in self.devices {
            DbSelectionPolicyRepo::new(self.management.clone().client())
                .set_prevalidated(SelectionPolicy {
                    device_id,
                    owner_subject: self.owner.to_string(),
                    application_id: Some(resource.application_id.parse().expect("application ID")),
                    selected_kind: Some(resource.kind.clone()),
                    selected_id: Some(resource.resource_id.parse().expect("resource ID")),
                    admin_allowed: true,
                })
                .await
                .expect("real stable selected-resource row");
        }
    }
    pub async fn restrict(&self, device: usize, allowed: bool) {
        assert!(
            DbSelectionPolicyRepo::new(self.management.clone().client())
                .set_admin_allowed_prevalidated(
                    self.devices[device],
                    &self.owner.to_string(),
                    allowed
                )
                .await
                .expect("persist current restriction")
        );
    }
    pub async fn revoke(&self, device: usize) {
        assert!(
            DbDeviceRepo::new(self.management.clone().client())
                .revoke(&self.owner.to_string(), self.devices[device], "")
                .await
                .expect("learn Device revocation")
        );
    }
    pub async fn revoke_signer(&self, device: usize) {
        assert!(
            DbReplicaSignerRepo::new(self.idp.client())
                .revoke(self.devices[device], INSTALLATION, 2)
                .await
                .expect("persist signer/member revocation")
        );
    }
    pub fn authorizer(&self) -> LocalReplicationAuthorizer {
        self.authorizer_retaining(Arc::new(()))
    }
    pub fn authorizer_retaining(
        &self,
        retained: Arc<dyn Send + Sync>,
    ) -> LocalReplicationAuthorizer {
        let idp = self.idp.clone();
        let management = self.management.clone();
        LocalReplicationAuthorizer::new(move |endpoint: EndpointId| {
            let idp = idp.clone();
            let management = management.clone();
            let retained = retained.clone();
            async move {
                let _retained = retained;
                let device = DbDeviceRepo::new(management.clone().client())
                    .find_approved_by_public_key(&endpoint.to_string())
                    .await
                    .map_err(|error| error.to_string())?
                    .ok_or("unknown or unapproved Device")?;

                let owner_id = device.owner_subject.parse().map_err(|_| "invalid owner")?;
                let owner_active = DbUserRepo::new(idp.client(), PasswordConfig::default())
                    .find_user_by_id(owner_id)
                    .await
                    .map_err(|error| error.to_string())?
                    .is_some();
                let policies = DbSelectionPolicyRepo::new(management.client());
                let policy = policies
                    .get(device.id, &device.owner_subject, Id::nil())
                    .await
                    .map_err(|error| error.to_string())?
                    .ok_or("missing local policy")?;
                let resources = policies
                    .selected_resources_for_device(device.id)
                    .await
                    .map_err(|error| error.to_string())?
                    .into_iter()
                    .map(|resource| SelectedResource {
                        owner_subject: resource.owner_subject,
                        application_id: resource.application_id.to_string(),
                        kind: resource.kind,
                        resource_id: resource.resource_id.to_string(),
                    })
                    .collect();
                Ok(DeviceReplicationPolicy {
                    device_id: device.id,
                    endpoint_id: endpoint,
                    owner_subject: device.owner_subject,
                    state: device.state,
                    owner_active,
                    admin_allowed: policy.admin_allowed,
                    resources,
                })
            }
        })
    }
}

pub async fn insert_owner(database: &Database) -> Id {
    let owner = Id::now_v7();
    sql(database, &format!("INSERT INTO users (id, handle, name, created_at, updated_at) VALUES (CAST('{owner}' AS UUID), 'owner', 'Owner', 1, 1)")).await;
    owner
}

pub async fn insert_device(database: &Database, owner: Id, endpoint: EndpointId) -> Id {
    insert_identity(database, &format!("INSERT INTO devices (owner_subject, name, public_key, address, state, approved_at, created_at, updated_at) VALUES ('{owner}', 'node', '{endpoint}', 'local', 1, 1, 1, 1) RETURNING id")).await
}

async fn insert_identity(database: &Database, sql: &str) -> Id {
    let results = database
        .client()
        .execute_sql(sql, None)
        .await
        .expect("generated fixture identity");
    ofdb_sql::decode(&results[0].rows[0].values[0], "id").expect("UUID identity")
}

pub async fn sql(database: &Database, sql: &str) {
    database
        .client()
        .translate_and_execute(sql, &SqlTranslator)
        .await
        .expect("public SQL fixture");
}

pub fn read_http_request(stream: &mut std::net::TcpStream) {
    let mut headers = Vec::new();
    while !headers.ends_with(b"\r\n\r\n") {
        let mut byte = [0];
        std::io::Read::read_exact(stream, &mut byte).expect("complete fixture request header");
        headers.push(byte[0]);
        assert!(headers.len() <= 16 * 1024, "bounded fixture headers");
    }
    let headers = String::from_utf8(headers).expect("fixture HTTP headers");
    let length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length").then(|| {
                value
                    .trim()
                    .parse::<usize>()
                    .expect("fixture content length")
            })
        })
        .unwrap_or(0);
    assert!(length <= 16 * 1024, "bounded fixture body");
    std::io::Read::read_exact(stream, &mut vec![0; length]).expect("complete fixture request body");
}
