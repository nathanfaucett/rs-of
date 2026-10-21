use std::sync::Arc;

use file_system::{
    FileHandle, FileOperation, FileSessionService, FileSystemId, MAX_FILE_READ_LENGTH, OpenRequest,
    OperationControl, RemoteOperation, Residency,
};
use iroh::{
    EndpointId,
    endpoint::{Connection, RecvStream, SendStream},
    protocol::{AcceptError, ProtocolHandler},
};
use iroh_chain::{DATA_ALPN, Server};
use model::contract::SelectedResource;
use storage_model::StorageNamespace;
use storage_service::ScopedFileSystemRuntime;
use tokio_util::sync::CancellationToken;

use crate::{
    LocalReplicationAuthorizer,
    filesystem_transport::{FILESYSTEM_STREAM_KIND, IrohFileTransport, IrohResourceDescriptor},
};

#[derive(Clone)]
pub(crate) struct StorageProtocolHandler {
    server: Server,
    authorizer: LocalReplicationAuthorizer,
    file_systems: Arc<ScopedFileSystemRuntime<EndpointId>>,
    pub(crate) admission: crate::replication_admission::ReplicationAdmission,
    cancellation_token: CancellationToken,
}

impl StorageProtocolHandler {
    pub(crate) fn new(
        server: Server,
        authorizer: LocalReplicationAuthorizer,
        file_systems: Arc<ScopedFileSystemRuntime<EndpointId>>,
        cancellation_token: CancellationToken,
    ) -> Self {
        Self {
            server,
            authorizer,
            file_systems,
            admission: crate::replication_admission::ReplicationAdmission::new(
                cancellation_token.clone(),
            ),
            cancellation_token,
        }
    }

    pub(crate) fn install_passthrough_reader(&self) -> Result<(), String> {
        let files = Arc::downgrade(&self.file_systems);
        let server = self.server.clone();
        let authorizer = self.authorizer.clone();
        let cancellation = self.cancellation_token.clone();
        let admission = self.admission.clone();
        self.file_systems.set_content_reader(move |owner, app, id, path| {
            let files = files.clone(); let server = server.clone(); let authorizer = authorizer.clone(); let cancellation_token = cancellation.clone(); let admission = admission.clone();
            async move {
                let handler = Self { server, authorizer, file_systems: files.upgrade().ok_or("Storage runtime is unavailable")?, cancellation_token, admission };
                let resource = IrohResourceDescriptor { owner_subject: owner, application_id: app.to_string(), filesystem_id: id.as_uuid().to_string() };
                let mut failure = "no configured peers".to_owned();
                for peer in handler.server.peers().ids() {
                    let connected = tokio::select! { () = handler.cancellation_token.cancelled() => return Err("Storage is shutting down".into()), result = handler.server.connect_direct_with_alpn(peer, DATA_ALPN) => result };
                    let connection = match connected { Ok(connection) => connection, Err(error) => { failure = format!("peer connection failed: {error}"); continue; } };
                    match handler.read_remote_connection(&connection, resource.clone(), &path).await { Ok(content) => return Ok(content), Err(error) => failure = format!("peer content denied/unavailable: {error}") }
                }
                Err(format!("no authorized reachable Full peer supplies this content: {failure}"))
            }
        })
    }

    pub(crate) async fn read_remote_connection(
        &self,
        connection: &Connection,
        resource: IrohResourceDescriptor,
        path: &str,
    ) -> std::io::Result<Vec<u8>> {
        self.require_authorized(&resource, connection).await?;
        let handler = self.clone();
        let peer_connection = connection.clone();
        let transport = IrohFileTransport::open_authorized(connection, resource, move |resource| {
            let handler = handler.clone();
            let connection = peer_connection.clone();
            async move { handler.authorize(&resource, &connection).await }
        })
        .await?;
        let mut handle = transport
            .open(
                connection.remote_id(),
                OpenRequest {
                    path: path.to_owned(),
                    revision: None,
                },
            )
            .await
            .map_err(|error| {
                std::io::Error::new(
                    error.kind(),
                    format!("Passthrough peer Open failed: {error}"),
                )
            })?;
        let mut content = Vec::new();
        let mut offset = 0_u64;
        loop {
            let bytes = tokio::select! { () = self.cancellation_token.cancelled() => return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "Storage is shutting down")), result = handle.read(offset, MAX_FILE_READ_LENGTH) => result.map_err(|error| std::io::Error::new(error.kind(), format!("Passthrough peer Read at {offset} failed: {error}")))? };
            if bytes.is_empty() {
                break;
            }
            offset = offset
                .checked_add(bytes.len() as u64)
                .ok_or_else(|| std::io::Error::other("content offset overflow"))?;
            content.extend_from_slice(&bytes);
        }
        Box::new(handle).close().await?;
        transport.close();
        Ok(content)
    }

    pub(crate) async fn synchronize_peer(
        &self,
        peer: EndpointId,
        resource: IrohResourceDescriptor,
    ) -> std::io::Result<()> {
        let connection = tokio::select! {
            () = self.cancellation_token.cancelled() => return Ok(()),
            result = self.server.connect_direct_with_alpn(peer, DATA_ALPN) => result.map_err(std::io::Error::other)?,
        };
        self.require_authorized(&resource, &connection).await?;
        let namespace = Namespace {
            owner_subject: resource.owner_subject.clone(),
            application_id: resource
                .application_id
                .parse()
                .map_err(std::io::Error::other)?,
        };
        let filesystem_id =
            FileSystemId::parse(&resource.filesystem_id).map_err(std::io::Error::other)?;
        let deleted = self
            .file_systems
            .is_tombstoned(&namespace, filesystem_id)
            .await
            .map_err(std::io::Error::other)?;
        let control = self.operation_control(resource.clone(), connection.clone());
        let handler = self.clone();
        let connection_for_guard = connection.clone();
        let authorize = move |resource| {
            let handler = handler.clone();
            let connection = connection_for_guard.clone();
            async move { handler.authorize(&resource, &connection).await }
        };
        if deleted {
            return tokio::select! {
                () = self.cancellation_token.cancelled() => Ok(()),
                result = IrohFileTransport::send_tombstone_authorized(&connection, resource, authorize) => result,
            };
        }
        let transport = tokio::select! {
            () = self.cancellation_token.cancelled() => return Ok(()),
            result = IrohFileTransport::open_authorized(&connection, resource.clone(), authorize) => result?,
        };
        self.require_authorized(&resource, &connection).await?;
        self.file_systems
            .register_selected_resource(&namespace, filesystem_id)
            .await
            .map_err(std::io::Error::other)?;
        let file_system = self
            .file_systems
            .open_resource(&namespace, filesystem_id)
            .await
            .map_err(std::io::Error::other)?;
        self.run_sync(file_system, transport, control).await
    }

    async fn run_sync(
        &self,
        file_system: Arc<file_system::FileSystem<EndpointId>>,
        transport: IrohFileTransport,
        control: OperationControl<EndpointId>,
    ) -> std::io::Result<()> {
        let sync = file_system
            .metadata_sync(transport)
            .with_operation_control(control.clone());
        let run = sync.run();
        tokio::pin!(run);
        tokio::select! {
            result = &mut run => result.map_err(std::io::Error::other),
            () = self.cancellation_token.cancelled() => {
                control.cancel();
                run.await.map_err(std::io::Error::other)
            }
        }
    }

    pub(crate) async fn accept_stream(
        &self,
        connection: Connection,
        send: SendStream,
        recv: RecvStream,
    ) {
        let _ = self.admission.run(&self.server, connection.remote_id(), Box::pin(async {
        let handler = self.clone();
        let connection_for_guard = connection.clone();
        let deletion_handler = self.clone();
        let deletion_connection = connection.clone();
        let accepted = tokio::select! {
            () = self.cancellation_token.cancelled() => return,
            result = IrohFileTransport::accept_authorized_after_marker(&connection, send, recv, move |resource| {
                let handler = handler.clone();
                let connection = connection_for_guard.clone();
                async move { handler.authorize(&resource, &connection).await }
            }, move |resource| async move {
                deletion_handler.require_authorized(&resource, &deletion_connection).await?;
                let namespace = Namespace { owner_subject: resource.owner_subject.clone(), application_id: resource.application_id.parse().map_err(std::io::Error::other)? };
                let id = FileSystemId::parse(&resource.filesystem_id).map_err(std::io::Error::other)?;
                deletion_handler.file_systems.apply_deletion_tombstone(&namespace, id).await.map_err(std::io::Error::other)
            }) => result,
        };
        let (transport, resource) = match accepted {
            Ok(Some(accepted)) => accepted,
            Ok(None) => return,
            Err(error) => {
                log::debug!("rejected filesystem sync stream: {error}");
                return;
            }
        };
        let result = async {
            let namespace = Namespace {
                owner_subject: resource.owner_subject.clone(),
                application_id: resource
                    .application_id
                    .parse()
                    .map_err(std::io::Error::other)?,
            };
            let filesystem_id =
                FileSystemId::parse(&resource.filesystem_id).map_err(std::io::Error::other)?;
            self.require_authorized(&resource, &connection).await?;
            self.file_systems
                .register_selected_resource(&namespace, filesystem_id)
                .await
                .map_err(std::io::Error::other)?;
            self.require_authorized(&resource, &connection).await?;
            let filesystem = self
                .file_systems
                .open_resource(&namespace, filesystem_id)
                .await
                .map_err(std::io::Error::other)?;
            let control = self.operation_control(resource, connection);
            self.run_sync(filesystem, transport, control).await
        }
        .await;
        if let Err(error) = result {
            log::debug!("filesystem sync session ended: {error}");
        }
        })).await;
    }

    fn operation_control(
        &self,
        resource: IrohResourceDescriptor,
        connection: Connection,
    ) -> OperationControl<EndpointId> {
        let handler = self.clone();
        OperationControl::new(move |peer, operation| {
            let handler = handler.clone();
            let resource = resource.clone();
            let connection = connection.clone();
            Box::pin(async move {
                if peer != connection.remote_id()
                    || !handler.authorize(&resource, &connection).await
                {
                    return false;
                }
                if let RemoteOperation::Session {
                    operation: FileOperation::Open(_) | FileOperation::Read { .. },
                    path,
                } = operation
                {
                    let Some(path) = path else {
                        return false;
                    };
                    let (Ok(app), Ok(id)) = (
                        resource.application_id.parse(),
                        FileSystemId::parse(&resource.filesystem_id),
                    ) else {
                        return false;
                    };
                    let scope = Namespace {
                        owner_subject: resource.owner_subject.clone(),
                        application_id: app,
                    };
                    let Ok(filesystem) = handler.file_systems.open_resource(&scope, id).await
                    else {
                        return false;
                    };
                    return filesystem
                        .residency(&path)
                        .is_ok_and(|mode| mode == Residency::Full);
                }
                true
            })
        })
    }

    pub(super) async fn authorize(
        &self,
        resource: &IrohResourceDescriptor,
        connection: &Connection,
    ) -> bool {
        if self.cancellation_token.is_cancelled()
            || !self.admission.allows()
            || !self.server.peers().contains(connection.remote_id())
        {
            return false;
        }
        let (Ok(application_id), Ok(filesystem_id)) = (
            resource.application_id.parse(),
            FileSystemId::parse(&resource.filesystem_id),
        ) else {
            return false;
        };
        let scope = Namespace {
            owner_subject: resource.owner_subject.clone(),
            application_id,
        };
        if self
            .file_systems
            .require_local_copy(&scope, filesystem_id)
            .is_err()
        {
            return false;
        }
        self.authorizer
            .authorize(
                self.server.endpoint().id(),
                connection.remote_id(),
                &SelectedResource {
                    owner_subject: resource.owner_subject.clone(),
                    application_id: resource.application_id.clone(),
                    kind: "filesystem".into(),
                    resource_id: resource.filesystem_id.clone(),
                },
            )
            .await
            && !self.cancellation_token.is_cancelled()
            && self.admission.allows()
            && self.server.peers().contains(connection.remote_id())
            && self
                .file_systems
                .require_local_copy(&scope, filesystem_id)
                .is_ok()
    }

    async fn require_authorized(
        &self,
        resource: &IrohResourceDescriptor,
        connection: &Connection,
    ) -> std::io::Result<()> {
        if self.authorize(resource, connection).await {
            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "filesystem resource is not locally authorized",
            ))
        }
    }
}

impl core::fmt::Debug for StorageProtocolHandler {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("StorageProtocolHandler")
            .finish_non_exhaustive()
    }
}
impl ProtocolHandler for StorageProtocolHandler {
    async fn accept(&self, connection: Connection) -> Result<(), AcceptError> {
        loop {
            let (send, mut recv) = tokio::select! { () = self.cancellation_token.cancelled() => return Ok(()), result = connection.accept_bi() => result? };
            let mut kind = [0];
            tokio::select! { () = self.cancellation_token.cancelled() => return Ok(()), result = recv.read_exact(&mut kind) => if result.is_err() { continue; } }
            if kind[0] == FILESYSTEM_STREAM_KIND {
                self.accept_stream(connection.clone(), send, recv).await;
            }
        }
    }
}
pub(crate) fn descriptor(resource: &SelectedResource) -> IrohResourceDescriptor {
    IrohResourceDescriptor {
        owner_subject: resource.owner_subject.clone(),
        application_id: resource.application_id.clone(),
        filesystem_id: resource.resource_id.clone(),
    }
}
struct Namespace {
    owner_subject: String,
    application_id: idp_model::model::Id,
}
impl StorageNamespace for Namespace {
    fn user_sub(&self) -> &str {
        &self.owner_subject
    }
    fn application_id(&self) -> idp_model::model::Id {
        self.application_id
    }
}
