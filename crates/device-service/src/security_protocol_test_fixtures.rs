use std::{
    any::Any,
    io,
    panic::AssertUnwindSafe,
    sync::{Arc, Mutex},
};

use db::{Database, SqlTranslator};
use futures_util::FutureExt;
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
) -> (impl iroh::protocol::ProtocolHandler, StreamPanics)
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
    let panics = StreamPanics::default();
    (
        NativeHandler {
            cancellation,
            stream,
            panics: panics.clone(),
        },
        panics,
    )
}

// Router/JoinSet shutdown may discard task failures; replay stream assertions in the parent test.
#[derive(Clone, Default)]
pub(super) struct StreamPanics(Arc<Mutex<Vec<Box<dyn Any + Send>>>>);

impl StreamPanics {
    async fn capture<U: core::future::Future<Output = io::Result<()>>>(
        &self,
        stream: U,
    ) -> io::Result<()> {
        match AssertUnwindSafe(stream).catch_unwind().await {
            Ok(result) => result,
            Err(panic) => {
                self.0.lock().expect("stream panic recorder").push(panic);
                Err(io::Error::other("fixture stream panicked"))
            }
        }
    }

    pub(super) fn check(&self) {
        let panic = self.0.lock().expect("stream panic recorder").pop();
        if let Some(panic) = panic {
            std::panic::resume_unwind(panic);
        }
    }
}

struct NativeHandler<F> {
    cancellation: tokio_util::sync::CancellationToken,
    stream: F,
    panics: StreamPanics,
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
                    let panics = self.panics.clone();
                    streams.spawn(async move { panics.capture(stream).await });
                }
                result = streams.join_next(), if !streams.is_empty() => {
                    match result.expect("active stream task") {
                        // Protocol rejection is an expected outcome of adversarial streams.
                        Ok(Ok(())) | Ok(Err(_)) => {},
                        Err(error) => std::panic::resume_unwind(error.into_panic()),
                    }
                }
            }
        }
        streams.shutdown().await;
        Ok(())
    }
}

#[tokio::test]
async fn stream_denial_is_not_a_task_panic() {
    let panics = StreamPanics::default();
    let error = panics
        .capture(async { Err(io::Error::new(io::ErrorKind::PermissionDenied, "denied")) })
        .await
        .expect_err("protocol denial is retained");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    panics.check();
}

#[tokio::test]
#[should_panic(expected = "stream assertion must reach parent")]
async fn stream_panic_survives_task_shutdown() {
    let panics = StreamPanics::default();
    let recorder = panics.clone();
    let mut streams = tokio::task::JoinSet::new();
    let (finished, completed) = tokio::sync::oneshot::channel();
    streams.spawn(async move {
        let result = recorder
            .capture(async { panic!("stream assertion must reach parent") })
            .await;
        finished.send(()).expect("signal completed panic capture");
        result
    });
    completed
        .await
        .expect("stream panic captured before shutdown");
    streams.shutdown().await;
    panics.check();
}
