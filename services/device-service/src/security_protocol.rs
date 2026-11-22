#[cfg(feature = "ofdb")]
use crate::security_policy::SecurityContext;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use crate::security_policy::{SecurityHistory, SecuritySync};
use db::DatabaseTransport;
use idp_service::replica::LocalIdentity;
use iroh::endpoint::{Connection, RecvStream, SendStream};
use iroh_chain::Server;
use management_service::LocalPermissions;
use of_model::id::Id;
use of_model::idp::contract::permission_evaluation::PermissionEvaluationRequest;
use of_model::idp::contract::permission_evaluation::PermissionSubject;
use of_model::idp::contract::permission_evaluation::PermissionTarget;
use ofdb_sql::Database;
use ofdb_sql::history::SyncTransport;
use ofdb_sql::{SessionConfig, SyncError, SyncResult, SyncRole};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::ofdb::DbDeviceRepo;

const SECURITY_STREAM_KIND: u8 = 3;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Handshake {
    version: u8,
    installation: String,
    member: Id,
    context: SecurityContext,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
enum Response {
    Accepted(Handshake),
    Denied,
}

#[derive(Clone)]
pub struct SecurityProtocol {
    server: Server,
    idp: Arc<Database>,
    management: Arc<Database>,
    policy: Arc<Database>,
    idp_identity: LocalIdentity,
    device_repo: DbDeviceRepo,
    permissions: LocalPermissions,
    installation: String,
    staging_directory: PathBuf,
    cancellation: CancellationToken,
}

impl SecurityProtocol {
    pub fn cancel(&self) {
        self.cancellation.cancel();
    }

    pub fn new(
        server: Server,
        idp: Arc<Database>,
        management: Arc<Database>,
        policy: Arc<Database>,
        installation: String,
        staging_directory: PathBuf,
        cancellation: CancellationToken,
    ) -> Self {
        let idp_identity = LocalIdentity::new(idp.client());
        let device_repo = DbDeviceRepo::new(management.client());
        let permissions = LocalPermissions::new(policy.client());
        Self {
            server,
            idp,
            management,
            policy,
            idp_identity,
            device_repo,
            permissions,
            installation,
            staging_directory,
            cancellation,
        }
    }

    fn owner(&self) -> SecuritySync<'_> {
        SecuritySync::new(&self.idp_identity, &self.device_repo, &self.permissions)
    }

    fn sessions(
        &self,
        context: SecurityContext,
    ) -> Vec<(&Arc<Database>, SecurityHistory, Vec<&'static str>)> {
        context
            .histories()
            .iter()
            .map(|history| {
                let database = match history {
                    SecurityHistory::Idp => &self.idp,
                    SecurityHistory::IdentityPolicy => &self.policy,
                    SecurityHistory::Devices => &self.management,
                };
                (database, *history, history.tables())
            })
            .collect()
    }

    pub async fn synchronize(
        &self,
        connection: &Connection,
        context: SecurityContext,
    ) -> io::Result<SyncResult> {
        self.validate_local_schema(context).await?;
        let handshake = self.local_handshake(context).await?;

        let remote = self
            .owner()
            .endpoint_binding(&connection.remote_id().to_string())
            .await?;
        self.authorize(connection, remote.member, context).await?;
        let (mut send, recv) = connection.open_bi().await.map_err(io::Error::other)?;
        send.write_all(&[SECURITY_STREAM_KIND])
            .await
            .map_err(io::Error::other)?;
        let mut transport = DatabaseTransport::new(send, recv);
        self.authorize(connection, remote.member, context).await?;
        transport
            .send(serde_json::to_vec(&handshake).map_err(io::Error::other)?)
            .await?;
        let data: Vec<u8> = transport.receive().await?;
        let response: Response = serde_json::from_slice(&data).map_err(io::Error::other)?;
        let Response::Accepted(remote) = response else {
            return Err(denied("peer denied security synchronization"));
        };
        self.validate_handshake(&remote, context)?;
        self.authorize(connection, remote.member, context).await?;

        self.exchange_framed(
            connection,
            remote.member,
            context,
            transport,
            SyncRole::Initiator,
        )
        .await
    }

    pub async fn accept_data_stream(
        &self,
        kind: u8,
        connection: Connection,
        send: SendStream,
        recv: RecvStream,
    ) -> io::Result<()> {
        if kind != SECURITY_STREAM_KIND {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unknown DATA stream kind",
            ));
        }
        self.accept_stream(connection, send, recv).await
    }

    async fn accept_stream(
        &self,
        connection: Connection,
        send: SendStream,
        recv: RecvStream,
    ) -> io::Result<()> {
        let mut transport = DatabaseTransport::new(send, recv);
        let data: Vec<u8> = transport.receive().await?;
        let remote: Handshake = serde_json::from_slice(&data).map_err(io::Error::other)?;
        let admission = async {
            self.validate_handshake(&remote, remote.context)?;
            self.validate_local_schema(remote.context).await?;
            let local = self.local_handshake(remote.context).await?;
            self.authorize(&connection, remote.member, remote.context)
                .await?;
            Ok::<_, io::Error>(local)
        }
        .await;
        let local = admission?;
        self.authorize(&connection, remote.member, remote.context)
            .await?;
        transport
            .send(serde_json::to_vec(&Response::Accepted(local)).map_err(io::Error::other)?)
            .await?;

        self.exchange_framed(
            &connection,
            remote.member,
            remote.context,
            transport,
            SyncRole::Responder,
        )
        .await
        .map(|_| ())
    }

    pub async fn validate_local_schema(&self, context: SecurityContext) -> io::Result<()> {
        for (database, history, tables) in self.sessions(context) {
            history.validate_schema(&database.client(), &tables).await?;
        }
        Ok(())
    }

    fn validate_handshake(&self, remote: &Handshake, context: SecurityContext) -> io::Result<()> {
        if remote.version != 1
            || remote.installation != self.installation
            || remote.context != context
            || remote.member.is_nil()
        {
            return Err(denied(
                "security synchronization Installation/context/member mismatch",
            ));
        }
        Ok(())
    }

    async fn local_handshake(&self, context: SecurityContext) -> io::Result<Handshake> {
        if self.cancellation.is_cancelled() {
            return Err(denied("security synchronization is shutting down"));
        }
        let local = self
            .owner()
            .endpoint_binding(&self.server.endpoint().id().to_string())
            .await?;
        self.authorize_owner(local.owner, context).await?;
        let member = local.member;
        Ok(Handshake {
            version: 1,
            installation: self.installation.clone(),
            member,
            context,
        })
    }

    async fn exchange_framed(
        &self,
        connection: &Connection,
        member: Id,
        context: SecurityContext,
        mut transport: DatabaseTransport,
        role: SyncRole,
    ) -> io::Result<SyncResult> {
        self.authorize(connection, member, context).await?;
        log::debug!("starting scoped security session: {context:?}");
        let mut bindings = Vec::new();
        {
            let local = self.local_handshake(context).await?;
            let endpoints: [(Id, String); 2] = [
                (local.member, self.server.endpoint().id().to_string()),
                (member, connection.remote_id().to_string()),
            ];
            for (member, endpoint) in endpoints {
                bindings.push(self.owner().peer_binding(&endpoint, member).await?);
            }
        }
        let mut total = SyncResult::default();
        let mut authorized = AuthorizedSecurityTransport {
            transport: &mut transport,
            protocol: self,
            connection,
            member,
            context,
        };
        for (database, history, tables) in self.sessions(context) {
            let result = database
                .exchange_history(
                    &mut authorized,
                    &SessionConfig::default(),
                    role,
                    self.staging_directory.join(history.name()),
                    &tables,
                    |table, column, value| {
                        history.validate_value(table, column, value, &self.installation)
                    },
                    |view| {
                        history
                            .validate_candidate(view, &bindings)
                            .map_err(|e| e.to_string())
                    },
                    || async {
                        self.authorize(connection, member, context)
                            .await
                            .map_err(|error| error.to_string())
                    },
                )
                .await
                .map_err(|error| match error {
                    SyncError::PreApply(reason) => denied(reason),
                    SyncError::Transport(error) => error,
                    error => io::Error::other(error.to_string()),
                })?;
            accumulate(&mut total, result);
        }
        Ok(total)
    }

    async fn authorize(
        &self,
        connection: &Connection,
        member: Id,
        context: SecurityContext,
    ) -> io::Result<()> {
        if self.cancellation.is_cancelled() {
            return Err(denied("security synchronization is shutting down"));
        }
        let remote = self
            .owner()
            .peer_binding(&connection.remote_id().to_string(), member)
            .await?;
        self.authorize_owner(remote.owner, context).await
    }

    async fn authorize_owner(&self, owner: Id, context: SecurityContext) -> io::Result<()> {
        let request = PermissionEvaluationRequest {
            request_id: Id::now_v7(),
            subject: PermissionSubject::User { id: owner },
            action: context.permission().0,
            target: PermissionTarget::Installation {
                resource: context.permission().1,
            },
        };
        self.owner().authorize(context, &request).await?;
        Ok(())
    }
}

struct AuthorizedSecurityTransport<'a, T> {
    transport: &'a mut T,
    protocol: &'a SecurityProtocol,
    connection: &'a Connection,
    member: Id,
    context: SecurityContext,
}
impl<T: SyncTransport<Error = io::Error> + Send> SyncTransport
    for AuthorizedSecurityTransport<'_, T>
{
    type Error = io::Error;
    async fn send(&mut self, frame: Vec<u8>) -> io::Result<()> {
        self.protocol
            .authorize(self.connection, self.member, self.context)
            .await?;
        self.transport.send(frame).await
    }
    async fn receive(&mut self) -> io::Result<Vec<u8>> {
        self.protocol
            .authorize(self.connection, self.member, self.context)
            .await?;
        let frame = self.transport.receive().await?;
        self.protocol
            .authorize(self.connection, self.member, self.context)
            .await?;
        Ok(frame)
    }
}

fn denied(reason: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, reason.to_string())
}

fn accumulate(total: &mut SyncResult, result: SyncResult) {
    total.received_units += result.received_units;
    total.sent_units += result.sent_units;
    total.received_changes += result.received_changes;
    total.sent_changes += result.sent_changes;
    total.received_snapshots += result.received_snapshots;
    total.sent_snapshots += result.sent_snapshots;
}

#[cfg(test)]
mod test_fixtures {
    include!("security_protocol_test_fixtures.rs");
}

#[cfg(test)]
mod sync_tests {
    include!("security_sync_tests.rs");
}

#[cfg(test)]
mod outbound_tests {
    include!("security_outbound_tests.rs");
}
#[cfg(test)]
mod history_tests {
    include!("security_history_tests.rs");
}
