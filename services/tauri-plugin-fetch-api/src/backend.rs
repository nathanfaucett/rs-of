use axum::body::Body;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tauri::Webview;
use tauri::{
    AppHandle, Manager, Runtime, State,
    http::{HeaderName, HeaderValue, Request, Response},
    ipc::{self, JavaScriptChannelId},
    plugin::{Builder, TauriPlugin},
};
use tokio::sync::Mutex;
use tokio_stream::wrappers::ReceiverStream;

pub type RequestBoxStream = Request<Body>;
pub type ResponseBoxStream = Response<Body>;

/// Type definition for the user-supplied asynchronous request handler signature.
/// Takes the Tauri AppHandle and an inbound HTTP Request containing the incoming byte stream,
/// and returns a Future that resolves to an HTTP Response containing the outbound byte stream.
pub type HandlerFn<R> = Arc<
    dyn Fn(
            AppHandle<R>,
            RequestBoxStream,
        ) -> Pin<Box<dyn Future<Output = ResponseBoxStream> + Send>>
        + Send
        + Sync,
>;

/// Internal state tracking active streaming channels for mapping incoming chunks
/// and managing live session cancellation states safely across thread boundaries.
type ReqSender = tokio::sync::mpsc::Sender<Result<Vec<u8>, std::io::Error>>;

#[derive(Default)]
pub struct PluginStreamState {
    pub active_streams: Arc<Mutex<HashSet<String>>>,
    pub upload_senders: Arc<Mutex<HashMap<String, ReqSender>>>,
}

/// Internal state wrapper to maintain reference to the registered user handler callback.
pub struct HandlerState<R: Runtime> {
    pub handler: HandlerFn<R>,
}

/// Incoming serialization payload context passed up by the JavaScript client wrapper layer.
#[derive(Deserialize)]
pub struct InitStreamRequest {
    method: String,
    uri: String,
    headers: Vec<(String, String)>,
    response_channel: JavaScriptChannelId,
}

/// A unified protocol frame sent over the response channel to the frontend.
/// This allows metadata (headers) and data chunks to share the same stream context asynchronously.
#[derive(Serialize, Clone)]
#[serde(tag = "type", content = "payload")]
pub enum StreamFrame {
    /// Sent once the backend handler receives the response head from the network target
    ResponseHead {
        status: u16,
        headers: Vec<(String, String)>,
    },
    /// Sent continuously as data blocks arrive from the response body stream
    Data(Vec<u8>),
    /// Sent if any network or protocol error occurs during processing
    Error(String),
    /// Sent when the stream completes successfully and cleanups are done
    Complete,
}

/// Outgoing metadata descriptor transmitted back to JavaScript immediately upon initialization.
#[derive(Serialize)]
pub struct InitStreamResponse {
    pub stream_id: String,
}

/// Tauri command allowing the JavaScript frontend engine to push sequential upload stream chunks.
#[tauri::command]
async fn upload_plugin_chunk(
    stream_id: String,
    chunk: Vec<u8>,
    is_eof: bool,
    state: State<'_, PluginStreamState>,
) -> Result<(), String> {
    let sender = {
        let senders = state.upload_senders.lock().await;
        senders.get(&stream_id).cloned()
    };

    if let Some(tx) = sender {
        if !chunk.is_empty() {
            tx.send(Ok(chunk)).await.map_err(|e| e.to_string())?;
        }

        if is_eof {
            let mut senders = state.upload_senders.lock().await;
            senders.remove(&stream_id);
        }
    }

    Ok(())
}

/// Tauri command to explicitly interrupt an active streaming operation via AbortController signals.
#[tauri::command]
async fn cancel_plugin_stream(
    stream_id: String,
    state: State<'_, PluginStreamState>,
) -> Result<(), String> {
    let mut active = state.active_streams.lock().await;
    active.remove(&stream_id);
    let mut senders = state.upload_senders.lock().await;
    senders.remove(&stream_id);
    Ok(())
}

/// Tauri command that initializes the request pipeline, hooks up handlers, and fires streaming tasks.
#[tauri::command]
async fn init_plugin_stream<R: Runtime>(
    request: InitStreamRequest,
    app_handle: AppHandle<R>,
    webview: Webview<R>,
    plugin_state: State<'_, PluginStreamState>,
    handler_state: State<'_, HandlerState<R>>,
) -> Result<ipc::Response, String> {
    // 1. Extract the raw numeric Channel ID and use it as the unified Stream ID key
    let res_channel = request.response_channel.channel_on(webview);
    let stream_id = res_channel.id().to_string();

    // 2. Create an in-memory channel to act as the pollable Request body stream source
    let (req_tx, req_rx) = tokio::sync::mpsc::channel::<Result<Vec<u8>, std::io::Error>>(32);
    let body_stream = ReceiverStream::new(req_rx);
    let boxed_req_body = Body::from_stream(body_stream);

    // Save channel sender to state registry so the upload command can map to it
    {
        let mut senders = plugin_state.upload_senders.lock().await;
        senders.insert(stream_id.clone(), req_tx);
        let mut active = plugin_state.active_streams.lock().await;
        active.insert(stream_id.clone());
    }

    // 3. Map client payload configurations into a native tauri::http::Request wrapper layout
    let mut http_req = Request::builder()
        .method(request.method.as_str())
        .uri(&request.uri);

    if let Some(headers_mut) = http_req.headers_mut() {
        for (k, v) in request.headers {
            if let (Ok(name), Ok(val)) = (
                HeaderName::from_bytes(k.as_bytes()),
                HeaderValue::from_bytes(v.as_bytes()),
            ) {
                headers_mut.insert(name, val);
            }
        }
    }

    let finalized_req = http_req.body(boxed_req_body).map_err(|e| e.to_string())?;

    // 4. Extract execution frame context states to move into the background worker task
    let handler = handler_state.handler.clone();
    let shared_active_registry = plugin_state.active_streams.clone();
    let task_stream_id = stream_id.clone();
    let res_channel_task = res_channel.clone();

    // 5. Detach an asynchronous task runner thread context to drive both the request and response pipelines
    tauri::async_runtime::spawn(async move {
        // Await user routing handler logic inside the spawned task.
        // This prevents blocking `init_plugin_stream` from returning the Stream ID back to JavaScript.
        let http_res = (handler)(app_handle, finalized_req).await;

        let status = http_res.status().as_u16();
        let mut out_headers = Vec::new();
        for (k, v) in http_res.headers() {
            out_headers.push((k.to_string(), v.to_str().unwrap_or("").to_string()));
        }

        // Emit the response head frames back through the channel so the frontend receives headers
        if res_channel_task
            .send(StreamFrame::ResponseHead {
                status,
                headers: out_headers,
            })
            .is_err()
        {
            let mut active = shared_active_registry.lock().await;
            active.remove(&task_stream_id);
            return; // Frontend channel dropped or page reloaded
        }

        // Stream output back to JavaScript sequentially
        let res_body = http_res.into_body();
        let mut res_body_stream = res_body.into_data_stream();

        while let Some(chunk_result) = res_body_stream.next().await {
            // Ensure cancellation signals are validated before writing data blocks
            {
                let active = shared_active_registry.lock().await;
                if !active.contains(&task_stream_id) {
                    break;
                }
            }

            match chunk_result {
                Ok(bytes) => {
                    if res_channel_task
                        .send(StreamFrame::Data(bytes.to_vec()))
                        .is_err()
                    {
                        break;
                    }
                }
                Err(e) => {
                    let _ = res_channel_task.send(StreamFrame::Error(e.to_string()));
                    break;
                }
            }
        }

        // Finalize state structures
        let _ = res_channel_task.send(StreamFrame::Complete);
        let mut active = shared_active_registry.lock().await;
        active.remove(&task_stream_id);
    });

    // 6. Return the mapped stream_id token immediately to JavaScript.
    // This allows the frontend to immediately invoke sequential data upload chunk commands.
    let serialized_head =
        serde_json::to_vec(&InitStreamResponse { stream_id }).map_err(|e| e.to_string())?;

    Ok(ipc::Response::new(serialized_head))
}

/// Exposes the fluent builder interface to register the streaming extension inside your Tauri project main context.
pub fn init<R: Runtime, F, Fut>(handler: F) -> TauriPlugin<R>
where
    F: Fn(AppHandle<R>, RequestBoxStream) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ResponseBoxStream> + Send + 'static,
{
    // Wrap generic function trait pointers securely inside thread-safe dynamic boxes
    let boxed_handler: HandlerFn<R> = Arc::new(move |app, req| Box::pin(handler(app, req)));

    Builder::<R>::new("fetch-api")
        .invoke_handler(tauri::generate_handler![
            init_plugin_stream,
            upload_plugin_chunk,
            cancel_plugin_stream
        ])
        .setup(move |app, _api| {
            app.manage(PluginStreamState::default());
            app.manage(HandlerState {
                handler: boxed_handler,
            });
            Ok(())
        })
        .build()
}
