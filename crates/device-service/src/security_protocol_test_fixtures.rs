use std::sync::Arc;

use db::{Database, SqlTranslator};
use idp_model::{
    contract::{
        IdpSignerRecord, JwkPublic, JwkPublicParameters, JwsAlgorithm, KeyUse, ReplicaMembership,
    },
    model::Id,
};
use idp_service::replica::DbReplicaSignerRepo;

const INSTALLATION: &str = "https://installation.example";
pub(super) async fn sql(database: &Database, statement: &str) -> Vec<db::QueryResult> {
    database
        .client()
        .translate_and_execute(statement, &SqlTranslator)
        .await
        .expect("public SQL fixture")
}

pub(super) async fn enroll(idp: Arc<Database>, member: Id, endpoint: iroh::EndpointId, key: Id) {
    DbReplicaSignerRepo::new(idp.client())
        .enroll(
            &ReplicaMembership {
                installation_id: INSTALLATION.into(),
                member_id: member,
                endpoint_id: endpoint.to_string(),
                issuer: INSTALLATION.into(),
                approved_at: 1,
                revoked_at: None,
            },
            &IdpSignerRecord {
                member_id: member,
                key_id: key,
                issuer: INSTALLATION.into(),
                public_jwk: JwkPublic {
                    kid: key.to_string(),
                    r#use: KeyUse::Signature,
                    alg: JwsAlgorithm::ES256K,
                    params: JwkPublicParameters::Ec {
                        crv: "secp256k1".into(),
                        x: "eb5mfvncu6xVoGKVzocLBwKb/NstzijZWfKBWxb4F5g".into(),
                        y: "SDradyajxGVdpPv8DhEIqP0XtEimhVQZnEfQj/sQ1Lg".into(),
                    },
                },
                approved_at: Some(1),
                revoked_at: None,
                expires_at: None,
            },
        )
        .await
        .expect("persist public peer approval");
}

pub(super) fn native_handler<F, U>(
    cancellation: tokio_util::sync::CancellationToken,
    stream: F,
) -> impl iroh::protocol::ProtocolHandler
where
    F: Fn(
            u8,
            iroh::endpoint::Connection,
            iroh::endpoint::SendStream,
            iroh::endpoint::RecvStream,
        ) -> U
        + Send
        + Sync
        + 'static,
    U: core::future::Future<Output = std::io::Result<()>> + Send + 'static,
{
    NativeHandler {
        cancellation,
        stream,
    }
}

struct NativeHandler<F> {
    cancellation: tokio_util::sync::CancellationToken,
    stream: F,
}

impl<F> core::fmt::Debug for NativeHandler<F> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("NativeHandler").finish_non_exhaustive()
    }
}

impl<F, U> iroh::protocol::ProtocolHandler for NativeHandler<F>
where
    F: Fn(
            u8,
            iroh::endpoint::Connection,
            iroh::endpoint::SendStream,
            iroh::endpoint::RecvStream,
        ) -> U
        + Send
        + Sync
        + 'static,
    U: core::future::Future<Output = std::io::Result<()>> + Send + 'static,
{
    async fn accept(
        &self,
        connection: iroh::endpoint::Connection,
    ) -> Result<(), iroh::protocol::AcceptError> {
        let mut streams = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                () = self.cancellation.cancelled() => break,
                result = connection.accept_bi() => {
                    let Ok((send, mut recv)) = result else { break };
                    let mut kind = [0];
                    recv.read_exact(&mut kind).await.map_err(std::io::Error::other)?;
                    let stream = (self.stream)(kind[0], connection.clone(), send, recv);
                    streams.spawn(stream);
                }
                result = streams.join_next(), if !streams.is_empty() => {
                    let _ = result.expect("active stream task").expect("security stream task must not panic");
                }
            }
        }
        streams.shutdown().await;
        Ok(())
    }
}
