use std::{
    io,
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{
    Json, Router,
    routing::{get, post},
};
use futures_util::{SinkExt, StreamExt};
use of_model::idp::model::Id;
use ofdb_sql::history::{SessionConfig, SyncRole, SyncTransport};
use serde_json::{Value, json};
use tokio::{net::TcpStream, task::JoinHandle};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

use super::{
    AbortReason, AttachDirection, AttachRequest, AuthToken, FRAMING_VERSION, Frame, FrameCodec,
    FrameError, Hello, MAX_FRAME_PAYLOAD, ProtocolId, RouterState, StoreIdentity, SyncMessage,
    WsNamespace, sync_router,
};
use crate::{
    DeviceClient, IdpClient,
    execution_test_support::{TestDirectory, sql},
};

#[test]
fn ws_frame_matches_existing_postcard_little_endian_payload_crc_contract() {
    let payload = SyncMessage::Ping.encode().expect("encode Ping");
    assert_eq!(payload, [5]);
    let bytes = FrameCodec::encode(&Frame {
        version: 1,
        flags: 0,
        payload,
    })
    .expect("frame");
    assert_eq!(&bytes[..7], &[1, 0, 1, 0, 0, 0, 5]);
    assert_eq!(&bytes[7..], &crc32fast::hash(&[5]).to_le_bytes());
    assert_eq!(
        SyncMessage::decode(&FrameCodec::decode(&bytes).expect("decode frame").payload)
            .expect("decode Ping"),
        SyncMessage::Ping
    );
}

#[test]
fn ws_frames_reject_flags_truncation_trailing_bytes_crc_and_oversize() {
    let bytes = FrameCodec::encode(&Frame {
        version: FRAMING_VERSION,
        flags: 0,
        payload: vec![5],
    })
    .expect("frame");
    let mut changed = bytes.clone();
    changed[1] = 1;
    assert_eq!(FrameCodec::decode(&changed), Err(FrameError::InvalidFlags));
    assert_eq!(FrameCodec::decode(&bytes[..9]), Err(FrameError::Truncated));
    let mut changed = bytes.clone();
    changed.push(0);
    assert_eq!(FrameCodec::decode(&changed), Err(FrameError::TrailingBytes));
    let mut changed = bytes.clone();
    changed[6] ^= 1;
    assert!(matches!(
        FrameCodec::decode(&changed),
        Err(FrameError::CrcMismatch { .. })
    ));
    let mut changed = bytes;
    changed[2..6].copy_from_slice(&(MAX_FRAME_PAYLOAD + 1).to_le_bytes());
    assert!(matches!(
        FrameCodec::decode(&changed),
        Err(FrameError::FrameTooLarge { .. })
    ));
    assert!(SyncMessage::decode(&[5, 0]).is_err());
}

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn send(socket: &mut Socket, message: SyncMessage) {
    let payload = message.encode().expect("typed message");
    let bytes = FrameCodec::encode(&Frame {
        version: 1,
        flags: 0,
        payload,
    })
    .expect("typed frame");
    socket
        .send(Message::Binary(bytes.into()))
        .await
        .expect("send frame");
}

async fn receive(socket: &mut Socket) -> SyncMessage {
    let message = tokio::time::timeout(Duration::from_secs(15), socket.next())
        .await
        .expect("WS reply deadline")
        .expect("WS reply")
        .expect("WS transport");
    let Message::Binary(bytes) = message else {
        panic!("binary frame expected: {message:?}")
    };
    SyncMessage::decode(
        &FrameCodec::decode(&bytes)
            .expect("valid reply frame")
            .payload,
    )
    .expect("typed reply")
}

struct Fixture {
    url: String,
    identity: StoreIdentity,
    namespace: WsNamespace,
    policy: Arc<Mutex<Value>>,
    databases: Arc<storage_service::DatabaseRuntime>,
    state: RouterState,
    tasks: Vec<JoinHandle<()>>,
    directory: TestDirectory,
}

impl Fixture {
    async fn new() -> Self {
        let directory = TestDirectory::new();
        let application = Id::now_v7();
        let identity = StoreIdentity {
            store_id: uuid::Uuid::now_v7(),
            replica_id: uuid::Uuid::now_v7(),
            creation_epoch: 1,
            schema_fingerprint: [0; 32],
        };
        let policy = Arc::new(Mutex::new(json!({
            "deviceId": "device", "endpointId": "test-endpoint", "ownerSubject": "alice", "adminAllowed": true,
            "resources": [{ "ownerSubject": "alice", "applicationId": application.to_string(), "kind": "database", "resourceId": identity.store_id.to_string() }]
        })));
        let selected = policy.clone();
        let owners = Router::new()
            .route("/idp/oauth2/token", post(|| async { Json(json!({
                "access_token": "service-token", "token_type": "Bearer", "expires_in": 300,
                "scope": "management.device.list management.replication.read management.replication.admit", "iss": "https://idp.example"
            })) }))
            .route("/device/replication/devices/test-endpoint/selections", get(move || {
                let selected = selected.clone();
                async move { Json(selected.lock().expect("policy lock").clone()) }
            }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("owner listener");
        let base = format!("http://{}", listener.local_addr().expect("owner address"));
        let owner_task = tokio::spawn(async move {
            axum::serve(listener, owners).await.expect("owner fixtures");
        });
        let device = DeviceClient::new(
            &format!("{base}/device"),
            &format!("{base}/idp"),
            "service",
            "secret",
            "https://idp.example",
            "idp-management",
        )
        .expect("Device client");
        let idp = IdpClient::local(move |_| async move {
            Ok(serde_json::from_value(json!({
                "claims": { "type": "Bearer", "use": "Access", "exp": i64::MAX, "iat": 0, "nbf": 0,
                    "iss": "https://idp.example", "aud": "storage:sync", "client_id": "client", "sub": "alice", "principal_type": "user", "scope": "" },
                "applicationId": application.to_string()
            })).expect("introspection"))
        });
        let databases = Arc::new(
            storage_service::DatabaseRuntime::new(directory.0.join("server"))
                .expect("server databases"),
        );
        let state = RouterState::new("http://localhost", "test-endpoint")
            .with_idp_client(idp)
            .with_device_client(device);
        let router = sync_router(
            state.clone(),
            databases.clone(),
            directory.0.join("staging"),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("WS listener");
        let url = format!(
            "ws://{}/storage/sync/ws",
            listener.local_addr().expect("WS address")
        );
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.expect("WS server");
        });
        Self {
            url,
            identity,
            namespace: WsNamespace {
                subject: "alice".into(),
                application_id: application,
            },
            policy,
            databases,
            state,
            tasks: vec![owner_task, task],
            directory,
        }
    }

    async fn hello(&self) -> Socket {
        let (mut socket, _) = connect_async(&self.url).await.expect("actual WS upgrade");
        send(
            &mut socket,
            SyncMessage::Hello(Hello {
                protocol_version: 1,
                manifest: Vec::new(),
                store_identity: self.identity.clone(),
                supported_protocols: vec![ProtocolId::OfdbSqlV7],
            }),
        )
        .await;
        assert!(matches!(receive(&mut socket).await, SyncMessage::Hello(_)));
        socket
    }

    async fn attach(&self) -> Socket {
        let mut socket = self.hello().await;
        send(
            &mut socket,
            SyncMessage::AttachRequest(AttachRequest {
                store_identity: self.identity.clone(),
                auth_token: AuthToken(b"user-token".to_vec()),
                direction: AttachDirection::Bidirectional,
            }),
        )
        .await;
        assert!(
            matches!(receive(&mut socket).await, SyncMessage::AttachResponse(response) if response.accepted)
        );
        socket
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

struct ClientTransport(Socket);
impl SyncTransport for ClientTransport {
    type Error = io::Error;
    async fn send(&mut self, payload: Vec<u8>) -> io::Result<()> {
        send(
            &mut self.0,
            SyncMessage::OwnerMessage {
                protocol_id: ProtocolId::OfdbSqlV7,
                payload,
            },
        )
        .await;
        Ok(())
    }
    async fn receive(&mut self) -> io::Result<Vec<u8>> {
        match receive(&mut self.0).await {
            SyncMessage::OwnerMessage {
                protocol_id: ProtocolId::OfdbSqlV7,
                payload,
            } => Ok(payload),
            message => Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{message:?}"),
            )),
        }
    }
}

#[tokio::test]
async fn ws_real_typed_sql_exchange_is_scoped_and_retries_after_revocation() {
    let fixture = Fixture::new().await;
    let source =
        ofdb_sql::Database::open(fixture.directory.0.join("source.redb")).expect("source SQL");
    sql(&source, &format!("CREATE TABLE documents (id UUID PRIMARY KEY, value TEXT); INSERT INTO documents VALUES (CAST('{}' AS UUID), 'kept')", Id::now_v7())).await;
    let target = fixture
        .databases
        .open_selected(&fixture.namespace, fixture.identity.store_id.into())
        .expect("target")
        .expect("selected");
    let other = fixture
        .databases
        .open_selected(
            &WsNamespace {
                subject: "bob".into(),
                application_id: fixture.namespace.application_id,
            },
            fixture.identity.store_id.into(),
        )
        .expect("other namespace")
        .expect("other selected");
    sql(&other, &format!("CREATE TABLE documents (id UUID PRIMARY KEY, value TEXT); INSERT INTO documents VALUES (CAST('{}' AS UUID), 'private')", Id::now_v7())).await;
    let mut transport = ClientTransport(fixture.attach().await);
    source
        .exchange_resource_history(
            &mut transport,
            &SessionConfig::default(),
            SyncRole::Initiator,
            fixture.directory.0.join("source-stage"),
            || async { Ok(()) },
        )
        .await
        .expect("real typed WS SQL exchange");
    assert_eq!(values(&target).await, [db::Value::Text("kept".into())]);
    assert_eq!(values(&other).await, [db::Value::Text("private".into())]);

    let mut socket = fixture.attach().await;
    fixture.policy.lock().expect("policy lock")["adminAllowed"] = json!(false);
    send(
        &mut socket,
        SyncMessage::OwnerMessage {
            protocol_id: ProtocolId::OfdbSqlV7,
            payload: vec![0],
        },
    )
    .await;
    assert_eq!(
        receive(&mut socket).await,
        SyncMessage::Abort(AbortReason::Unauthorized)
    );
    assert_eq!(values(&target).await, [db::Value::Text("kept".into())]);
    fixture.policy.lock().expect("policy lock")["adminAllowed"] = json!(true);
    sql(
        &source,
        &format!(
            "INSERT INTO documents VALUES (CAST('{}' AS UUID), 'retry')",
            Id::now_v7()
        ),
    )
    .await;
    let mut transport = ClientTransport(fixture.attach().await);
    source
        .exchange_resource_history(
            &mut transport,
            &SessionConfig::default(),
            SyncRole::Initiator,
            fixture.directory.0.join("source-stage"),
            || async { Ok(()) },
        )
        .await
        .expect("authorized reconnect exchange");
    assert_eq!(
        values(&target).await,
        [
            db::Value::Text("kept".into()),
            db::Value::Text("retry".into())
        ]
    );
}

struct PairedTransport {
    send: tokio::sync::mpsc::Sender<Vec<u8>>,
    receive: tokio::sync::mpsc::Receiver<Vec<u8>>,
}

impl SyncTransport for PairedTransport {
    type Error = io::Error;
    async fn send(&mut self, bytes: Vec<u8>) -> io::Result<()> {
        self.send.send(bytes).await.map_err(io::Error::other)
    }
    async fn receive(&mut self) -> io::Result<Vec<u8>> {
        self.receive
            .recv()
            .await
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "peer closed"))
    }
}

#[tokio::test]
async fn ws_authorization_revoked_during_preapply_preserves_committed_history() {
    use super::{SessionAuthorization, authorize_sync};
    use crate::execution_test_support::Gate;

    let fixture = Fixture::new().await;
    let source = ofdb_sql::Database::open(fixture.directory.0.join("source.redb")).expect("source");
    sql(&source, &format!("CREATE TABLE documents (id UUID PRIMARY KEY, value TEXT); INSERT INTO documents VALUES (CAST('{}' AS UUID), 'denied')", Id::now_v7())).await;
    let target = fixture
        .databases
        .open_selected(&fixture.namespace, fixture.identity.store_id.into())
        .expect("target")
        .expect("selected");
    let before = target
        .export_history()
        .await
        .expect("initial committed history");
    let idp = fixture.state.idp_client.as_ref().expect("IdP client");
    let device = fixture.state.device_client.as_ref().expect("Device client");
    let auth = authorize_sync(
        idp,
        device,
        "user-token",
        &fixture.identity,
        "test-endpoint",
    )
    .await
    .expect("initial authorization");
    let authorization = SessionAuthorization {
        idp,
        device,
        token: "user-token",
        identity: &fixture.identity,
        endpoint_id: "test-endpoint",
        expected: &auth,
    };
    let gate = Gate::default();
    let (left, right_receive) = tokio::sync::mpsc::channel(16);
    let (right, left_receive) = tokio::sync::mpsc::channel(16);
    let config = SessionConfig::default();
    let mut run = Box::pin(async {
        tokio::join!(
            async {
                let mut transport = PairedTransport {
                    send: left,
                    receive: left_receive,
                };
                source
                    .exchange_resource_history(
                        &mut transport,
                        &config,
                        SyncRole::Initiator,
                        fixture.directory.0.join("source-stage"),
                        || async { Ok(()) },
                    )
                    .await
            },
            async {
                let mut transport = PairedTransport {
                    send: right,
                    receive: right_receive,
                };
                target
                    .exchange_resource_history(
                        &mut transport,
                        &config,
                        SyncRole::Responder,
                        fixture.directory.0.join("target-stage"),
                        || async {
                            gate.pause().await;
                            authorization
                                .check()
                                .await
                                .map_err(|error| error.to_string())
                        },
                    )
                    .await
            },
        )
    });
    tokio::select! {
        result = &mut run => panic!("exchange ended before pre-apply: {result:?}"),
        () = gate.reached() => {}
    }
    fixture.policy.lock().expect("policy lock")["adminAllowed"] = json!(false);
    gate.release();
    let (_, denied) = tokio::time::timeout(Duration::from_secs(15), run)
        .await
        .expect("denied exchange ends");
    assert!(
        matches!(denied, Err(ofdb_sql::history::SyncError::PreApply(_))),
        "authorization guard must deny apply: {denied:?}"
    );
    assert_eq!(
        target.export_history().await.expect("history after denial"),
        before
    );
}

async fn values(database: &ofdb_sql::Database) -> Vec<db::Value> {
    database
        .client()
        .translate_and_execute(
            "SELECT value FROM documents ORDER BY value",
            &db::SqlTranslator,
        )
        .await
        .expect("observe SQL")[0]
        .rows
        .iter()
        .map(|row| row.values[0].clone())
        .collect()
}

#[tokio::test]
async fn ws_rejects_changed_identity_unsupported_direction_and_owner_protocol() {
    let fixture = Fixture::new().await;
    for direction in [AttachDirection::Push, AttachDirection::Pull] {
        let mut socket = fixture.hello().await;
        send(
            &mut socket,
            SyncMessage::AttachRequest(AttachRequest {
                store_identity: fixture.identity.clone(),
                auth_token: AuthToken(b"user-token".to_vec()),
                direction,
            }),
        )
        .await;
        assert_eq!(
            receive(&mut socket).await,
            SyncMessage::Abort(AbortReason::MalformedMessage)
        );
    }
    let mut socket = fixture.hello().await;
    let mut changed = fixture.identity.clone();
    changed.store_id = uuid::Uuid::now_v7();
    send(
        &mut socket,
        SyncMessage::AttachRequest(AttachRequest {
            store_identity: changed,
            auth_token: AuthToken(b"user-token".to_vec()),
            direction: AttachDirection::Bidirectional,
        }),
    )
    .await;
    assert_eq!(
        receive(&mut socket).await,
        SyncMessage::Abort(AbortReason::MalformedMessage)
    );
    let mut socket = fixture.attach().await;
    send(
        &mut socket,
        SyncMessage::OwnerMessage {
            protocol_id: ProtocolId::Other("kv".into()),
            payload: vec![],
        },
    )
    .await;
    assert_eq!(
        receive(&mut socket).await,
        SyncMessage::Abort(AbortReason::MalformedMessage)
    );
}
