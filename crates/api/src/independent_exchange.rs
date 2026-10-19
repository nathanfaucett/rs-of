use std::{
    io,
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use crate::{IndependentCreateControl, IndependentCreateIntent, IndependentOwnerPublic, LocalTls};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Challenge {
    operation: IndependentCreateIntent,
    recipient: String,
    registration: serde_json::Value,
    challenge: String,
}

pub(crate) struct OutstandingChallenge {
    bound: Challenge,
    expires: Instant,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Registration {
    operation: IndependentCreateIntent,
    recipient: String,
    registration: serde_json::Value,
    challenge: String,
}

pub(crate) fn random_capability() -> io::Result<String> {
    let mut random = [0u8; 32];
    getrandom::fill(&mut random).map_err(|_| io::Error::other("secure randomness unavailable"))?;
    Ok(random.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub(crate) fn routes() -> Router<IndependentCreateControl> {
    Router::new()
        .route("/setup/independent/public", get(public))
        .route("/setup/independent/challenge", post(challenge))
        .route("/setup/independent/register", post(register))
}

impl IndependentCreateControl {
    fn receipt_name(registration: &serde_json::Value) -> io::Result<String> {
        let id = registration["client_id"]
            .as_str()
            .ok_or_else(|| io::Error::other("missing receipt Client ID"))?;
        if id.len() != 36
            || !id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
            || registration.get("client_secret").is_some()
        {
            return Err(io::Error::other("invalid public registration receipt"));
        }
        Ok(format!("registration-{id}.json"))
    }
    pub fn registration_confirmed(
        &self,
        operation: &IndependentCreateIntent,
        registration: &serde_json::Value,
    ) -> io::Result<bool> {
        let path = self.directory().join(Self::receipt_name(registration)?);
        if !crate::setup_files::present(&path)? {
            return Ok(false);
        }
        let retained: serde_json::Value =
            serde_json::from_slice(&crate::setup_files::read_private(&path)?)?;
        if retained != serde_json::json!({"operation":operation,"registration":registration}) {
            return Err(io::Error::other("retained registration receipt changed"));
        }
        Ok(true)
    }
    fn confirm_registration(
        &self,
        operation: &IndependentCreateIntent,
        registration: &serde_json::Value,
    ) -> io::Result<()> {
        if self.registration_confirmed(operation, registration)? {
            return Ok(());
        }
        self.write_document(
            &Self::receipt_name(registration)?,
            &serde_json::json!({"operation":operation,"registration":registration}),
        )
    }

    /// Worker-only initiation. The capability is delivered only to the exact HTTPS recipient,
    /// never returned through an app-facing challenge-initiation route.
    pub async fn register_recipient(
        &self,
        recipient: &IndependentOwnerPublic,
        registration: &serde_json::Value,
    ) -> io::Result<()> {
        if self.owner() != "idp"
            || !["management", "storage"].contains(&recipient.owner.as_str())
            || !recipient.clients.contains(registration)
            || registration.get("client_secret").is_some()
            || registration["client_id"].as_str().is_none()
        {
            return Err(io::Error::other("invalid exact registration recipient"));
        }
        let bound = Challenge {
            operation: recipient.operation.clone(),
            recipient: recipient.owner.clone(),
            registration: registration.clone(),
            challenge: random_capability()?,
        };
        {
            let mut state = self.0.lifecycle.lock().await;
            if state.intent.as_ref() != Some(&bound.operation)
                || state
                    .active
                    .as_ref()
                    .is_none_or(|active| active.is_cancelled())
                || self.0.shutdown.is_cancelled()
            {
                return Err(io::Error::other(
                    "registration has no active exact setup operation",
                ));
            }
            state.challenges.retain(|entry| {
                !(entry.bound.recipient == bound.recipient
                    && entry.bound.registration == bound.registration)
            });
            state.challenges.push(OutstandingChallenge {
                bound: bound.clone(),
                expires: Instant::now() + Duration::from_secs(30),
            });
        }
        let result = LocalTls::verification_client()?
            .post(format!(
                "{}/setup/independent/challenge",
                bound.operation.base(&bound.recipient)?
            ))
            .json(&bound)
            .send()
            .await;
        let mut state = self.0.lifecycle.lock().await;
        state
            .challenges
            .retain(|entry| entry.bound.challenge != bound.challenge);
        match result {
            Ok(response) if response.status() == StatusCode::NO_CONTENT => Ok(()),
            _ => Err(io::Error::other(
                "exact HTTPS recipient registration did not complete",
            )),
        }
    }
}

async fn public(
    State(control): State<IndependentCreateControl>,
) -> Result<Json<IndependentOwnerPublic>, StatusCode> {
    let state = control.0.lifecycle.lock().await;
    state.public.clone().map(Json).ok_or(StatusCode::CONFLICT)
}

async fn challenge(
    State(control): State<IndependentCreateControl>,
    Json(bound): Json<Challenge>,
) -> StatusCode {
    let state = control.0.lifecycle.lock().await;
    if control.owner() == "idp"
        || control.0.admission.is_retired().unwrap_or(true)
        || state
            .active
            .as_ref()
            .is_none_or(|active| active.is_cancelled())
        || control.0.shutdown.is_cancelled()
        || state.intent.as_ref() != Some(&bound.operation)
        || bound.recipient != control.owner()
        || bound.challenge.len() != 64
        || !bound.challenge.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return StatusCode::FORBIDDEN;
    }
    let Some(public) = state.public.as_ref() else {
        return StatusCode::CONFLICT;
    };
    if !public.clients.contains(&bound.registration)
        || bound.registration.get("client_secret").is_some()
    {
        return StatusCode::FORBIDDEN;
    }
    let Some(id) = bound.registration["client_id"].as_str().map(str::to_owned) else {
        return StatusCode::BAD_REQUEST;
    };
    let Some(loader) = state.loader.clone() else {
        return StatusCode::CONFLICT;
    };
    let active = state
        .active
        .as_ref()
        .expect("validated active operation")
        .clone();
    drop(state);
    let load_id = id.clone();
    let secret = tokio::select! {
        biased;
        () = active.cancelled() => return StatusCode::FORBIDDEN,
        result = tokio::task::spawn_blocking(move || loader(&load_id)) => match result {
            Ok(Ok(secret)) => secret,
            _ => return StatusCode::SERVICE_UNAVAILABLE,
        },
    };
    let public_registration = bound.registration.clone();
    let mut registration = bound.registration;
    registration["client_secret"] = serde_json::Value::String(secret);
    let reply = Registration {
        operation: bound.operation.clone(),
        recipient: bound.recipient,
        registration,
        challenge: bound.challenge,
    };
    let client = match LocalTls::verification_client() {
        Ok(client) => client,
        Err(_) => return StatusCode::SERVICE_UNAVAILABLE,
    };
    let response = tokio::select! {
        biased;
        () = active.cancelled() => return StatusCode::FORBIDDEN,
        response = client
            .post(format!("{}/setup/independent/register", bound.operation.idp))
            .json(&reply)
            .send() => response,
    };
    let mut state = control.0.lifecycle.lock().await;
    if active.is_cancelled()
        || control.0.shutdown.is_cancelled()
        || control.0.admission.is_retired().unwrap_or(true)
        || state.intent.as_ref() != Some(&bound.operation)
        || state
            .active
            .as_ref()
            .is_none_or(|active| active.is_cancelled())
        || state
            .public
            .as_ref()
            .is_none_or(|public| !public.clients.contains(&public_registration))
    {
        return StatusCode::FORBIDDEN;
    }
    match response {
        Ok(response) if response.status() == StatusCode::NO_CONTENT => {
            if control
                .confirm_registration(&bound.operation, &public_registration)
                .is_err()
            {
                return StatusCode::SERVICE_UNAVAILABLE;
            }
            let public = state
                .public
                .as_mut()
                .expect("validated owner public evidence");
            if !public.registered.contains(&id) {
                public.registered.push(id);
            }
            StatusCode::NO_CONTENT
        }
        _ => StatusCode::FORBIDDEN,
    }
}

async fn register(
    State(control): State<IndependentCreateControl>,
    Json(mut reply): Json<Registration>,
) -> StatusCode {
    let mut state = control.0.lifecycle.lock().await;
    if control.owner() != "idp"
        || control.0.admission.is_retired().unwrap_or(true)
        || state
            .active
            .as_ref()
            .is_none_or(|active| active.is_cancelled())
        || control.0.shutdown.is_cancelled()
        || state.intent.as_ref() != Some(&reply.operation)
    {
        return StatusCode::FORBIDDEN;
    }
    let Some(secret) = reply
        .registration
        .as_object_mut()
        .and_then(|r| r.remove("client_secret"))
    else {
        return StatusCode::BAD_REQUEST;
    };
    if secret
        .as_str()
        .is_none_or(|secret| secret.len() != 64 || !secret.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return StatusCode::BAD_REQUEST;
    }
    let Some(index) = state.challenges.iter().position(|entry| {
        entry.expires > Instant::now()
            && entry.bound.challenge == reply.challenge
            && entry.bound.operation == reply.operation
            && entry.bound.recipient == reply.recipient
            && entry.bound.registration == reply.registration
    }) else {
        return StatusCode::FORBIDDEN;
    };
    state.challenges.remove(index);
    let Some(registrar) = state.registrar.clone() else {
        return StatusCode::CONFLICT;
    };
    let public_registration = reply.registration.clone();
    reply.registration["client_secret"] = secret;
    match registrar(reply.registration).await {
        Ok(()) => {
            if control
                .confirm_registration(&reply.operation, &public_registration)
                .is_ok()
            {
                StatusCode::NO_CONTENT
            } else {
                StatusCode::SERVICE_UNAVAILABLE
            }
        }
        Err(_) => StatusCode::CONFLICT,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::{Duration, Instant},
    };

    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tokio_util::sync::CancellationToken;
    use tower::ServiceExt;

    use super::{Challenge, OutstandingChallenge};
    use crate::{IndependentCreateIntent, IndependentSetupHost};

    #[tokio::test]
    async fn registration_consumes_only_the_exact_outstanding_capability() {
        let root = tempfile::tempdir().expect("isolated host");
        let setup = IndependentSetupHost::prepare(
            "idp",
            &root.path().join("idp"),
            "127.0.0.1:3000".parse().expect("address"),
            "https://localhost:5173",
            "",
            false,
        )
        .expect("prepare")
        .expect("fresh host");
        let admission = fs::read_to_string(setup.admission_path()).expect("protected admission");
        let code = admission.lines().nth(1).expect("host capability");
        let (host, mut commands) = setup.into_create(CancellationToken::new()).expect("host");
        let intent = IndependentCreateIntent {
            purpose: "create".into(),
            operation_nonce: "0123456789abcdef0123456789abcdef".into(),
            administrator: "operator".into(),
            node: "Independent Node".into(),
            idp: "https://127.0.0.1:3000".into(),
            management: "https://127.0.0.1:3001".into(),
            storage: "https://127.0.0.1:3002".into(),
            issuer: "https://127.0.0.1:3000".into(),
            peer: None,
        };
        let mut create = serde_json::to_value(&intent).expect("intent");
        create["password"] = serde_json::json!("protected-password");
        let response = host
            .router()
            .oneshot(
                Request::post("/setup/create")
                    .header("Content-Type", "application/json")
                    .header("Authorization", format!("Setup {code}"))
                    .body(Body::from(create.to_string()))
                    .expect("authorized request"),
            )
            .await
            .expect("acceptance");
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let _command = commands.recv().await.expect("host-owned command");
        let control = host.control();
        let registrations = Arc::new(AtomicUsize::new(0));
        let observed = registrations.clone();
        control
            .set_registrar(Arc::new(move |_| {
                observed.fetch_add(1, Ordering::SeqCst);
                Box::pin(async { Ok(()) })
            }))
            .await;
        let registration = serde_json::json!({"client_id":"00000000-0000-4000-8000-000000000001"});
        let bound = Challenge {
            operation: intent.clone(),
            recipient: "management".into(),
            registration: registration.clone(),
            challenge: "a".repeat(64),
        };
        control
            .0
            .lifecycle
            .lock()
            .await
            .challenges
            .push(OutstandingChallenge {
                bound: bound.clone(),
                expires: Instant::now() + Duration::from_secs(30),
            });
        let mut request = serde_json::to_value(&bound).expect("registration");
        request["registration"]["client_secret"] = serde_json::json!("b".repeat(64));
        for mutation in [
            "recipient",
            "operation",
            "registration",
            "challenge",
            "valid",
            "replay",
        ] {
            let mut body = request.clone();
            match mutation {
                "recipient" => body["recipient"] = serde_json::json!("storage"),
                "operation" => body["operation"]["node"] = serde_json::json!("competing"),
                "registration" => {
                    body["registration"]["client_id"] =
                        serde_json::json!("00000000-0000-4000-8000-000000000002")
                }
                "challenge" => body["challenge"] = serde_json::json!("c".repeat(64)),
                _ => (),
            }
            let response = host
                .router()
                .oneshot(
                    Request::post("/setup/independent/register")
                        .header("Content-Type", "application/json")
                        .body(Body::from(body.to_string()))
                        .expect("registration request"),
                )
                .await
                .expect("registration response");
            assert_eq!(
                response.status(),
                if mutation == "valid" {
                    StatusCode::NO_CONTENT
                } else {
                    StatusCode::FORBIDDEN
                },
                "{mutation}"
            );
        }
        assert_eq!(
            registrations.load(Ordering::SeqCst),
            1,
            "the exact capability can provision only once"
        );
    }
}
