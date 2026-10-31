use std::{fs, sync::Arc, time::Duration};

use futures_util::{SinkExt, StreamExt};
use rustls::pki_types::{CertificateDer, pem::PemObject};
use storage_model::{StorageRequest, StorageResponse};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    Connector, MaybeTlsStream, WebSocketStream, connect_async_tls_with_config, tungstenite::Message,
};

use of_storage_client::{
    apis::{
        configuration::Configuration, resources_api as storage,
        resources_residency_api as residency,
    },
    models::{CreateResourceRequest, ResidencyMode, ResourceResidency},
};

use super::Node;

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;
const CONTENT: &[u8] = b"cross-layout retained file content";

pub(super) fn connection(
    node: &Node,
    token: &str,
    resource: &str,
) -> (reqwest::Url, Option<Connector>) {
    let mut url = reqwest::Url::parse(&node.storage).expect("explicit Storage base");
    url.set_scheme(if node.storage.starts_with("https:") {
        "wss"
    } else {
        "ws"
    })
    .expect("resource WebSocket scheme");
    url.query_pairs_mut()
        .append_pair("access_token", token)
        .append_pair("filesystem_id", resource);
    let connector = if node.storage.starts_with("https:") {
        let pem = fs::read(node.root.path().join("roots.pem")).expect("isolated native roots");
        let mut roots = rustls::RootCertStore::empty();
        for certificate in CertificateDer::pem_slice_iter(&pem) {
            roots
                .add(certificate.expect("fixture CA certificate"))
                .expect("trusted fixture CA");
        }
        Some(Connector::Rustls(Arc::new(
            rustls::ClientConfig::builder()
                .with_root_certificates(roots)
                .with_no_client_auth(),
        )))
    } else {
        None
    };
    (url, connector)
}

async fn open(node: &Node, token: &str, resource: &str) -> Socket {
    let (url, connector) = connection(node, token, resource);
    tokio::time::timeout(
        Duration::from_secs(30),
        connect_async_tls_with_config(url.as_str(), None, false, connector),
    )
    .await
    .expect("bounded Storage protocol connection")
    .unwrap_or_else(|_| panic!("authorized Storage protocol connection"))
    .0
}

async fn request(socket: &mut Socket, request: StorageRequest) -> StorageResponse {
    tokio::time::timeout(Duration::from_secs(30), async {
        socket
            .send(Message::Text(
                serde_json::to_string(&request)
                    .expect("Storage request")
                    .into(),
            ))
            .await
            .expect("Storage protocol send");
        loop {
            match socket.next().await {
                Some(Ok(Message::Text(body))) => {
                    return serde_json::from_str(&body).expect("typed Storage response");
                }
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => continue,
                _ => panic!("Storage protocol closed before response"),
            }
        }
    })
    .await
    .expect("bounded Storage protocol request")
}

pub(super) async fn retained(node: &Node, token: &str, resource: &str) {
    let mut socket = open(node, token, resource).await;
    assert_eq!(
        request(
            &mut socket,
            StorageRequest::Read {
                path: "retained.txt".into()
            }
        )
        .await,
        StorageResponse::Read {
            content: CONTENT.to_vec()
        }
    );
    let listed = request(
        &mut socket,
        StorageRequest::List {
            path: String::new(),
        },
    )
    .await;
    let StorageResponse::Listed { entries } = listed else {
        panic!("typed directory listing")
    };
    assert!(
        entries.iter().all(|entry| entry.name != "deleted.txt"),
        "file tombstone must survive restart"
    );
    socket.close(None).await.expect("close retained protocol");
}

pub(super) async fn write(node: &Node, token: &str, resource: &str) {
    let mut socket = open(node, token, resource).await;
    assert!(matches!(
        request(
            &mut socket,
            StorageRequest::Write {
                path: "retained.txt".into(),
                content: CONTENT.to_vec(),
            }
        )
        .await,
        StorageResponse::Written { .. }
    ));
    socket.close(None).await.expect("close write protocol");
    retained(node, token, resource).await;
}

pub(super) async fn qualify(node: &mut Node, config: &Configuration, resource: &str) {
    let token = config
        .bearer_access_token
        .as_deref()
        .expect("Storage authority");
    write(node, token, resource).await;
    let disposable = storage::create_file_system(
        config,
        storage::CreateFileSystemParams {
            create_resource_request: CreateResourceRequest {
                name: Some(Some("Tombstoned resource".into())),
            },
        },
    )
    .await
    .expect("create disposable resource");
    for mode in [ResidencyMode::Passthrough, ResidencyMode::Full] {
        residency::put_file_system_residency(
            config,
            residency::PutFileSystemResidencyParams {
                filesystem_id: disposable.id.clone(),
                resource_residency: ResourceResidency::new(mode),
            },
        )
        .await
        .expect("change local residency");
        assert_eq!(
            residency::get_file_system_residency(
                config,
                residency::GetFileSystemResidencyParams {
                    filesystem_id: disposable.id.clone(),
                }
            )
            .await
            .expect("observe local residency")
            .mode,
            mode
        );
        let mut socket = open(node, token, &disposable.id).await;
        let result = request(
            &mut socket,
            StorageRequest::Write {
                path: "residency.txt".into(),
                content: b"residency effect".to_vec(),
            },
        )
        .await;
        match mode {
            ResidencyMode::Passthrough => assert!(
                matches!(result, StorageResponse::Error { .. }),
                "Passthrough must not permit writes"
            ),
            ResidencyMode::Full => assert!(
                matches!(result, StorageResponse::Written { .. }),
                "Full must restore writes"
            ),
        }
        socket.close(None).await.expect("close residency protocol");
    }
    write(node, token, resource).await;
    let mut socket = open(node, token, resource).await;
    assert!(matches!(
        request(
            &mut socket,
            StorageRequest::Write {
                path: "deleted.txt".into(),
                content: b"delete me".to_vec(),
            }
        )
        .await,
        StorageResponse::Written { .. }
    ));
    assert_eq!(
        request(
            &mut socket,
            StorageRequest::Delete {
                path: "deleted.txt".into()
            }
        )
        .await,
        StorageResponse::Deleted
    );
    assert!(matches!(
        request(
            &mut socket,
            StorageRequest::Read {
                path: "deleted.txt".into()
            }
        )
        .await,
        StorageResponse::Error { .. }
    ));
    socket.close(None).await.expect("close deletion protocol");
    write(node, token, &disposable.id).await;
    storage::delete_file_system(
        config,
        storage::DeleteFileSystemParams {
            filesystem_id: disposable.id.clone(),
        },
    )
    .await
    .expect("tombstone resource");
    absent(config, &disposable.id).await;
    node.deleted.push(disposable.id);
}

pub(super) async fn absent(config: &Configuration, resource: &str) {
    assert!(
        matches!(storage::get_file_system(config, storage::GetFileSystemParams {
        filesystem_id: resource.into(),
    }).await, Err(of_storage_client::apis::Error::ResponseError(r)) if r.status.as_u16() == 404),
        "tombstoned resource must not be readable, including after restart"
    );
    assert!(
        storage::list_file_systems(config)
            .await
            .expect("live catalog after deletion")
            .iter()
            .all(|entry| entry.id != resource),
        "tombstone must not reappear in catalog"
    );
}
