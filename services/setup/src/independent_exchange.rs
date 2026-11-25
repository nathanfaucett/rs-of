use std::{
    io,
    time::{Duration, Instant},
};

use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};

use crate::independent_create::IndependentCreateControl;
use crate::independent_operation::IndependentCreateIntent;
use crate::independent_operation::IndependentOwnerPublic;

#[derive(Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Challenge {
    operation: IndependentCreateIntent,
    recipient: String,
    registration: serde_json::Value,
    challenge: String,
}

pub struct OutstandingChallenge {
    bound: Challenge,
    expires: Instant,
}

#[derive(Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Registration {
    operation: IndependentCreateIntent,
    recipient: String,
    registration: serde_json::Value,
    challenge: String,
}

pub fn random_capability() -> io::Result<String> {
    let mut random = [0u8; 32];
    getrandom::fill(&mut random).map_err(|_| io::Error::other("secure randomness unavailable"))?;
    Ok(random.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub fn routes() -> utoipa_axum::router::OpenApiRouter<IndependentCreateControl> {
    utoipa_axum::router::OpenApiRouter::new()
        .routes(utoipa_axum::routes!(public))
        .routes(utoipa_axum::routes!(challenge))
        .routes(utoipa_axum::routes!(register))
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

    /// Worker-only initiation. The capability is delivered only to the exact recipient,
    /// never returned through an app-facing challenge-initiation route.
    pub async fn register_recipient(
        &self,
        recipient: &IndependentOwnerPublic,
        registration: &serde_json::Value,
    ) -> io::Result<()> {
        let _observation = crate::independent_create::RequestObservation::start(
            self.owner(),
            "register_recipient",
        );
        if self.owner() != "idp"
            || !["device", "storage"].contains(&recipient.owner.as_str())
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
        let local = self.local_peer(&bound.operation, &bound.recipient)?;
        {
            let mut state = self.0.lifecycle.lock().await;
            if state.intent.as_ref() != Some(&bound.operation)
                || state
                    .active
                    .as_ref()
                    .is_none_or(tokio_util::sync::CancellationToken::is_cancelled)
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
        };
        let (delivered, failure) = if let Some(peer) = local {
            (
                challenge(State(peer), Json(bound.clone())).await == StatusCode::NO_CONTENT,
                "exact local recipient registration did not complete",
            )
        } else {
            let configuration = of_idp_client::apis::configuration::Configuration {
                base_path: bound.operation.base(&bound.recipient)?.to_owned(),
                client: self.http_client(),
                ..Default::default()
            };
            (
                of_idp_client::apis::owner_setup_api::owner_setup_challenge(
                    &configuration,
                    of_idp_client::apis::owner_setup_api::OwnerSetupChallengeParams {
                        challenge: serde_json::from_value(serde_json::to_value(&bound)?)?,
                    },
                )
                .await
                .is_ok(),
                "exact HTTPS recipient registration did not complete",
            )
        };
        let mut state = self.0.lifecycle.lock().await;
        state
            .challenges
            .retain(|entry| entry.bound.challenge != bound.challenge);
        if delivered {
            Ok(())
        } else {
            Err(io::Error::other(failure))
        }
    }
}

#[utoipa::path(get, tag = "default", path = "/setup/owner/public", operation_id = "owner_public", responses((status = 200, body = IndependentOwnerPublic, description = "Operation-bound public owner metadata, not setup authorization"), (status = 409, description = "Public owner metadata not yet available")))]
pub async fn public(
    State(control): State<IndependentCreateControl>,
) -> Result<Json<IndependentOwnerPublic>, StatusCode> {
    let state = control.0.lifecycle.lock().await;
    state.public.clone().map(Json).ok_or(StatusCode::CONFLICT)
}

#[utoipa::path(post, tag = "default", path = "/setup/owner/challenge", operation_id = "challenge", request_body = Challenge, security(()), description = "Worker handoff over verified HTTPS. Requires an active exact Setup operation and operation-bound recipient challenge; discovery grants no authority. Never log capability or registration credentials.", responses((status = 204, description = "Exact handoff accepted"), (status = 400, description = "Invalid registration"), (status = 403, description = "Operation or challenge denied"), (status = 409, description = "Owner not ready"), (status = 503, description = "Owner unavailable")))]
pub async fn challenge(
    State(control): State<IndependentCreateControl>,
    Json(bound): Json<Challenge>,
) -> StatusCode {
    let state = control.0.lifecycle.lock().await;
    if control.owner() == "idp"
        || control.0.admission.is_retired().unwrap_or(true)
        || state
            .active
            .as_ref()
            .is_none_or(tokio_util::sync::CancellationToken::is_cancelled)
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
    let registered = match control.local_peer(&bound.operation, "idp") {
        Ok(Some(peer)) => tokio::select! {
            biased;
            () = active.cancelled() => return StatusCode::FORBIDDEN,
            response = register(State(peer), Json(reply)) => response == StatusCode::NO_CONTENT,
        },
        Ok(None) => {
            let configuration = of_idp_client::apis::configuration::Configuration {
                base_path: bound.operation.idp.clone(),
                client: control.http_client(),
                ..Default::default()
            };
            let registration = match serde_json::to_value(reply).and_then(serde_json::from_value) {
                Ok(registration) => registration,
                Err(_) => return StatusCode::SERVICE_UNAVAILABLE,
            };
            tokio::select! {
                biased;
                () = active.cancelled() => return StatusCode::FORBIDDEN,
                response = of_idp_client::apis::owner_setup_api::owner_setup_register(&configuration,
                    of_idp_client::apis::owner_setup_api::OwnerSetupRegisterParams { registration }) => response.is_ok(),
            }
        }
        Err(_) => return StatusCode::FORBIDDEN,
    };
    let mut state = control.0.lifecycle.lock().await;
    if active.is_cancelled()
        || control.0.shutdown.is_cancelled()
        || control.0.admission.is_retired().unwrap_or(true)
        || state.intent.as_ref() != Some(&bound.operation)
        || state
            .active
            .as_ref()
            .is_none_or(tokio_util::sync::CancellationToken::is_cancelled)
        || state
            .public
            .as_ref()
            .is_none_or(|public| !public.clients.contains(&public_registration))
    {
        return StatusCode::FORBIDDEN;
    }
    if !registered {
        return StatusCode::FORBIDDEN;
    }
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

#[utoipa::path(post, tag = "default", path = "/setup/owner/register", operation_id = "register", request_body = Registration, security(()), description = "Worker handoff over verified HTTPS. Requires an active exact Setup operation and operation-bound recipient challenge; discovery grants no authority. Never log capability or registration credentials.", responses((status = 204, description = "Exact handoff accepted"), (status = 400, description = "Invalid registration"), (status = 403, description = "Operation or challenge denied"), (status = 409, description = "Owner not ready"), (status = 503, description = "Owner unavailable")))]
pub async fn register(
    State(control): State<IndependentCreateControl>,
    Json(mut reply): Json<Registration>,
) -> StatusCode {
    let _observation =
        crate::independent_create::RequestObservation::start(control.owner(), "register");
    let mut state = control.0.lifecycle.lock().await;
    if std::env::var_os("OF_SETUP_TIMEOUT_TIMING").is_some() {
        eprintln!(
            "seam=owner_registration owner={} pid={} phase=lifecycle_acquired",
            control.owner(),
            std::process::id()
        );
    }
    if control.owner() != "idp"
        || control.0.admission.is_retired().unwrap_or(true)
        || state
            .active
            .as_ref()
            .is_none_or(tokio_util::sync::CancellationToken::is_cancelled)
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
    let active = state
        .active
        .as_ref()
        .expect("validated active operation")
        .clone();
    drop(state);
    let public_registration = reply.registration.clone();
    reply.registration["client_secret"] = secret;
    let result = crate::independent_create::observe_request(
        registrar(reply.registration),
        control.owner(),
        "registrar",
    )
    .await;
    if std::env::var_os("OF_SETUP_TIMEOUT_TIMING").is_some() {
        eprintln!(
            "seam=owner_registration owner={} pid={} phase=registrar_end error_kind={:?}",
            control.owner(),
            std::process::id(),
            result.as_ref().err().map(io::Error::kind)
        );
    }
    let state = control.0.lifecycle.lock().await;
    if active.is_cancelled()
        || control.0.shutdown.is_cancelled()
        || control.0.admission.is_retired().unwrap_or(true)
        || state.intent.as_ref() != Some(&reply.operation)
        || state
            .active
            .as_ref()
            .is_none_or(tokio_util::sync::CancellationToken::is_cancelled)
    {
        return StatusCode::FORBIDDEN;
    }
    match result {
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
#[path = "independent_exchange_lifecycle_tests.rs"]
mod lifecycle_tests;

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

    use super::Challenge;
    use super::OutstandingChallenge;
    use crate::independent_create::IndependentCreateControl;
    use crate::independent_operation::{IndependentCreateIntent, IndependentOwnerPublic};
    use crate::independent_setup::IndependentSetupHost;

    #[tokio::test]
    async fn local_exchange_uses_exact_live_peers_without_listeners() {
        let root = tempfile::tempdir().expect("isolated owners");
        let prepare = |owner, address: &str| {
            IndependentSetupHost::prepare(
                owner,
                &root.path().join(owner),
                address.parse().expect("address"),
                "https://localhost:5173",
                "",
                false,
            )
            .expect("prepare")
            .expect("fresh owner")
            .into_create(CancellationToken::new())
            .expect("host")
            .0
        };
        let idp = prepare("idp", "127.0.0.1:3000");
        let device = prepare("device", "127.0.0.1:3001");
        let storage = prepare("storage", "127.0.0.1:3002");
        let [idp_control, device_control, storage_control] =
            [idp.control(), device.control(), storage.control()];
        assert!(
            IndependentCreateControl::connect_local(&[
                idp_control.clone(),
                device_control.clone(),
                device_control.clone(),
            ])
            .is_err()
        );
        IndependentCreateControl::connect_local(&[
            storage_control.clone(),
            idp_control.clone(),
            device_control.clone(),
        ])
        .expect("connect exact local owners");

        let intent = IndependentCreateIntent {
            purpose: "create".into(),
            operation_nonce: "0123456789abcdef0123456789abcdef".into(),
            administrator: "operator".into(),
            node: "Local Node".into(),
            idp: "https://127.0.0.1:3000".into(),
            device: "https://127.0.0.1:3001".into(),
            storage: "https://127.0.0.1:3002".into(),
            issuer: "https://127.0.0.1:3000".into(),
        };
        for control in [&idp_control, &device_control, &storage_control] {
            let mut lifecycle = control.0.lifecycle.lock().await;
            lifecycle.intent = Some(intent.clone());
            lifecycle.active = Some(control.shutdown_token().child_token());
        }
        let client_id = "00000000-0000-4000-8000-000000000001";
        let registration = serde_json::json!({"client_id": client_id});
        for control in [&device_control, &storage_control] {
            control
                .set_public(
                    IndependentOwnerPublic {
                        operation: intent.clone(),
                        owner: control.owner().into(),
                        clients: vec![registration.clone()],
                        facts: serde_json::json!({}),
                        registered: vec![],
                    },
                    Arc::new(move |id| {
                        assert_eq!(id, client_id);
                        Ok("b".repeat(64))
                    }),
                )
                .await
                .expect("publish metadata");
        }
        idp_control
            .set_registrar(Arc::new(move |registration| {
                assert_eq!(registration["client_id"], client_id);
                assert_eq!(registration["client_secret"], "b".repeat(64));
                Box::pin(async { Ok(()) })
            }))
            .await;

        let mut wrong_base = intent.clone();
        wrong_base.device = "https://127.0.0.1:3003".into();
        assert_eq!(
            idp_control
                .peer(&wrong_base, "device")
                .await
                .err()
                .expect("wrong base")
                .kind(),
            std::io::ErrorKind::InvalidInput
        );
        let discovery = idp_control
            .peer(&intent, "device")
            .await
            .expect("local discovery");
        assert_eq!(discovery.clients, [registration.clone()]);
        idp_control
            .register_recipient(&discovery, &registration)
            .await
            .expect("local handoff");
        assert!(device_control.registered(client_id).await);
        for control in [&idp_control, &device_control] {
            assert!(
                control
                    .registration_confirmed(&intent, &registration)
                    .expect("valid receipt")
            );
        }

        let storage_public = idp_control
            .peer(&intent, "storage")
            .await
            .expect("local discovery");
        storage_control.unfinished().await;
        assert!(
            idp_control
                .register_recipient(&storage_public, &registration)
                .await
                .is_err()
        );
        assert!(
            !storage_control
                .registration_confirmed(&intent, &registration)
                .expect("no late receipt")
        );
        drop((device, device_control));
        assert_eq!(
            idp_control
                .peer(&intent, "device")
                .await
                .err()
                .expect("dropped local owner")
                .kind(),
            std::io::ErrorKind::NotConnected
        );
    }

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
            device: "https://127.0.0.1:3001".into(),
            storage: "https://127.0.0.1:3002".into(),
            issuer: "https://127.0.0.1:3000".into(),
        };
        let mut create = serde_json::to_value(&intent).expect("intent");
        create["password"] = serde_json::json!("protected-password");
        let response = host
            .router()
            .oneshot(
                Request::post("/setup/owner/create")
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
            recipient: "device".into(),
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
                        serde_json::json!("00000000-0000-4000-8000-000000000002");
                }
                "challenge" => body["challenge"] = serde_json::json!("c".repeat(64)),
                _ => (),
            }
            let response = host
                .router()
                .oneshot(
                    Request::post("/setup/owner/register")
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
