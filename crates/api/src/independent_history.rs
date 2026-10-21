use std::io;

use axum::{Json, extract::State, http::StatusCode};
use serde_json::Value;
use subtle::ConstantTimeEq;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{IndependentCreateControl, LocalTls};

impl IndependentCreateControl {
    pub async fn request_history(
        &self,
        source: &str,
        approval: Value,
        cancellation: &CancellationToken,
    ) -> io::Result<Value> {
        crate::exact_https(source)?;
        let nonce = crate::independent_exchange::random_capability()?;
        let (sender, mut receiver) = mpsc::channel(1);
        {
            let mut state = self.0.lifecycle.lock().await;
            if state
                .intent
                .as_ref()
                .map(serde_json::to_value)
                .transpose()?
                .as_ref()
                != Some(&approval["request"]["operation"])
                || state
                    .active
                    .as_ref()
                    .is_none_or(CancellationToken::is_cancelled)
                || cancellation.is_cancelled()
            {
                return Err(io::Error::other(
                    "history requires the active exact admitted Join",
                ));
            }
            state.history = Some((nonce.clone(), approval.clone(), sender));
        }
        let body =
            serde_json::json!({"approval":approval,"recipient":self.owner(),"return_nonce":nonce});
        match self.owner() {
            "idp" => {
                let configuration = of_idp_client::apis::configuration::Configuration {
                    base_path: source.to_owned(),
                    client: LocalTls::verification_client()?,
                    ..Default::default()
                };
                of_idp_client::apis::independent_join_api::independent_join_request_history(
                    &configuration,
                    of_idp_client::apis::independent_join_api::IndependentJoinRequestHistoryParams {
                        history_request: serde_json::from_value(body)?,
                    },
                )
                .await
                .map_err(|_| io::Error::other("source public history unavailable or denied"))?;
            }
            "management" => {
                let configuration = of_management_client::apis::configuration::Configuration {
                    base_path: source.to_owned(),
                    client: LocalTls::verification_client()?,
                    ..Default::default()
                };
                of_management_client::apis::independent_join_api::independent_join_request_history(
                    &configuration,
                    of_management_client::apis::independent_join_api::IndependentJoinRequestHistoryParams {
                        history_request: serde_json::from_value(body)?,
                    },
                )
                .await
                .map_err(|_| io::Error::other("source public history unavailable or denied"))?;
            }
            _ => return Err(io::Error::other("unsupported public history owner")),
        }
        let result = tokio::select! {
            biased;
            () = cancellation.cancelled() => Err(io::Error::new(io::ErrorKind::Interrupted, "history import cancelled")),
            challenge = receiver.recv() => challenge.ok_or_else(|| io::Error::other("history challenge unavailable")),
        };
        self.0.lifecycle.lock().await.history = None;
        result
    }
}

#[utoipa::path(post, tag = "default", path = "/independent/setup/independent/history", operation_id = "history", request_body = Object, security(()), description = "Worker-only return channel: active exact admitted Join, matching approval and constant-time return nonce, and a 64-character capability are required. Not OAuth authority.", responses((status = 202, description = "History capability accepted"), (status = 403, description = "Operation or return nonce denied"), (status = 409, description = "Return channel full")))]
pub(crate) async fn challenge(
    State(control): State<IndependentCreateControl>,
    Json(body): Json<Value>,
) -> StatusCode {
    let state = control.0.lifecycle.lock().await;
    if control.0.shutdown.is_cancelled()
        || state
            .active
            .as_ref()
            .is_none_or(CancellationToken::is_cancelled)
        || control.0.admission.is_retired().unwrap_or(true)
    {
        return StatusCode::FORBIDDEN;
    }
    let Some((nonce, approval, sender)) = &state.history else {
        return StatusCode::FORBIDDEN;
    };
    if body["return_nonce"]
        .as_str()
        .is_none_or(|value| !bool::from(nonce.as_bytes().ct_eq(value.as_bytes())))
        || &body["approval"] != approval
        || body["capability"].as_str().is_none_or(|value| {
            value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
    {
        return StatusCode::FORBIDDEN;
    }
    match sender.try_send(body) {
        Ok(()) => StatusCode::ACCEPTED,
        Err(_) => StatusCode::CONFLICT,
    }
}
