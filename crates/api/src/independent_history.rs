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
        let response = LocalTls::verification_client()?.post(format!("{source}/node-joins/history/request")).json(&serde_json::json!({"approval":approval,"recipient":self.owner(),"return_nonce":nonce})).send().await.map_err(|_| io::Error::other("source public history unavailable"))?;
        if response.status() != StatusCode::ACCEPTED {
            return Err(io::Error::other("source public history denied"));
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
