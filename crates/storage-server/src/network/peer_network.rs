use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::Path,
    time::Duration,
};

use iroh::{EndpointId, SecretKey, endpoint::presets};
use iroh_chain::{EndpointIdStore, Server};
use tokio::{select, time::sleep};
use tokio_util::sync::CancellationToken;

use crate::ManagementClient;

const ENDPOINT_KEY_FILE: &str = "endpoint.key";
const PEER_REFRESH_INTERVAL: Duration = Duration::from_secs(10);

pub struct StoragePeerNetwork {
    server: Server,
    allowed_peers: Option<EndpointIdStore>,
    owns_endpoint: bool,
}

impl StoragePeerNetwork {
    pub async fn open(data_dir: &Path) -> io::Result<Self> {
        fs::create_dir_all(data_dir)?;
        let secret_key = load_or_create_secret_key(&data_dir.join(ENDPOINT_KEY_FILE))?;
        let endpoint_id = secret_key.public();
        let allowed_peers = EndpointIdStore::default();
        allowed_peers.replace([endpoint_id]);
        let server = Server::bind_with_secret_key(presets::N0, secret_key, allowed_peers.clone())
            .await
            .map_err(io::Error::other)?;
        Ok(Self {
            server,
            allowed_peers: Some(allowed_peers),
            owns_endpoint: true,
        })
    }

    pub fn from_host_server(server: Server) -> Self {
        Self {
            server,
            allowed_peers: None,
            owns_endpoint: false,
        }
    }

    pub fn server(&self) -> &Server {
        &self.server
    }

    pub fn endpoint_id(&self) -> EndpointId {
        self.server.endpoint().id()
    }

    pub async fn refresh_approved_peers(
        &self,
        mgmt_client: &ManagementClient,
    ) -> Result<(), String> {
        let allowed_peers = self
            .allowed_peers
            .as_ref()
            .ok_or_else(|| "peer admission is owned by the unified host".to_owned())?;
        let local_endpoint_id = self.endpoint_id();
        let endpoint_ids = match mgmt_client.approved_device_endpoints().await {
            Ok(endpoint_ids) => endpoint_ids,
            Err(_) => {
                allowed_peers.replace([local_endpoint_id]);
                return Err("Management endpoint authorization unavailable".to_owned());
            }
        };
        let mut peers = match endpoint_ids
            .iter()
            .map(|endpoint_id| {
                endpoint_id
                    .parse::<EndpointId>()
                    .map_err(|_| "Management returned an invalid endpoint ID".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(peers) => peers,
            Err(error) => {
                allowed_peers.replace([local_endpoint_id]);
                return Err(error);
            }
        };
        peers.push(local_endpoint_id);
        allowed_peers.replace(peers);
        Ok(())
    }

    pub async fn run_peer_refresh(
        &self,
        mgmt_client: ManagementClient,
        cancellation_token: CancellationToken,
    ) {
        loop {
            if let Err(error) = self.refresh_approved_peers(&mgmt_client).await {
                log::warn!("failed to refresh Storage Iroh peer allowlist: {error}");
            }
            select! {
                () = cancellation_token.cancelled() => return,
                () = sleep(PEER_REFRESH_INTERVAL) => {}
            }
        }
    }

    pub async fn close(&self) {
        if self.owns_endpoint {
            self.server.endpoint().close().await;
        }
    }
}

fn load_or_create_secret_key(path: &Path) -> io::Result<SecretKey> {
    match fs::read(path) {
        Ok(bytes) => {
            let bytes: [u8; 32] = bytes.try_into().map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "invalid Storage endpoint key")
            })?;
            Ok(SecretKey::from_bytes(&bytes))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let secret_key = SecretKey::generate();
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(path)?;
            file.write_all(&secret_key.to_bytes())?;
            file.sync_all()?;
            Ok(secret_key)
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use iroh::{SecretKey, endpoint::presets};
    use iroh_chain::{EndpointIdStore, Server};

    use super::{StoragePeerNetwork, load_or_create_secret_key};
    use crate::ManagementClient;

    #[tokio::test]
    async fn refresh_admits_management_remote_peers_and_removes_learned_denials() {
        use std::sync::{Arc, Mutex};

        use axum::{
            Json,
            extract::State,
            http::{HeaderMap, StatusCode},
            routing::{get, post},
        };

        let endpoint = iroh::Endpoint::builder(presets::Minimal)
            .bind()
            .await
            .expect("bind isolated endpoint");
        let local = endpoint.id();
        let remote = SecretKey::generate().public();
        let unknown = SecretKey::generate().public();
        let peers = EndpointIdStore::default();
        peers.replace([local]);
        let network = StoragePeerNetwork {
            server: Server::new(endpoint, peers.clone()),
            allowed_peers: Some(peers),
            owns_endpoint: true,
        };
        let evidence = Arc::new(Mutex::new((
            StatusCode::OK,
            serde_json::json!({"endpointIds": [remote.to_string()]}),
        )));
        let app = axum::Router::new()
            .route("/idp/oauth2/token", post(|| async {
                Json(serde_json::json!({
                    "access_token": "service-token", "token_type": "Bearer", "expires_in": 300,
                    "scope": "management.device.list management.replication.read management.replication.admit",
                    "iss": "https://idp.example"
                }))
            }))
            .route("/management/devices/endpoints", get(
                |State(evidence): State<Arc<Mutex<(StatusCode, serde_json::Value)>>>, headers: HeaderMap| async move {
                    assert_eq!(headers["authorization"], "Bearer service-token");
                    let (status, body) = evidence.lock().expect("current approved endpoints").clone();
                    (status, Json(body))
                }
            ))
            .with_state(evidence.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind Management API");
        let base = format!(
            "http://{}",
            listener.local_addr().expect("Management address")
        );
        let http = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("serve Management API")
        });
        let client = ManagementClient::new(
            &format!("{base}/management"),
            &format!("{base}/idp"),
            "storage-service",
            "secret",
            "https://idp.example",
            "management",
        )
        .expect("authenticated Management client");
        network
            .refresh_approved_peers(&client)
            .await
            .expect("enumerate actual approved remote peer");
        assert!(network.server().peers().contains(local));
        assert!(network.server().peers().contains(remote));
        assert!(!network.server().peers().contains(unknown));
        *evidence.lock().expect("learn revocation") =
            (StatusCode::OK, serde_json::json!({"endpointIds": []}));
        network
            .refresh_approved_peers(&client)
            .await
            .expect("learn empty approved membership");
        assert!(!network.server().peers().contains(remote));
        for (status, body) in [
            (StatusCode::FORBIDDEN, serde_json::json!({})),
            (StatusCode::SERVICE_UNAVAILABLE, serde_json::json!({})),
            (
                StatusCode::OK,
                serde_json::json!({"endpointIds": [remote.to_string(), "invalid"]}),
            ),
        ] {
            *evidence.lock().expect("restore approved peer") = (
                StatusCode::OK,
                serde_json::json!({"endpointIds": [remote.to_string()]}),
            );
            network
                .refresh_approved_peers(&client)
                .await
                .expect("restore approved peer");
            *evidence.lock().expect("learn denial") = (status, body);
            assert!(network.refresh_approved_peers(&client).await.is_err());
            assert!(!network.server().peers().contains(remote));
            assert!(network.server().peers().contains(local));
        }
        *evidence
            .lock()
            .expect("restore peer before transport failure") = (
            StatusCode::OK,
            serde_json::json!({"endpointIds": [remote.to_string()]}),
        );
        network
            .refresh_approved_peers(&client)
            .await
            .expect("restore peer before transport failure");
        http.abort();
        let _ = http.await;
        assert!(network.refresh_approved_peers(&client).await.is_err());
        assert!(!network.server().peers().contains(remote));
        assert!(network.server().peers().contains(local));
        network.close().await;
    }

    #[tokio::test]
    async fn host_owned_server_admission_is_not_replaced_by_storage_refresh() {
        let server_key = SecretKey::generate();
        let local_id = server_key.public();
        let remote_id = SecretKey::generate().public();
        let allowed_peers = EndpointIdStore::default();
        allowed_peers.replace([local_id, remote_id]);
        let server = Server::bind_with_secret_key(presets::N0, server_key, allowed_peers)
            .await
            .expect("bind host-owned Iroh server");
        let storage_network = StoragePeerNetwork::from_host_server(server.clone());
        assert_eq!(
            storage_network.server().endpoint().id(),
            server.endpoint().id()
        );
        let mgmt_client = ManagementClient::new(
            "http://127.0.0.1:3001/management",
            "http://127.0.0.1:3000/idp",
            "storage-service",
            "secret",
            "https://idp.example",
            "management",
        )
        .expect("valid Management client configuration");

        assert!(
            storage_network
                .refresh_approved_peers(&mgmt_client)
                .await
                .is_err()
        );
        assert!(server.peers().contains(remote_id));
        storage_network.close().await;
        server.endpoint().close().await;
    }

    #[test]
    fn endpoint_key_is_persisted_and_restored() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time is after Unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "storage-endpoint-key-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("create temporary key directory");
        let path = directory.join("endpoint.key");
        let original = load_or_create_secret_key(&path).expect("create endpoint key");
        let restored = load_or_create_secret_key(&path).expect("restore endpoint key");
        assert_eq!(original.public(), restored.public());
        fs::remove_dir_all(directory).expect("remove temporary key directory");
    }
}
