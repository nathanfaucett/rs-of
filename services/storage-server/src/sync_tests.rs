use std::{
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicI64, AtomicUsize, Ordering},
    },
    time::Duration,
};

use axum::{
    Json, Router,
    routing::{get, post},
};
use futures_util::{SinkExt, StreamExt};
use of_model::id::Id;
use ofdb_sql::{
    SessionConfig, SyncRole, SyncTransport,
    history::{SyncError, SyncManifest, SyncMessage as SqlSyncMessage},
};
use serde_json::{Value, json};
use tokio::{net::TcpStream, task::JoinHandle, time::timeout};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

use super::{
    AbortReason, AttachDirection, AttachRequest, AuthToken, FRAMING_VERSION, Frame, FrameCodec,
    FrameError, Hello, MAX_FRAME_PAYLOAD, ProtocolId, RouterState, StoreIdentity, SyncMessage,
    WsNamespace, sync_router,
};
use crate::{
    device_client::DeviceClient,
    execution_test_support::{Gate, TestDirectory, sql},
    idp_client::IdpClient,
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
    let message = timeout(Duration::from_secs(15), socket.next())
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

#[derive(Default)]
struct OwnerCalls {
    tokens: AtomicUsize,
    selection_reads_with_repair: AtomicUsize,
    snapshots: AtomicUsize,
    snapshot_admissions: AtomicUsize,
}

struct Fixture {
    url: String,
    identity: StoreIdentity,
    namespace: WsNamespace,
    policy: Arc<Mutex<Value>>,
    expires_at: Arc<AtomicI64>,
    authorization_details: Arc<Mutex<Option<Value>>>,
    snapshot_gate: Arc<Mutex<Option<Gate>>>,
    owner_calls: Arc<OwnerCalls>,
    databases: Arc<storage_service::database_runtime::DatabaseRuntime>,
    state: RouterState,
    tasks: Vec<JoinHandle<()>>,
    directory: TestDirectory,
}

impl Fixture {
    async fn new() -> Self {
        Self::with_storage(false).await
    }

    async fn with_storage(shared: bool) -> Self {
        Self::with_options(shared, false).await
    }

    async fn with_options(shared: bool, local: bool) -> Self {
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
        let (databases, control) = if shared {
            use db::adapters::EmbeddedDatabase;
            let database = EmbeddedDatabase::open(directory.0.join("server"))
                .await
                .expect("shared host");
            let client = database.client();
            client.execute_sql("CREATE TABLE snapshot_control (id UUID PRIMARY KEY); INSERT INTO snapshot_control DEFAULT VALUES", None).await.expect("read-only owner fixture");
            (
                storage_service::database_runtime::DatabaseRuntime::with_shared(
                    database.storage(),
                    client.clone(),
                )
                .expect("shared runtime"),
                Some(client),
            )
        } else {
            (
                storage_service::database_runtime::DatabaseRuntime::new(directory.0.join("server"))
                    .expect("server databases"),
                None,
            )
        };
        let databases = Arc::new(databases);
        let snapshot_gate: Arc<Mutex<Option<Gate>>> = Arc::new(Mutex::new(None));
        let gate = snapshot_gate.clone();
        let snapshot = policy.clone();
        let expires_at = Arc::new(AtomicI64::new(i64::MAX));
        let expiry = expires_at.clone();
        let selected = policy.clone();
        let owner_calls = Arc::new(OwnerCalls::default());
        let tokens = owner_calls.clone();
        let repairs = owner_calls.clone();
        let snapshots = owner_calls.clone();
        let admissions = owner_calls.clone();
        let admission_policy = policy.clone();
        let owners = Router::new()
            .route("/idp/oauth2/token", post(move || {
                tokens.tokens.fetch_add(1, Ordering::Relaxed);
                async { Json(json!({
                    "access_token": "service-token", "token_type": "Bearer", "expires_in": 300,
                    "scope": "management.device.list management.replication.read management.replication.admit", "iss": "https://idp.example"
                })) }
            }))
            .route("/device/replication/devices/test-endpoint/selections", get(move || {
                repairs.selection_reads_with_repair.fetch_add(1, Ordering::Relaxed);
                let selected = selected.clone();
                async move { Json(selected.lock().expect("policy lock").clone()) }
            }))
            .route("/device/replication/selection-snapshots/test-endpoint/admission", post(move || {
                admissions.snapshot_admissions.fetch_add(1, Ordering::Relaxed);
                let policy = admission_policy.clone();
                async move { Json(policy.lock().expect("policy lock").clone()) }
            }))
            .route("/device/replication/selection-snapshots/test-endpoint", get(move || {
                snapshots.snapshots.fetch_add(1, Ordering::Relaxed);
                let control = control.clone();
                let policy = snapshot.clone();
                let gate = gate.lock().expect("snapshot gate").clone();
                async move {
                    if let Some(control) = control {
                        let rows = control.execute_sql("SELECT id FROM snapshot_control", None).await.expect("read selection evidence from the same shared host");
                        assert_eq!(rows[0].rows.len(), 1);
                    }
                    if let Some(gate) = gate {
                        gate.pause().await;
                    }
                    Json(policy.lock().expect("policy lock").clone())
                }
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
        let authorization_details = Arc::new(Mutex::new(Some(json!([
            { "type": "storage", "actions": ["read", "write"] }
        ]))));
        let details = authorization_details.clone();
        let idp = IdpClient::local(move |_| {
            let exp = expiry.load(Ordering::Relaxed);
            let details = details.lock().expect("authorization details lock").clone();
            async move {
                let mut response = json!({
                    "claims": { "type": "Bearer", "use": "Access", "exp": exp, "iat": 0, "nbf": 0,
                        "iss": "https://idp.example", "aud": "storage:sync", "resource": "storage:sync",
                        "client_id": "client", "sub": "alice", "principal_type": "user", "scope": "" },
                    "applicationId": application.to_string()
                });
                if let Some(details) = details {
                    response["claims"]["authorization_details"] = details;
                }
                Ok(serde_json::from_value(response).expect("introspection"))
            }
        });
        let state = RouterState::new("http://localhost", "test-endpoint")
            .with_idp_client(idp)
            .with_device_client(device);
        let router = if local {
            super::local_sync_router(
                state.clone(),
                databases.clone(),
                directory.0.join("staging"),
            )
        } else {
            sync_router(
                state.clone(),
                databases.clone(),
                directory.0.join("staging"),
            )
        };
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
            expires_at,
            authorization_details,
            snapshot_gate,
            owner_calls,
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

    async fn first_upload(&self) -> (Socket, SyncMessage) {
        let mut socket = self.hello().await;
        let mut payload = SyncMessage::AttachRequest(AttachRequest {
            store_identity: self.identity.clone(),
            auth_token: AuthToken(b"user-token".to_vec()),
            direction: AttachDirection::Bidirectional,
        })
        .encode()
        .expect("attach payload");
        payload[0] = 7;
        let bytes = FrameCodec::encode(&Frame {
            version: 1,
            flags: 0,
            payload,
        })
        .expect("first-upload frame");
        socket
            .send(Message::Binary(bytes.into()))
            .await
            .expect("send first upload");
        let response = receive(&mut socket).await;
        (socket, response)
    }

    async fn attach(&self) -> Socket {
        let (socket, response) = self.request_attach().await;
        assert!(matches!(response, SyncMessage::AttachResponse(response) if response.accepted));
        socket
    }

    async fn request_attach(&self) -> (Socket, SyncMessage) {
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
        let response = receive(&mut socket).await;
        (socket, response)
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
async fn ws_shared_resource_sync_and_retirement_create_no_staging_or_sidecars() {
    shared_resource_sync_and_retirement(false).await;
}

#[tokio::test]
async fn ws_local_scoped_resource_sync_and_retirement_without_device_selection() {
    shared_resource_sync_and_retirement(true).await;
}

async fn shared_resource_sync_and_retirement(local: bool) {
    let fixture = Fixture::with_options(true, local).await;
    if local {
        *fixture.policy.lock().expect("policy lock") = Value::Null;
    }
    let source_host = ofdb_sql::SharedDatabase::create(fixture.directory.0.join("source.redb"))
        .expect("source host");
    let source = ofdb_sql::Database::open_shared(&source_host, "source").expect("source namespace");
    sql(&source, "CREATE TABLE documents (id UUID PRIMARY KEY, value TEXT); INSERT INTO documents (value) VALUES ('kept')").await;
    let target = fixture
        .databases
        .open_selected(&fixture.namespace, fixture.identity.store_id)
        .expect("target")
        .expect("selected resource");
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
        .expect("shared SQL exchange");
    assert_eq!(
        values(&target).await,
        [ofdb_sql::Value::Text("kept".into())]
    );
    fixture
        .databases
        .remove_local_copy(&fixture.namespace, fixture.identity.store_id)
        .await
        .expect("resource retirement");
    assert!(
        target
            .client()
            .execute_sql("SELECT * FROM documents", None)
            .await
            .is_err()
    );
    assert!(fixture.directory.0.join("server/unified.redb").is_file());
    assert_eq!(
        std::fs::read_dir(fixture.directory.0.join("server"))
            .expect("single file host")
            .count(),
        1
    );
    assert!(!fixture.directory.0.join("staging").exists());
    assert!(!fixture.directory.0.join("source-stage").exists());
}

#[tokio::test]
async fn ws_real_typed_sql_exchange_is_scoped_and_retries_after_revocation() {
    let fixture = Fixture::new().await;
    let source =
        ofdb_sql::Database::open(fixture.directory.0.join("source.redb")).expect("source SQL");
    sql(&source, "CREATE TABLE documents (id UUID PRIMARY KEY, value TEXT); INSERT INTO documents (value) VALUES ('kept')").await;
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
    sql(&other, "CREATE TABLE documents (id UUID PRIMARY KEY, value TEXT); INSERT INTO documents (value) VALUES ('private')").await;
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
    assert_eq!(
        values(&target).await,
        [ofdb_sql::Value::Text("kept".into())]
    );
    assert_eq!(
        values(&other).await,
        [ofdb_sql::Value::Text("private".into())]
    );

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
    assert_eq!(
        values(&target).await,
        [ofdb_sql::Value::Text("kept".into())]
    );
    fixture.policy.lock().expect("policy lock")["adminAllowed"] = json!(true);
    sql(&source, "INSERT INTO documents (value) VALUES ('retry')").await;
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
            ofdb_sql::Value::Text("kept".into()),
            ofdb_sql::Value::Text("retry".into())
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
    sql(&source, "CREATE TABLE documents (id UUID PRIMARY KEY, value TEXT); INSERT INTO documents (value) VALUES ('denied')").await;
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
        device: Some(device),
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
    let (_, denied) = timeout(Duration::from_secs(15), run)
        .await
        .expect("denied exchange ends");
    assert!(
        matches!(denied, Err(SyncError::PreApply(_))),
        "authorization guard must deny apply: {denied:?}"
    );
    assert_eq!(
        target.export_history().await.expect("history after denial"),
        before
    );
}

async fn values(database: &ofdb_sql::Database) -> Vec<ofdb_sql::Value> {
    database
        .client()
        .translate_and_execute(
            "SELECT value FROM documents ORDER BY value",
            &ofdb_sql::SqlTranslator,
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
    let (_, response) = request_pull_or_push(&fixture, AttachDirection::Push).await;
    assert_eq!(response, SyncMessage::Abort(AbortReason::MalformedMessage));
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

async fn request_pull_or_push(
    fixture: &Fixture,
    direction: AttachDirection,
) -> (Socket, SyncMessage) {
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
    let response = receive(&mut socket).await;
    (socket, response)
}

#[tokio::test]
async fn ws_pull_does_not_provision_a_selected_resource() {
    for shared in [false, true] {
        let fixture = Fixture::with_storage(shared).await;
        let (_, response) = request_pull_or_push(&fixture, AttachDirection::Pull).await;
        assert_eq!(
            response,
            SyncMessage::Abort(AbortReason::BackendUnavailable)
        );
        assert!(
            fixture
                .databases
                .list(&fixture.namespace)
                .expect("catalog")
                .is_empty()
        );
    }
}

#[tokio::test]
async fn ws_pull_needs_read_and_never_accepts_incoming_sql_history() {
    let fixture = Fixture::new().await;
    *fixture.authorization_details.lock().expect("capability") =
        Some(json!([{ "type": "storage", "actions": ["write"] }]));
    let (_, denied) = request_pull_or_push(&fixture, AttachDirection::Pull).await;
    assert_eq!(denied, SyncMessage::Abort(AbortReason::Unauthorized));
    *fixture.authorization_details.lock().expect("capability") =
        Some(json!([{ "type": "storage", "actions": ["read"] }]));
    let remote = fixture
        .databases
        .open_selected(&fixture.namespace, fixture.identity.store_id)
        .expect("remote database")
        .expect("selected resource");
    sql(&remote, "CREATE TABLE documents (id INTEGER PRIMARY KEY, value TEXT); INSERT INTO documents VALUES (1, 'only remote')").await;
    let before = remote.history_manifest().await.expect("source history");
    let (socket, response) = request_pull_or_push(&fixture, AttachDirection::Pull).await;
    assert!(
        matches!(response, SyncMessage::AttachResponse(response) if response.granted_direction == AttachDirection::Pull)
    );
    let restored = ofdb_sql::Database::in_memory();
    restored
        .restore_resource_history_memory(
            &mut ClientTransport(socket),
            &SessionConfig::default(),
            b"source".to_vec(),
            || Ok(()),
        )
        .await
        .expect("read-only restore");
    assert_eq!(
        values(&restored).await,
        [ofdb_sql::Value::Text("only remote".into())]
    );
    assert_eq!(
        restored.initial_restore_tag().await.expect("local tag"),
        Some(b"source".to_vec())
    );

    assert_eq!(
        restored.history_manifest().await.expect("tag stays local"),
        before
    );

    let (socket, _) = request_pull_or_push(&fixture, AttachDirection::Pull).await;
    let sender = ofdb_sql::Database::in_memory();
    sql(&sender, "CREATE TABLE documents (id INTEGER PRIMARY KEY, value TEXT); INSERT INTO documents VALUES (2, 'not allowed')").await;
    let result = sender
        .exchange_resource_history(
            &mut ClientTransport(socket),
            &SessionConfig::default(),
            SyncRole::Initiator,
            fixture.directory.0.join("stage"),
            || async { Ok(()) },
        )
        .await;
    assert!(
        matches!(result, Err(SyncError::Transport(error)) if error.to_string().contains("MalformedMessage"))
    );
    assert_eq!(
        remote.history_manifest().await.expect("no remote write"),
        before
    );
    let (mut socket, response) = request_pull_or_push(&fixture, AttachDirection::Pull).await;
    assert!(matches!(response, SyncMessage::AttachResponse(_)));
    fixture.policy.lock().expect("policy lock")["resources"] = json!([]);
    send(
        &mut socket,
        SyncMessage::OwnerMessage {
            protocol_id: ProtocolId::OfdbSqlV7,
            payload: vec![],
        },
    )
    .await;
    assert_eq!(
        receive(&mut socket).await,
        SyncMessage::Abort(AbortReason::Unauthorized)
    );
    assert_eq!(
        fixture
            .owner_calls
            .selection_reads_with_repair
            .load(Ordering::Relaxed),
        0
    );
    assert!(fixture.owner_calls.snapshots.load(Ordering::Relaxed) > 4);
    assert_eq!(
        fixture
            .owner_calls
            .snapshot_admissions
            .load(Ordering::Relaxed),
        3
    );
    assert_eq!(fixture.owner_calls.tokens.load(Ordering::Relaxed), 1);
}

struct WriteAfterPullManifest {
    transport: ClientTransport,
    writer: Option<Arc<ofdb_sql::Database>>,
}

impl SyncTransport for WriteAfterPullManifest {
    type Error = io::Error;

    async fn send(&mut self, frame: Vec<u8>) -> io::Result<()> {
        self.transport.send(frame).await
    }

    async fn receive(&mut self) -> io::Result<Vec<u8>> {
        let frame = self.transport.receive().await?;
        if matches!(
            postcard::from_bytes(&frame),
            Ok(SqlSyncMessage::Manifest(_))
        ) && let Some(writer) = self.writer.take()
        {
            sql(
                &writer,
                "UPDATE documents SET value = 'later'; INSERT INTO documents VALUES (2, 'new')",
            )
            .await;
        }
        Ok(frame)
    }
}

#[tokio::test]
async fn ws_pull_serves_one_snapshot_even_if_a_write_commits_during_exchange() {
    let fixture = Fixture::with_storage(true).await;
    *fixture.authorization_details.lock().expect("capability") =
        Some(json!([{ "type": "storage", "actions": ["read"] }]));
    let remote = fixture
        .databases
        .open_selected(&fixture.namespace, fixture.identity.store_id)
        .expect("open resource")
        .expect("selected resource");
    sql(&remote, "CREATE TABLE documents (id INTEGER PRIMARY KEY, value TEXT); INSERT INTO documents VALUES (1, 'snapshot')").await;
    let snapshot = remote.history_manifest().await.expect("snapshot history");
    let (socket, response) = request_pull_or_push(&fixture, AttachDirection::Pull).await;
    assert!(
        matches!(response, SyncMessage::AttachResponse(response) if response.granted_direction == AttachDirection::Pull)
    );
    let staged = ofdb_sql::Database::in_memory();
    staged
        .restore_resource_history_memory(
            &mut WriteAfterPullManifest {
                transport: ClientTransport(socket),
                writer: Some(remote.clone()),
            },
            &SessionConfig::default(),
            Vec::new(),
            || Ok(()),
        )
        .await
        .expect("read a coherent snapshot");
    assert_eq!(
        values(&staged).await,
        [ofdb_sql::Value::Text("snapshot".into())]
    );
    assert_eq!(
        staged.history_manifest().await.expect("stage history"),
        snapshot
    );
    assert_ne!(
        remote
            .history_manifest()
            .await
            .expect("updated remote history"),
        snapshot
    );
    assert_eq!(values(&remote).await.len(), 2);
}

#[tokio::test]
async fn ws_first_upload_rejects_even_dropped_remote_schema_without_changing_history() {
    let fixture = Fixture::new().await;
    let target = fixture
        .databases
        .open_selected(&fixture.namespace, fixture.identity.store_id)
        .expect("open target")
        .expect("selected target");
    sql(
        &target,
        "CREATE TABLE prior (id UUID PRIMARY KEY); DROP TABLE prior",
    )
    .await;
    let before = target.history_manifest().await.expect("remote history");
    assert_ne!(before, SyncManifest::default());

    let (_, response) = fixture.first_upload().await;
    assert_eq!(response.encode().expect("abort payload"), [4, 4]);
    assert_eq!(
        target.history_manifest().await.expect("retained history"),
        before
    );
}

async fn stage_upload(fixture: &Fixture, source: &ofdb_sql::Database, socket: Socket) -> Socket {
    let mut transport = ClientTransport(socket);
    timeout(
        Duration::from_secs(15),
        source.exchange_resource_history(
            &mut transport,
            &SessionConfig::default(),
            SyncRole::Initiator,
            fixture.directory.0.join("source-stage"),
            || async { Ok(()) },
        ),
    )
    .await
    .expect("SQL exchange deadline")
    .expect("SQL exchange with private staging database");
    transport.0
}

#[tokio::test]
async fn ws_first_upload_publishes_history_and_refuses_replay_on_same_resource() {
    for shared in [false, true] {
        let fixture = Fixture::with_storage(shared).await;
        let source = ofdb_sql::Database::in_memory();
        sql(&source, "CREATE TABLE documents (id UUID PRIMARY KEY, value TEXT); CREATE INDEX by_value ON documents (value); INSERT INTO documents (value) VALUES ('kept'), ('deleted'); DELETE FROM documents WHERE value = 'deleted'").await;
        let before = source.history_manifest().await.expect("source history");
        let target = fixture
            .databases
            .open_selected(&fixture.namespace, fixture.identity.store_id)
            .expect("open target")
            .expect("selected target");
        let (socket, response) = fixture.first_upload().await;
        assert!(matches!(response, SyncMessage::AttachResponse(response) if response.accepted));
        let receipt = receive(&mut stage_upload(&fixture, &source, socket).await).await;
        assert_eq!(receipt.encode().expect("publish receipt"), [8]);
        assert_eq!(
            values(&target).await,
            [ofdb_sql::Value::Text("kept".into())]
        );
        assert_eq!(
            target.history_manifest().await.expect("published history"),
            before
        );
        assert_eq!(
            source.history_manifest().await.expect("source unchanged"),
            before
        );

        let (_, response) = fixture.first_upload().await;
        assert_eq!(
            response,
            SyncMessage::Abort(AbortReason::DestinationHasHistory)
        );
        assert_eq!(target.history_manifest().await.expect("no replay"), before);
    }
}

#[tokio::test]
async fn ws_first_upload_loses_to_sql_write_after_attach_without_merging_either_side() {
    for shared in [false, true] {
        let fixture = Fixture::with_storage(shared).await;
        let source = ofdb_sql::Database::in_memory();
        sql(&source, "CREATE TABLE documents (id UUID PRIMARY KEY, value TEXT); INSERT INTO documents (value) VALUES ('local')").await;
        let local = source.history_manifest().await.expect("source history");
        let target = fixture
            .databases
            .open_selected(&fixture.namespace, fixture.identity.store_id)
            .expect("open target")
            .expect("selected target");
        let (socket, response) = fixture.first_upload().await;
        assert!(matches!(response, SyncMessage::AttachResponse(response) if response.accepted));

        let socket = {
            let client = target.client();
            let mut writer = client
                .transaction()
                .await
                .expect("begin writer during upload");
            writer.execute_sql("CREATE TABLE documents (id UUID PRIMARY KEY, value TEXT); INSERT INTO documents (value) VALUES ('remote')", None).await.expect("write competing history");
            // Standalone Redb serializes writers. Finish SQL exchange while the competing
            // transaction still owns the writer, then prove it can commit while publish waits.
            let socket = if shared {
                socket
            } else {
                stage_upload(&fixture, &source, socket).await
            };
            timeout(Duration::from_secs(5), writer.commit())
                .await
                .expect("writer commit deadline")
                .expect("competing commit");
            socket
        };
        let remote = target.history_manifest().await.expect("competing history");
        let mut socket = if shared {
            stage_upload(&fixture, &source, socket).await
        } else {
            socket
        };
        let receipt = receive(&mut socket).await;
        assert_eq!(
            receipt,
            SyncMessage::Abort(AbortReason::DestinationHasHistory)
        );
        assert_eq!(
            target.history_manifest().await.expect("no remote merge"),
            remote
        );
        assert_eq!(
            source.history_manifest().await.expect("no local merge"),
            local
        );
    }
}

struct DisconnectBeforeRecovery {
    transport: ClientTransport,
}

impl SyncTransport for DisconnectBeforeRecovery {
    type Error = io::Error;

    async fn send(&mut self, frame: Vec<u8>) -> io::Result<()> {
        if matches!(
            postcard::from_bytes(&frame),
            Ok(SqlSyncMessage::RequestSnapshots(_))
        ) {
            self.transport
                .0
                .close(None)
                .await
                .map_err(io::Error::other)?;
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "test interrupted upload",
            ));
        }
        self.transport.send(frame).await
    }

    async fn receive(&mut self) -> io::Result<Vec<u8>> {
        self.transport.receive().await
    }
}

#[tokio::test]
async fn ws_first_upload_disconnect_before_completion_can_retry_same_resource() {
    let fixture = Fixture::with_storage(true).await;
    let source = ofdb_sql::Database::in_memory();
    sql(&source, "CREATE TABLE documents (id UUID PRIMARY KEY, value TEXT); INSERT INTO documents (value) VALUES ('local')").await;
    let before = source.history_manifest().await.expect("source history");
    let target = fixture
        .databases
        .open_selected(&fixture.namespace, fixture.identity.store_id)
        .expect("open target")
        .expect("selected target");
    let (socket, response) = fixture.first_upload().await;
    assert!(matches!(response, SyncMessage::AttachResponse(response) if response.accepted));
    let error = source
        .exchange_resource_history(
            &mut DisconnectBeforeRecovery {
                transport: ClientTransport(socket),
            },
            &SessionConfig::default(),
            SyncRole::Initiator,
            fixture.directory.0.join("source-stage"),
            || async { Ok(()) },
        )
        .await
        .expect_err("interrupted after the first SQL transfer round");
    assert!(
        matches!(error, SyncError::Transport(error) if error.kind() == io::ErrorKind::ConnectionAborted)
    );
    assert_eq!(
        target.history_manifest().await.expect("target history"),
        SyncManifest::default()
    );
    assert_eq!(
        source.history_manifest().await.expect("source unchanged"),
        before
    );

    let (socket, response) = fixture.first_upload().await;
    assert!(matches!(response, SyncMessage::AttachResponse(response) if response.accepted));
    assert_eq!(
        receive(&mut stage_upload(&fixture, &source, socket).await).await,
        SyncMessage::FirstUploadPublished
    );
    assert_eq!(
        target.history_manifest().await.expect("published retry"),
        before
    );
}

fn insufficient_storage_details() -> [Option<Value>; 3] {
    [
        Some(json!([{ "type": "storage", "actions": ["read"] }])),
        Some(json!([{ "type": "storage", "actions": ["write"] }])),
        None,
    ]
}

#[tokio::test]
async fn ws_bidirectional_admission_requires_both_storage_actions() {
    let fixture = Fixture::new().await;
    for details in insufficient_storage_details() {
        *fixture
            .authorization_details
            .lock()
            .expect("authorization details lock") = details;
        let (_, response) = fixture.request_attach().await;
        assert_eq!(response, SyncMessage::Abort(AbortReason::Unauthorized));
        let (_, response) = fixture.first_upload().await;
        assert_eq!(response, SyncMessage::Abort(AbortReason::Unauthorized));
    }
}

#[tokio::test]
async fn ws_bidirectional_rechecks_require_both_storage_actions() {
    let fixture = Fixture::with_storage(true).await;
    let idp = fixture.state.idp_client.as_ref().expect("IdP client");
    let device = fixture.state.device_client.as_ref().expect("Device client");
    let endpoint = &fixture.state.endpoint_id;
    let expected = super::authorize_sync(idp, device, "user-token", &fixture.identity, endpoint)
        .await
        .expect("bidirectional capability and exact Selection");
    let authorization = super::SessionAuthorization {
        idp,
        device: Some(device),
        token: "user-token",
        identity: &fixture.identity,
        endpoint_id: endpoint,
        expected: &expected,
    };
    let baseline = fixture
        .authorization_details
        .lock()
        .expect("authorization details lock")
        .clone();
    authorization
        .check_publish()
        .await
        .expect("initial publish authorization");
    for details in insufficient_storage_details() {
        *fixture
            .authorization_details
            .lock()
            .expect("authorization details lock") = baseline.clone();
        let mut socket = fixture.attach().await;
        *fixture
            .authorization_details
            .lock()
            .expect("authorization details lock") = details;
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
        assert_eq!(
            authorization
                .check_publish()
                .await
                .expect_err("publish requires both actions")
                .kind(),
            io::ErrorKind::PermissionDenied
        );
    }
}

#[tokio::test]
async fn ws_first_upload_cannot_bypass_selection() {
    let fixture = Fixture::new().await;
    let target = fixture
        .databases
        .open_selected(&fixture.namespace, fixture.identity.store_id)
        .expect("open target")
        .expect("selected target");
    sql(&target, "CREATE TABLE prior (id UUID PRIMARY KEY)").await;
    fixture.policy.lock().expect("policy lock")["resources"] = json!([]);
    // A denied selection must never disclose whether this Resource contains history.
    let (_, response) = fixture.first_upload().await;
    assert_eq!(response, SyncMessage::Abort(AbortReason::Unauthorized));
}

#[tokio::test]
async fn ws_first_upload_aborts_unauthorized_if_selection_is_revoked_while_writer_is_held() {
    let fixture = Fixture::new().await;
    let source = ofdb_sql::Database::in_memory();
    sql(&source, "CREATE TABLE documents (id UUID PRIMARY KEY, value TEXT); INSERT INTO documents (value) VALUES ('local')").await;
    let before = source.history_manifest().await.expect("source history");
    let target = fixture
        .databases
        .open_selected(&fixture.namespace, fixture.identity.store_id)
        .expect("open target")
        .expect("selected target");
    let (socket, response) = fixture.first_upload().await;
    assert!(matches!(response, SyncMessage::AttachResponse(response) if response.accepted));

    let client = target.client();
    let writer = client
        .transaction()
        .await
        .expect("hold empty destination writer");
    let mut socket = stage_upload(&fixture, &source, socket).await;
    fixture.policy.lock().expect("policy lock")["resources"] = json!([]);
    writer
        .rollback()
        .await
        .expect("release unchanged destination");

    assert_eq!(
        receive(&mut socket).await,
        SyncMessage::Abort(AbortReason::Unauthorized)
    );
    assert_eq!(
        target
            .history_manifest()
            .await
            .expect("destination history"),
        SyncManifest::default()
    );
    assert_eq!(
        source.history_manifest().await.expect("source history"),
        before
    );
}

#[tokio::test]
async fn ws_first_upload_rejects_token_expiring_during_shared_selection_snapshot() {
    let fixture = Fixture::with_storage(true).await;
    let source = ofdb_sql::Database::in_memory();
    sql(&source, "CREATE TABLE documents (id UUID PRIMARY KEY, value TEXT); INSERT INTO documents (value) VALUES ('local')").await;
    let before = source.history_manifest().await.expect("source history");
    let target = fixture
        .databases
        .open_selected(&fixture.namespace, fixture.identity.store_id)
        .expect("open target")
        .expect("selected target");
    let gate = Gate::default();
    *fixture.snapshot_gate.lock().expect("snapshot gate") = Some(gate.clone());
    let expires_at = super::unix_time_seconds() + 4;
    fixture.expires_at.store(expires_at, Ordering::Relaxed);
    let (socket, response) = fixture.first_upload().await;
    assert!(matches!(response, SyncMessage::AttachResponse(response) if response.accepted));
    let mut socket = stage_upload(&fixture, &source, socket).await;
    timeout(Duration::from_secs(5), gate.reached())
        .await
        .expect("read-only snapshot under the shared writer");
    assert!(
        super::unix_time_seconds() < expires_at,
        "snapshot must start before expiry"
    );
    while super::unix_time_seconds() < expires_at {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    gate.release();
    assert_eq!(
        receive(&mut socket).await,
        SyncMessage::Abort(AbortReason::Unauthorized)
    );
    assert_eq!(
        target
            .history_manifest()
            .await
            .expect("destination history"),
        SyncManifest::default()
    );
    assert_eq!(
        source.history_manifest().await.expect("unchanged source"),
        before
    );
}
