#![cfg(feature = "native")]

use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use futures_util::{SinkExt, StreamExt};
use of_storage_client::{
    handle::SqlDatabaseHandle,
    protocol::{
        AttachDirection, AttachResponse, Frame, FrameCodec, ProtocolId, StoreIdentity, SyncMessage,
    },
    transport::{OwnerTransport, TransportConfig},
};
use ofdb_sql::history::{SessionConfig, SyncRole, SyncTransport};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{WebSocketStream, tungstenite::Message};
use uuid::Uuid;

type Socket = WebSocketStream<TcpStream>;
async fn receive(socket: &mut Socket) -> SyncMessage {
    let Message::Binary(bytes) = socket.next().await.expect("frame").expect("socket") else {
        panic!("binary frame required")
    };
    SyncMessage::decode(&FrameCodec::decode(&bytes).expect("valid frame").payload)
        .expect("typed message")
}
async fn send(socket: &mut Socket, message: SyncMessage) {
    let bytes = FrameCodec::encode(&Frame {
        version: 1,
        flags: 0,
        payload: message.encode().expect("message"),
    })
    .expect("frame");
    socket
        .send(Message::Binary(bytes.into()))
        .await
        .expect("send reply");
}
struct OwnerStream(Socket);
impl SyncTransport for OwnerStream {
    type Error = io::Error;
    async fn receive(&mut self) -> io::Result<Vec<u8>> {
        match receive(&mut self.0).await {
            SyncMessage::OwnerMessage {
                protocol_id: ProtocolId::OfdbSqlV7,
                payload,
            } => Ok(payload),
            message => Err(io::Error::other(format!(
                "wrong owner message: {message:?}"
            ))),
        }
    }
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
}
fn identity() -> StoreIdentity {
    StoreIdentity {
        store_id: Uuid::now_v7(),
        replica_id: Uuid::now_v7(),
        creation_epoch: 0,
        schema_fingerprint: [0; 32],
    }
}
async fn attach(socket: &mut Socket) {
    let SyncMessage::Hello(hello) = receive(socket).await else {
        panic!("hello first")
    };
    assert_eq!(hello.protocol_version, 1);
    assert_eq!(hello.supported_protocols, vec![ProtocolId::OfdbSqlV7]);
    send(socket, SyncMessage::Hello(hello.clone())).await;
    let SyncMessage::AttachRequest(request) = receive(socket).await else {
        panic!("attach second")
    };
    assert_eq!(request.store_identity, hello.store_identity);
    assert_eq!(request.auth_token.0, b"fresh-sync-token");
    assert_eq!(request.direction, AttachDirection::Bidirectional);
    send(
        socket,
        SyncMessage::AttachResponse(AttachResponse {
            accepted: true,
            backend_replica_id: Uuid::now_v7(),
            session_id: Uuid::now_v7(),
            granted_direction: AttachDirection::Bidirectional,
            max_frame_bytes: 16 * 1024 * 1024,
            max_session_bytes: 1024 * 1024 * 1024,
        }),
    )
    .await;
}

#[tokio::test]
async fn websocket_attaches_before_typed_checked_sql_exchange() {
    tokio::time::timeout(std::time::Duration::from_secs(20), async {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let url = format!("ws://{}", listener.local_addr().expect("address"));
        let server = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.expect("connection");
            let mut socket = tokio_tungstenite::accept_async(socket).await.expect("upgrade");
            attach(&mut socket).await;
            let database = ofdb_sql::Database::in_memory();
            database.client().execute_sql("CREATE TABLE records (id TEXT PRIMARY KEY, value TEXT); INSERT INTO records VALUES ('one', 'remote')", None).await.expect("remote data");
            let directory = tempfile::tempdir().expect("staging");
            database.exchange_resource_history(&mut OwnerStream(socket), &SessionConfig::default(), SyncRole::Responder, directory.path(), || async { Ok(()) }).await.expect("checked exchange");
        });
        let checks = Arc::new(AtomicUsize::new(0));
        let tokens = checks.clone();
        let handle = SqlDatabaseHandle::new(ofdb_sql::Database::in_memory(), identity(), TransportConfig::new(url), Arc::new(move || { tokens.fetch_add(1, Ordering::SeqCst); Box::pin(async { Ok("fresh-sync-token".into()) }) }));
        handle.synchronize(None).await.expect("authenticated SQL exchange");
        assert!(checks.load(Ordering::SeqCst) > 1, "pre-apply must recheck credentials");
        assert_eq!(handle.execute("SELECT * FROM records").await.expect("received row").rows.len(), 1);
        server.await.expect("server task");
    }).await.expect("session deadline");
}

#[tokio::test]
async fn websocket_rejects_wrong_resource_before_sending_credentials() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
    let url = format!("ws://{}", listener.local_addr().expect("address"));
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.expect("connection");
        let mut socket = tokio_tungstenite::accept_async(socket)
            .await
            .expect("upgrade");
        let SyncMessage::Hello(mut hello) = receive(&mut socket).await else {
            panic!("hello")
        };
        hello.store_identity.store_id = Uuid::now_v7();
        send(&mut socket, SyncMessage::Hello(hello)).await;
        assert!(!matches!(socket.next().await, Some(Ok(Message::Binary(_)))));
    });
    assert!(
        OwnerTransport::connect(&url, &identity(), "must-not-leak".into())
            .await
            .is_err()
    );
    server.await.expect("server task");
}

#[test]
fn frame_wire_format_and_corruption_are_checked() {
    // Ping is variant 5. This is the server v1 header and IEEE CRC32 of [5].
    let expected = [1, 0, 1, 0, 0, 0, 5, 2, 27, 104, 162];
    let frame = Frame {
        version: 1,
        flags: 0,
        payload: SyncMessage::Ping.encode().expect("ping"),
    };
    assert_eq!(FrameCodec::encode(&frame).expect("encode"), expected);
    assert_eq!(FrameCodec::decode(&expected).expect("decode"), frame);
    let mut corrupt = expected;
    corrupt[6] = 0;
    assert!(FrameCodec::decode(&corrupt).is_err());
    assert!(FrameCodec::decode(&expected[..10]).is_err());
    assert!(SyncMessage::decode(&[5, 0]).is_err());
}
