use std::{io, sync::Arc};

use iroh::{EndpointAddr, EndpointId, address_lookup::MemoryLookup};
use iroh_chain::Server;
use of_model::contract::SelectedResource;
use serde::{Deserialize, Serialize};

use crate::{IdpClient, LocalReplicationAuthorizer, authorize_storage_token};
use crate::database_protocol::{DatabaseProtocolHandler, FrameAuthorizer, descriptor};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum StorageSyncDirection { Push, Pull, Bidirectional }

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StorageSyncRequest {
    pub target_endpoint_id: String,
    pub resource_id: String,
    pub kind: String,
    pub direction: StorageSyncDirection,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StorageSyncResult {
    pub resource_id: String,
    pub target_endpoint_id: String,
    pub direction: StorageSyncDirection,
}

#[derive(Clone)]
pub struct StorageSyncDriver {
    idp: IdpClient,
    server: Server,
    authorizer: LocalReplicationAuthorizer,
    database: DatabaseProtocolHandler,
    addresses: MemoryLookup,
    lifetime: tokio_util::sync::CancellationToken,
    sessions: Arc<tokio::sync::RwLock<()>>,
}
impl StorageSyncDriver {
    pub(crate) fn new(idp: IdpClient, server: Server, authorizer: LocalReplicationAuthorizer, database: DatabaseProtocolHandler, lifetime: tokio_util::sync::CancellationToken, sessions: Arc<tokio::sync::RwLock<()>>) -> io::Result<Self> {
        let addresses = MemoryLookup::new();
        server.endpoint().address_lookup().map_err(io::Error::other)?.add(addresses.clone());
        Ok(Self { idp, server, authorizer, database, addresses, lifetime, sessions })
    }

    pub async fn synchronize(&self, token: &str, request: StorageSyncRequest,
        resolve: impl std::future::Future<Output = io::Result<EndpointAddr>> + Send,
    ) -> io::Result<StorageSyncResult> {
        let _session = self.sessions.read().await;
        tokio::select! {
            biased;
            () = self.lifetime.cancelled() => Err(io::Error::new(io::ErrorKind::Interrupted, "Native sync cancelled")),
            result = self.synchronize_active(token, request, resolve) => result,
        }
    }

    async fn synchronize_active(&self, token: &str, request: StorageSyncRequest,
        resolve: impl std::future::Future<Output = io::Result<EndpointAddr>> + Send,
    ) -> io::Result<StorageSyncResult> {
        let authorization = authorize_storage_token(&self.idp, token).await.map_err(|_| denied())?;
        if authorization.resource != "storage:databases" || !authorization.can_read || !authorization.can_write {
            return Err(denied());
        }
        if request.kind != "database" || request.direction != StorageSyncDirection::Bidirectional {
            return Err(io::Error::new(io::ErrorKind::Unsupported, "Native sync currently supports Bidirectional SQL + KV databases only; Push, Pull, and filesystem content are unsupported"));
        }
        let peer: EndpointId = request.target_endpoint_id.parse().map_err(|_| denied())?;
        if peer.to_string() != request.target_endpoint_id { return Err(denied()); }
        let resource = SelectedResource {
            owner_subject: authorization.subject.clone(),
            application_id: authorization.application_id.clone(),
            kind: request.kind,
            resource_id: request.resource_id.clone(),
        };
        if !self.server.peers().contains(peer) || !self.authorizer.authorize(self.server.endpoint().id(), peer, &resource).await {
            return Err(denied());
        }
        let address = resolve.await?;
        if address.id != peer { return Err(denied()); }
        self.addresses.add_endpoint_info(address);
        let idp = self.idp.clone();
        let token = token.to_owned();
        let authorize: FrameAuthorizer = Arc::new(move || {
            let idp = idp.clone(); let token = token.clone(); let expected = authorization.clone();
            Box::pin(async move {
                authorize_storage_token(&idp, &token).await.is_ok_and(|current|
                    current.subject == expected.subject && current.application_id == expected.application_id &&
                    current.resource == "storage:databases" && current.can_read && current.can_write)
            })
        });
        let database = self.database.clone().with_execution_guard(authorize.clone());
        database.synchronize_peer(peer, descriptor(&resource)).await?;
        if !authorize().await { return Err(denied()); }
        Ok(StorageSyncResult { resource_id: request.resource_id, target_endpoint_id: request.target_endpoint_id, direction: request.direction })
    }
}
fn denied() -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, "Current User, Application, Device, and selected database authorization is required")
}
