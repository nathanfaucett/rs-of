use std::{
    collections::BTreeMap,
    future::Future,
    io,
    path::PathBuf,
    pin::Pin,
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use db::{Database, Value};
use futures_util::{SinkExt, StreamExt};
use ofdb_sql::{
    SessionConfig, SyncCandidateView, SyncDirection, SyncRole, SyncTransport,
    SyncValidationDirection,
};
use serde::{Deserialize, Serialize};
use tokio::{net::TcpStream, sync::Mutex};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream,
    tungstenite::{Message as ClientMessage, client::IntoClientRequest},
};
use tokio_util::sync::CancellationToken;

use crate::independent_join::IndependentJoinApproval;

pub type CheckJoinHistory = Arc<
    dyn Fn(IndependentJoinApproval) -> Pin<Box<dyn Future<Output = io::Result<()>> + Send>>
        + Send
        + Sync,
>;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum JoinHistoryContext {
    Idp,
    Management,
}
impl JoinHistoryContext {
    fn role(self) -> &'static str {
        match self {
            Self::Idp => "idp",
            Self::Management => "management",
        }
    }
    fn tables(self) -> Vec<&'static str> {
        match self {
            Self::Idp => idp_service::security_sync::public_security_tables()
                .iter()
                .map(|table| table.name())
                .collect(),
            Self::Management => crate::PUBLIC_POLICY_TABLES.to_vec(),
        }
    }
    fn permits(self, table: &str, column: &str, value: &Value, issuer: &str) -> bool {
        match self {
            Self::Idp => {
                idp_service::security_sync::public_security_tables()
                    .iter()
                    .find(|public| public.name() == table)
                    .is_some_and(|public| public.permits_historical_value(column, value))
                    && (!matches!(
                        (table, column),
                        ("idp_replica_members", "issuer" | "installation_id")
                            | ("idp_replica_signers", "issuer")
                    ) || matches!(value, Value::Null)
                        || matches!(value, Value::Text(text) if text == issuer))
            }
            Self::Management => {
                crate::public_policy_columns(table).is_some_and(|columns| columns.contains(&column))
                    && (table != "devices"
                        || crate::security_candidates::permits_device_value(column, value))
            }
        }
    }
}

#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
struct HistoryRequest {
    approval: IndependentJoinApproval,
    recipient: String,
    return_nonce: String,
}
struct PendingHistory {
    approval: IndependentJoinApproval,
    expires: Instant,
}

#[derive(Clone)]
pub struct JoinHistorySource {
    engine: Arc<Database>,
    context: JoinHistoryContext,
    staging: PathBuf,
    check: CheckJoinHistory,
    pending: Arc<Mutex<BTreeMap<String, PendingHistory>>>,
    cancellation: CancellationToken,
}
impl JoinHistorySource {
    pub fn new(
        engine: Arc<Database>,
        context: JoinHistoryContext,
        staging: PathBuf,
        check: CheckJoinHistory,
        cancellation: CancellationToken,
    ) -> Self {
        Self {
            engine,
            context,
            staging,
            check,
            pending: Arc::new(Mutex::new(BTreeMap::new())),
            cancellation,
        }
    }
    pub fn router(&self) -> Router {
        Router::new()
            .route("/node-joins/history/request", post(request))
            .route("/node-joins/history/stream", get(stream))
            .with_state(self.clone())
    }
}

fn capability() -> io::Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| io::Error::other("secure randomness unavailable"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}
#[cfg_attr(feature = "utoipa", utoipa::path(post, path = "/node-joins/history/request", operation_id = "request_history", request_body = HistoryRequest, security(()), description = "Requires verified exact independent approval and current owner permission. Returns a capability only through the operation-bound recipient callback.", responses((status = 202, description = "Callback accepted"), (status = 400, description = "Invalid recipient"), (status = 403, description = "Approval denied"), (status = 503, description = "Unavailable"))))]
async fn request(
    State(source): State<JoinHistorySource>,
    Json(body): Json<HistoryRequest>,
) -> StatusCode {
    if source.cancellation.is_cancelled()
        || body.recipient != source.context.role()
        || body.return_nonce.len() != 64
        || !body
            .return_nonce
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || body.approval.verify().is_err()
        || (source.check)(body.approval.clone()).await.is_err()
    {
        return StatusCode::FORBIDDEN;
    }
    let Ok(nonce) = capability() else {
        return StatusCode::SERVICE_UNAVAILABLE;
    };
    let base = match body.approval.request.operation.base(&body.recipient) {
        Ok(base) => base.to_owned(),
        Err(_) => return StatusCode::BAD_REQUEST,
    };
    {
        let mut pending = source.pending.lock().await;
        pending.retain(|_, entry| entry.expires > Instant::now());
        if pending.len() >= 64 {
            return StatusCode::SERVICE_UNAVAILABLE;
        }
        pending.insert(
            nonce.clone(),
            PendingHistory {
                approval: body.approval.clone(),
                expires: Instant::now() + Duration::from_secs(30),
            },
        );
    }
    let result = async {
        let configuration = of_idp_independent_client::apis::configuration::Configuration {
            base_path: base, client: api::LocalTls::verification_client()?, ..Default::default()
        };
        of_idp_independent_client::apis::default_api::history(&configuration,
            of_idp_independent_client::apis::default_api::HistoryParams {
                body: serde_json::json!({"approval":body.approval,"return_nonce":body.return_nonce,"capability":nonce}),
            }).await.map_err(|_| io::Error::other("history callback denied"))
    }.await;
    if result.is_ok() {
        StatusCode::ACCEPTED
    } else {
        source.pending.lock().await.remove(&nonce);
        StatusCode::FORBIDDEN
    }
}
#[cfg_attr(feature = "utoipa", utoipa::path(get, path = "/node-joins/history/stream", operation_id = "history_stream", params(("Authorization" = String, Header, description = "History <single-use operation-bound capability>"), ("Upgrade" = String, Header), ("Connection" = String, Header), ("Sec-WebSocket-Key" = String, Header), ("Sec-WebSocket-Version" = String, Header)), security(()), description = "WebSocket upgrade only. The subsequent bounded binary SQL history protocol is not implemented by generated HTTP clients.", responses((status = 101, description = "Switching to bounded history WebSocket protocol"), (status = 400, description = "Invalid WebSocket handshake"), (status = 401, description = "History capability required"), (status = 403, description = "Capability expired or denied"))))]
async fn stream(
    State(source): State<JoinHistorySource>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    let Some(nonce) = headers
        .get("Authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("History "))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Some(pending) = source.pending.lock().await.remove(nonce) else {
        return StatusCode::FORBIDDEN.into_response();
    };
    if pending.expires <= Instant::now()
        || source.cancellation.is_cancelled()
        || (source.check)(pending.approval.clone()).await.is_err()
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    upgrade
        .max_message_size(1024 * 1024)
        .max_frame_size(1024 * 1024)
        .on_upgrade(move |socket| async move {
            let mut transport = HistorySocket::Server(Box::new(socket));
            let _ = synchronize(
                &source.engine,
                &mut transport,
                source.context,
                &pending.approval,
                source.staging.clone(),
                SyncDirection::SendOnly,
                source.check.clone(),
                &source.cancellation,
            )
            .await;
        })
}

enum HistorySocket {
    Server(Box<WebSocket>),
    Client(Box<WebSocketStream<MaybeTlsStream<TcpStream>>>),
}
impl SyncTransport for HistorySocket {
    type Error = io::Error;
    async fn send(&mut self, frame: Vec<u8>) -> io::Result<()> {
        if frame.len() > 1024 * 1024 {
            return Err(io::Error::other("history frame too large"));
        }
        match self {
            Self::Server(socket) => socket
                .send(Message::Binary(frame.into()))
                .await
                .map_err(io::Error::other),
            Self::Client(socket) => socket
                .send(ClientMessage::Binary(frame.into()))
                .await
                .map_err(io::Error::other),
        }
    }
    async fn receive(&mut self) -> io::Result<Vec<u8>> {
        loop {
            let frame = match self {
                Self::Server(socket) => match socket.next().await {
                    Some(Ok(Message::Binary(frame))) => Some(frame.to_vec()),
                    Some(Ok(Message::Ping(_) | Message::Pong(_))) => None,
                    _ => return Err(io::Error::other("history stream closed")),
                },
                Self::Client(socket) => match socket.next().await {
                    Some(Ok(ClientMessage::Binary(frame))) => Some(frame.to_vec()),
                    Some(Ok(ClientMessage::Ping(_) | ClientMessage::Pong(_))) => None,
                    _ => return Err(io::Error::other("history stream closed")),
                },
            };
            if let Some(frame) = frame {
                if frame.len() > 1024 * 1024 {
                    return Err(io::Error::other("history frame too large"));
                }
                return Ok(frame);
            }
        }
    }
}

pub async fn import_join_history(
    engine: Arc<Database>,
    context: JoinHistoryContext,
    approval: &IndependentJoinApproval,
    capability: &str,
    staging: PathBuf,
    cancellation: &CancellationToken,
) -> io::Result<()> {
    approval.verify()?;
    let base = match context {
        JoinHistoryContext::Idp => &approval.request.source.idp,
        JoinHistoryContext::Management => &approval.request.source.management,
    };
    let client = api::LocalTls::verification_client()?;
    let key = tokio_tungstenite::tungstenite::handshake::client::generate_key();
    let request = match context {
        JoinHistoryContext::Idp => {
            let configuration = of_independent_idp_client::apis::configuration::Configuration {
                base_path: base.clone(),
                client,
                ..Default::default()
            };
            of_independent_idp_client::apis::default_api::history_stream_request(
                &configuration,
                of_independent_idp_client::apis::default_api::HistoryStreamParams {
                    authorization: format!("History {capability}"),
                    upgrade: "websocket".to_owned(),
                    connection: "Upgrade".to_owned(),
                    sec_web_socket_key: key,
                    sec_web_socket_version: "13".to_owned(),
                },
            )
            .map_err(io::Error::other)?
        }
        JoinHistoryContext::Management => {
            let configuration =
                of_independent_management_client::apis::configuration::Configuration {
                    base_path: base.clone(),
                    client,
                    ..Default::default()
                };
            of_independent_management_client::apis::default_api::history_stream_request(
                &configuration,
                of_independent_management_client::apis::default_api::HistoryStreamParams {
                    authorization: format!("History {capability}"),
                    upgrade: "websocket".to_owned(),
                    connection: "Upgrade".to_owned(),
                    sec_web_socket_key: key,
                    sec_web_socket_version: "13".to_owned(),
                },
            )
            .map_err(io::Error::other)?
        }
    };
    let mut url = request.url().clone();
    url.set_scheme("wss")
        .map_err(|_| io::Error::other("invalid history WebSocket URL"))?;
    let mut handshake = url
        .as_str()
        .into_client_request()
        .map_err(io::Error::other)?;
    handshake.headers_mut().extend(request.headers().clone());
    let (socket, _) = tokio::select! { biased; () = cancellation.cancelled() => return Err(io::Error::new(io::ErrorKind::Interrupted,"history cancelled")), result = tokio_tungstenite::connect_async(handshake) => result.map_err(|_| io::Error::other("authenticated public history stream unavailable"))? };
    let check: CheckJoinHistory = Arc::new(|approval| Box::pin(async move { approval.verify() }));
    let mut transport = HistorySocket::Client(Box::new(socket));
    synchronize(
        &engine,
        &mut transport,
        context,
        approval,
        staging,
        SyncDirection::ReceiveOnly,
        check,
        cancellation,
    )
    .await
}

async fn synchronize(
    engine: &Arc<Database>,
    transport: &mut HistorySocket,
    context: JoinHistoryContext,
    approval: &IndependentJoinApproval,
    staging: PathBuf,
    direction: SyncDirection,
    check: CheckJoinHistory,
    cancellation: &CancellationToken,
) -> io::Result<()> {
    let config = SessionConfig {
        direction,
        ..Default::default()
    };
    let tables = context.tables();
    let result = engine.synchronize_tables_with_stage_directory_candidate_validated_checked(
        transport,
        &config,
        if direction == SyncDirection::ReceiveOnly {
            SyncRole::Initiator
        } else {
            SyncRole::Responder
        },
        staging,
        &tables,
        |table, column, value| {
            if context.permits(table, column, value, &approval.request.operation.issuer) {
                Ok(())
            } else {
                Err("invalid owner public history".into())
            }
        },
        |view| {
            if (direction == SyncDirection::ReceiveOnly
                && view.direction == SyncValidationDirection::Outgoing)
                || (direction == SyncDirection::SendOnly
                    && view.direction == SyncValidationDirection::Incoming)
            {
                return Err("reverse public history denied".into());
            }
            validate_candidate(view, context, approval)
        },
        || {
            let check = check.clone();
            let approval = approval.clone();
            async move {
                if cancellation.is_cancelled() {
                    return Err("history cancelled".into());
                }
                check(approval)
                    .await
                    .map_err(|_| "current owner Join authorization denied".into())
            }
        },
    );
    tokio::select! { biased; () = cancellation.cancelled() => Err(io::Error::new(io::ErrorKind::Interrupted,"history cancelled")), result = result => result.map(|_| ()).map_err(|_| io::Error::other("validated owner public history import failed")) }
}

fn validate_candidate(
    view: &SyncCandidateView,
    context: JoinHistoryContext,
    approval: &IndependentJoinApproval,
) -> Result<(), String> {
    let candidate = crate::security_candidates::JoinCandidate {
        member_id: approval.request.member_id,
        device_id: approval.request.device_id,
        endpoint: &approval.request.endpoint_id,
        owner: approval.owner_subject,
        public_signer: &approval.request.public_signer,
        name: &approval.request.operation.node,
    };
    match context {
        JoinHistoryContext::Idp => crate::security_candidates::validate_node_join(
            view,
            &approval.request.operation.issuer,
            &candidate,
        ),
        JoinHistoryContext::Management => crate::security_candidates::validate_management(
            view,
            &[crate::security_candidates::PeerBinding {
                member: approval.request.device_id,
                endpoint: approval.request.endpoint_id.clone(),
                owner: approval.owner_subject,
            }],
            Some(&candidate),
        ),
    }
}

#[cfg(feature = "utoipa")]
#[derive(utoipa::OpenApi)]
#[openapi(paths(request, stream))]
struct HistoryDoc;
#[cfg(feature = "utoipa")]
pub fn history_openapi() -> utoipa::openapi::OpenApi {
    <HistoryDoc as utoipa::OpenApi>::openapi()
}
