use std::{
    io,
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use db::{Database, DatabaseTransport};
use idp_model::model::Id;
use idp_service::oauth2::NodeSignerBinding;
use iroh::endpoint::{Connection, RecvStream, SendStream};
use iroh_chain::Server;
use ofdb_sql::{SessionConfig, SyncError, SyncResult, SyncRole, SyncTransport};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

pub use crate::security_policy::SecurityContext;
use crate::security_policy::{SecurityHistory, SecuritySync};

#[path = "security_bootstrap.rs"]
pub mod bootstrap;
pub use bootstrap::BOOTSTRAP_ALPN;

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
    installation: String,
    binding: Option<NodeSignerBinding>,
    staging_directory: PathBuf,
    cancellation: CancellationToken,
}

impl SecurityProtocol {
    pub fn cancel(&self) {
        self.cancellation.cancel();
    }

    pub fn into_databases(self) -> (Arc<Database>, Arc<Database>) {
        (self.idp, self.management)
    }

    pub fn new(
        server: Server,
        idp: Arc<Database>,
        management: Arc<Database>,
        policy: Arc<Database>,
        installation: String,
        binding: Option<NodeSignerBinding>,
        staging_directory: PathBuf,
        cancellation: CancellationToken,
    ) -> Self {
        Self {
            server,
            idp,
            management,
            policy,
            installation,
            binding,
            staging_directory,
            cancellation,
        }
    }

    fn owner(&self) -> SecuritySync<'_> {
        SecuritySync::new(&self.idp, &self.management, &self.policy)
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

        let remote_member = self
            .owner()
            .binding(&connection.remote_id().to_string(), None)
            .await?
            .member;
        self.authorize(connection, remote_member, context).await?;

        let (mut send, recv) = connection.open_bi().await.map_err(io::Error::other)?;
        self.authorize(connection, remote_member, context).await?;
        send.write_all(&[SECURITY_STREAM_KIND])
            .await
            .map_err(io::Error::other)?;
        let mut transport = DatabaseTransport::new(send, recv);
        self.authorize(connection, remote_member, context).await?;
        transport
            .send(serde_json::to_vec(&handshake).map_err(io::Error::other)?)
            .await?;
        let response: Response =
            serde_json::from_slice(&transport.receive().await?).map_err(io::Error::other)?;
        let Response::Accepted(remote) = response else {
            return Err(denied("peer denied security synchronization"));
        };
        self.validate_handshake(&remote, context)?;
        if remote.member != remote_member {
            return Err(denied("peer member does not match local endpoint binding"));
        }
        self.authorize(connection, remote.member, context).await?;
        self.exchange(
            connection,
            remote.member,
            context,
            &mut transport,
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
        let remote: Handshake =
            serde_json::from_slice(&transport.receive().await?).map_err(io::Error::other)?;
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
        self.exchange(
            &connection,
            remote.member,
            remote.context,
            &mut transport,
            SyncRole::Responder,
        )
        .await
        .map(|_| ())
    }

    pub async fn validate_local_schema(&self, context: SecurityContext) -> io::Result<()> {
        for (database, history, _) in self.sessions(context) {
            history.validate_schema(database).await?;
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
        let binding = self
            .owner()
            .binding(&self.server.endpoint().id().to_string(), None)
            .await?;
        Ok(Handshake {
            version: 1,
            installation: self.installation.clone(),
            member: binding.member,
            context,
        })
    }

    async fn authorize(
        &self,
        connection: &Connection,
        member: Id,
        context: SecurityContext,
    ) -> io::Result<()> {
        self.local_handshake(context).await?;
        self.owner()
            .authorize(&connection.remote_id().to_string(), member, context)
            .await?;
        if self.cancellation.is_cancelled() {
            return Err(denied("security synchronization is shutting down"));
        }
        Ok(())
    }

    async fn exchange(
        &self,
        connection: &Connection,
        member: Id,
        context: SecurityContext,
        transport: &mut DatabaseTransport,
        role: SyncRole,
    ) -> io::Result<SyncResult> {
        self.authorize(connection, member, context).await?;
        log::debug!("starting scoped security session: {context:?} {role:?}");
        let mut bindings = Vec::new();
        {
            let local = self.local_handshake(context).await?;
            for (member, endpoint) in [
                (local.member, self.server.endpoint().id().to_string()),
                (member, connection.remote_id().to_string()),
            ] {
                bindings.push(self.owner().binding(&endpoint, Some(member)).await?);
            }
        }
        let mut total = SyncResult::default();
        for (database, history, tables) in self.sessions(context) {
            let directory = self.staging_directory.join(history.name());
            let mut transport = AuthorizedSecurityTransport {
                transport: &mut *transport,
                protocol: self,
                connection,
                member,
                context,
            };
            let result = database
                .synchronize_tables_with_stage_directory_candidate_validated_checked(
                    &mut transport,
                    &SessionConfig::default(),
                    role,
                    directory,
                    &tables,
                    |table, column, value| {
                        history.validate_value(table, column, value, &self.installation)
                    },
                    |view| history.validate_candidate(view, &self.installation, &bindings, None),
                    || async {
                        log::debug!("checking scoped security apply: {context:?} {role:?}");
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
}

struct AuthorizedSecurityTransport<'a, T> {
    transport: &'a mut T,
    protocol: &'a SecurityProtocol,
    connection: &'a Connection,
    member: Id,
    context: SecurityContext,
}

impl<T: SyncTransport<Error = io::Error>> SyncTransport for AuthorizedSecurityTransport<'_, T> {
    type Error = io::Error;

    async fn send(&mut self, frame: Vec<u8>) -> io::Result<()> {
        self.protocol
            .authorize(self.connection, self.member, self.context)
            .await?;
        self.transport.send(frame).await
    }

    async fn receive(&mut self) -> io::Result<Vec<u8>> {
        self.transport.receive().await
    }
}

fn denied(reason: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, reason.to_string())
}

fn now() -> io::Result<i64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_secs()
        .try_into()
        .map_err(io::Error::other)
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
#[path = "security_outbound_tests.rs"]
mod outbound_tests;
#[cfg(test)]
#[path = "security_protocol_test_fixtures.rs"]
mod test_fixtures;

#[cfg(test)]
#[path = "security_history_tests.rs"]
mod history_tests;
#[cfg(test)]
#[path = "security_sync_tests.rs"]
mod sync_tests;
