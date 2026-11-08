from pathlib import Path
import json
import subprocess
import tempfile
import sys

package = Path(__file__).resolve().parents[1]
server = (package / "../../crates/storage-server/src/sync.rs").resolve()
source = server.read_text()
protocol = source[source.index("pub const MAX_FRAME_PAYLOAD"):source.index("pub enum SyncAuthError")]
# Keep the actual server types and codec, without its authentication code.
protocol = protocol[:protocol.rfind("#[derive")]
main = r'''
fn main() {
    let identity = StoreIdentity {
        store_id: Uuid::from_bytes([0x11; 16]),
        replica_id: Uuid::from_bytes([0x22; 16]),
        creation_epoch: 300,
        schema_fingerprint: [0x33; 32],
    };
    let emit_payload = |name: &str, payload: Vec<u8>| {
        let bytes = FrameCodec::encode(&Frame {version: 1, flags: 0, payload}).expect("encode frame");
        println!("{} {}", name, bytes.iter().map(|b| format!("{b:02x}")).collect::<String>());
    };
    let emit = |name: &str, message: SyncMessage| emit_payload(name, message.encode().expect("encode message"));
    emit_payload("invalidBoolean", vec![2, 2]);
    emit_payload("trailingPostcard", vec![5, 0]);
    emit_payload("wrongClientAbortTag", vec![9, 0]);
    let mut max_identity = identity.clone();
    max_identity.creation_epoch = u64::MAX;
    emit("maxEpochHello", SyncMessage::Hello(Hello {protocol_version: 1, manifest: vec![], store_identity: max_identity.clone(), supported_protocols: vec![ProtocolId::OfdbSqlV7]}));
    emit("maxEpochAttach", SyncMessage::AttachRequest(AttachRequest {store_identity: max_identity, auth_token: AuthToken("idp-token-é".as_bytes().to_vec()), direction: AttachDirection::Push}));
    emit("hello", SyncMessage::Hello(Hello {protocol_version: 1, manifest: vec![], store_identity: identity.clone(), supported_protocols: vec![ProtocolId::OfdbSqlV7]}));
    for (name, direction) in [("Push", AttachDirection::Push), ("Pull", AttachDirection::Pull), ("Bidirectional", AttachDirection::Bidirectional)] {
        emit(&format!("attach{name}"), SyncMessage::AttachRequest(AttachRequest {store_identity: identity.clone(), auth_token: AuthToken("idp-token-é".as_bytes().to_vec()), direction}));
        emit(&format!("accept{name}"), SyncMessage::AttachResponse(AttachResponse {accepted: true, backend_replica_id: Uuid::from_bytes([0x44; 16]), session_id: Uuid::from_bytes([0x55; 16]), granted_direction: direction, max_frame_bytes: MAX_FRAME_PAYLOAD, max_session_bytes: MAX_SESSION_BYTES}));
    }
    for (name, max_frame_bytes, max_session_bytes) in [("maxLimits", u32::MAX, u64::MAX), ("tinyFrame", 8, 1024), ("tinySession", MAX_FRAME_PAYLOAD, 1), ("budget", MAX_FRAME_PAYLOAD, 512)] {
        emit(name, SyncMessage::AttachResponse(AttachResponse {accepted: true, backend_replica_id: Uuid::from_bytes([0x44; 16]), session_id: Uuid::from_bytes([0x55; 16]), granted_direction: AttachDirection::Bidirectional, max_frame_bytes, max_session_bytes}));
    }
    emit("rejected", SyncMessage::AttachResponse(AttachResponse {accepted: false, backend_replica_id: Uuid::nil(), session_id: Uuid::nil(), granted_direction: AttachDirection::Push, max_frame_bytes: MAX_FRAME_PAYLOAD, max_session_bytes: MAX_SESSION_BYTES}));
    emit("unauthorized", SyncMessage::Abort(AbortReason::Unauthorized));
    emit("unavailable", SyncMessage::Abort(AbortReason::BackendUnavailable));
    emit("malformed", SyncMessage::Abort(AbortReason::MalformedMessage));
    emit("internal", SyncMessage::Abort(AbortReason::InternalError("private server details".into())));
    emit("ping", SyncMessage::Ping);
    emit("pong", SyncMessage::Pong);
    emit("largeOwner", SyncMessage::OwnerMessage {protocol_id: ProtocolId::OfdbSqlV7, payload: vec![7; 7]});
    emit("owner", SyncMessage::OwnerMessage {protocol_id: ProtocolId::OfdbSqlV7, payload: vec![7, 8, 9]});
}
'''
with tempfile.TemporaryDirectory(prefix="of-sync-wire-") as directory:
    root = Path(directory)
    (root / "src").mkdir()
    (root / "Cargo.toml").write_text('''[package]
name = "of-sync-wire-fixtures"
version = "0.1.0"
edition = "2024"
[dependencies]
serde = { version = "1.0", default-features = false, features = ["derive", "std"] }
uuid = { version = "1.0", default-features = false, features = ["serde"] }
postcard = { version = "1.1", default-features = false, features = ["alloc"] }
crc32fast = { version = "1.0", default-features = false }
''')
    (root / "src/main.rs").write_text("use serde::{Deserialize, Serialize};\nuse uuid::Uuid;\nuse crc32fast::Hasher;\n" + protocol + main)
    result = subprocess.run(["cargo", "run", "--quiet", "--offline", "--manifest-path", str(root / "Cargo.toml")], capture_output=True, text=True)
    if result.returncode:
        raise SystemExit(result.stderr)
    fixtures = dict(line.split() for line in result.stdout.splitlines())
    output = json.dumps(fixtures, indent=4) + "\n"
    destination = package / "src/wire-fixtures.json"
    if "--check" in sys.argv:
        if destination.read_text() != output:
            raise SystemExit("Server wire fixtures changed; run pnpm fixtures and review compatibility")
    else:
        destination.write_text(output)
