use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, header::AUTHORIZATION},
};
use idp_model::{
    contract::{ErrorCode, ErrorResponse},
    model::Id,
};
use management_service::replica::{
    OwnerPolicyObservation, SelectedResource as StoredSelectedResource,
};
use model::contract::{
    MANAGEMENT_REPLICATION_ADMIT_SCOPE, MANAGEMENT_REPLICATION_READ_SCOPE, OwnerCopyCleanup,
    PrincipalType, ReplicationAdmissionRequest, SelectedResourcesResponse, StandardClaims,
};

use crate::router::RouterState;

#[utoipa::path(
    get,
    path = "/replication/devices/{endpoint_id}/selections",
    params(("endpoint_id" = String, Path, description = "Iroh endpoint public key")),
    responses(
        (status = OK, description = "Selected resources", body = SelectedResourcesResponse),
        (status = 401, description = "Invalid bearer token"),
        (status = 403, description = "Token lacks replication-read permission"),
        (status = 503, description = "Authorization authority unavailable")
    ),
    security(("authorization" = []))
)]
pub(crate) async fn selected_resources(
    State(state): State<RouterState>,
    Path(endpoint_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<SelectedResourcesResponse>, ErrorResponse> {
    let started = std::time::Instant::now();
    let (authorized, identity) = tokio::join!(
        authorize_storage(&state, &headers, MANAGEMENT_REPLICATION_READ_SCOPE),
        async {
            state
                .control_plane
                .get_storage_endpoint_identity(&endpoint_id)
                .await
                .map_err(|error| {
                    log::warn!("Management endpoint identity lookup failed: {error}");
                    unavailable()
                })
        }
    );
    authorized?;
    let identity = identity?;
    log::debug!(
        "Management policy phase=authentication-identity elapsed_ms={}",
        started.elapsed().as_millis()
    );

    let observation = state
        .selection_policies
        .observe_owner_prevalidated(identity.device_id, &identity.owner_subject)
        .await
        .map_err(|_| unavailable())?;
    let (pending_cleanup, admin_allowed, resources) = match observation {
        OwnerPolicyObservation::PendingCleanup {
            cleanup_id,
            old_owner_subject,
        } => (
            Some(OwnerCopyCleanup {
                cleanup_id: cleanup_id.to_string(),
                device_id: identity.device_id.to_string(),
                endpoint_id: identity.endpoint_id.clone(),
                old_owner_subject,
                new_owner_subject: identity.owner_subject.clone(),
            }),
            false,
            vec![],
        ),
        ready => {
            let (allowed, resources) = crate::runtime::replication_policy_from_observation(ready)
                .map_err(|_| unavailable())?;
            (None, allowed, resources)
        }
    };
    log::debug!(
        "Management policy phase=selection elapsed_ms={}",
        started.elapsed().as_millis()
    );
    if state
        .control_plane
        .get_storage_endpoint_identity(&endpoint_id)
        .await
        .map_err(|_| unavailable())?
        != identity
    {
        return Err(denied());
    }
    log::debug!(
        "Management policy phase=recheck elapsed_ms={}",
        started.elapsed().as_millis()
    );
    Ok(Json(SelectedResourcesResponse {
        device_id: identity.device_id.to_string(),
        endpoint_id: identity.endpoint_id,
        owner_subject: identity.owner_subject,
        pending_cleanup,
        admin_allowed,
        resources,
    }))
}

#[utoipa::path(
    post,
    path = "/replication/admission",
    request_body = ReplicationAdmissionRequest,
    responses(
        (status = 204, description = "Replication admitted"),
        (status = 403, description = "Replication denied"),
        (status = 401, description = "Invalid bearer token"),
        (status = 503, description = "Authorization authority unavailable")
    ),
    security(("authorization" = []))
)]
pub(crate) async fn replication_admission(
    State(state): State<RouterState>,
    headers: HeaderMap,
    Json(request): Json<ReplicationAdmissionRequest>,
) -> Result<axum::http::StatusCode, ErrorResponse> {
    authorize_storage(&state, &headers, MANAGEMENT_REPLICATION_ADMIT_SCOPE).await?;
    let application_id = request
        .application_id
        .parse::<Id>()
        .map_err(|_| ErrorResponse::new(ErrorCode::InvalidRequest))?;
    let resource_id = request
        .resource_id
        .parse::<Id>()
        .map_err(|_| ErrorResponse::new(ErrorCode::InvalidRequest))?;
    if !matches!(request.kind.as_str(), "database" | "filesystem")
        || request.operation != "synchronize"
    {
        return Err(denied());
    }
    let source = state
        .control_plane
        .get_storage_endpoint_identity(&request.source_endpoint_id)
        .await
        .map_err(|_| unavailable())?;
    let target = state
        .control_plane
        .get_storage_endpoint_identity(&request.target_endpoint_id)
        .await
        .map_err(|_| unavailable())?;
    if source.device_id == target.device_id || source.owner_subject != target.owner_subject {
        return Err(denied());
    }

    for identity in [&source, &target] {
        if !state
            .selection_policies
            .reconcile_owner_prevalidated(identity.device_id, &identity.owner_subject)
            .await
            .map_err(|_| unavailable())?
        {
            return Err(denied());
        }
        let policy = state
            .selection_policies
            .get(identity.device_id, &identity.owner_subject, application_id)
            .await
            .map_err(|_| unavailable())?;
        if !policy.is_some_and(|policy| policy.admin_allowed) {
            return Err(denied());
        }
        let selected = state
            .selection_policies
            .selected_resources_for_device(identity.device_id)
            .await
            .map_err(|_| unavailable())?;
        if !resource_is_selected(
            &selected,
            identity.device_id,
            &identity.owner_subject,
            application_id,
            &request.kind,
            resource_id,
        ) {
            return Err(denied());
        }
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/replication/owner-cleanup",
    request_body = OwnerCopyCleanup,
    responses(
        (status = 204, description = "Exact owner cleanup acknowledged"),
        (status = 401, description = "Invalid bearer token"),
        (status = 403, description = "Stale binding or insufficient service permission"),
        (status = 503, description = "Authorization authority unavailable")
    ),
    security(("authorization" = []))
)]
pub(crate) async fn owner_cleanup(
    State(state): State<RouterState>,
    headers: HeaderMap,
    Json(request): Json<OwnerCopyCleanup>,
) -> Result<axum::http::StatusCode, ErrorResponse> {
    authorize_storage(&state, &headers, MANAGEMENT_REPLICATION_ADMIT_SCOPE).await?;
    let identity = state
        .control_plane
        .get_storage_endpoint_identity(&request.endpoint_id)
        .await
        .map_err(|_| unavailable())?;
    if identity.device_id.to_string() != request.device_id
        || identity.owner_subject != request.new_owner_subject
        || !request
            .old_owner_subject
            .parse::<Id>()
            .is_ok_and(|owner| !owner.is_nil())
        || !state
            .selection_policies
            .complete_owner_cleanup_bound(
                identity.device_id,
                &request.old_owner_subject,
                &request.new_owner_subject,
                &request.endpoint_id,
                request.cleanup_id.parse::<Id>().map_err(|_| denied())?,
            )
            .await
            .map_err(|_| unavailable())?
    {
        return Err(denied());
    }
    if state
        .control_plane
        .get_storage_endpoint_identity(&request.endpoint_id)
        .await
        .map_err(|_| unavailable())?
        != identity
    {
        return Err(denied());
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn authorize_storage(
    state: &RouterState,
    headers: &HeaderMap,
    required_scope: &str,
) -> Result<(), ErrorResponse> {
    let token = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|token| !token.is_empty())
        .ok_or_else(|| ErrorResponse::new(ErrorCode::NotAuthorized))?;
    let (claims, _) = state
        .control_plane
        .validate_actor_token(token)
        .await
        .map_err(|error| {
            if error == "actor token was rejected by IdP" {
                ErrorResponse::new(ErrorCode::NotAuthorized)
            } else {
                unavailable()
            }
        })?;
    if !replication_client_authorized(&claims, &state.storage_audience, required_scope) {
        return Err(denied());
    }
    Ok(())
}

fn replication_client_authorized(
    claims: &StandardClaims,
    storage_audience: &str,
    required_scope: &str,
) -> bool {
    claims.principal_type == PrincipalType::Client
        && claims.aud == storage_audience
        && claims.scope.iter().any(|scope| scope == required_scope)
}

fn resource_is_selected(
    resources: &[StoredSelectedResource],
    device_id: Id,
    owner_subject: &str,
    application_id: Id,
    kind: &str,
    resource_id: Id,
) -> bool {
    resources.iter().any(|resource| {
        resource.device_id == device_id
            && resource.owner_subject == owner_subject
            && resource.application_id == application_id
            && resource.kind == kind
            && resource.resource_id == resource_id
    })
}

fn denied() -> ErrorResponse {
    ErrorResponse::new(ErrorCode::AccessDenied)
}

fn unavailable() -> ErrorResponse {
    ErrorResponse::new(ErrorCode::TemporarilyUnavailable)
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "cli")]
    use std::{
        fs,
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        path::Path,
        sync::Arc,
        thread,
    };

    #[cfg(feature = "cli")]
    use axum::{
        extract::{Path as AxumPath, State},
        http::{HeaderMap, HeaderValue, header::AUTHORIZATION},
    };
    #[cfg(feature = "cli")]
    use idp_model::contract::IntrospectionResponse;
    use idp_model::model::Id;
    use management_service::replica::SelectedResource as StoredSelectedResource;
    #[cfg(feature = "cli")]
    use management_service::{
        HostedControlPlane, ManagementService,
        replica::{DbPermissionRepo, DbRoleRepo, DbSelectionPolicyRepo},
    };
    use model::contract::{
        MANAGEMENT_REPLICATION_ADMIT_SCOPE, MANAGEMENT_REPLICATION_READ_SCOPE, PrincipalType,
        StandardClaims, TokenType, TokenUse,
    };

    #[cfg(feature = "cli")]
    use crate::router::RouterState;

    #[cfg(feature = "cli")]
    use super::selected_resources;
    use super::{replication_client_authorized, resource_is_selected};

    #[cfg(feature = "cli")]
    #[tokio::test]
    async fn selections_return_current_authenticated_device_policy() {
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

        use axum::{
            Json,
            body::{Body, to_bytes},
            http::{Request, StatusCode},
            routing::{get, post},
        };
        use management_service::replica::{SelectionPolicy, up};
        use model::contract::SelectedResourcesResponse;
        use tower::ServiceExt;

        let device = Id::now_v7();
        let owner = Id::now_v7().to_string();
        let application = Id::now_v7();
        let resource = Id::now_v7();
        let endpoint =
            "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a".to_owned();
        let root =
            std::env::temp_dir().join(format!("management-current-policy-{}.redb", Id::now_v7()));
        let engine = Arc::new(db::Database::open(&root).expect("open policy"));
        up(&engine).await.expect("initialize policy");
        engine.client().translate_and_execute(&format!("INSERT INTO devices (id, owner_subject, name, public_key, address, state, approved_at, revoked_at, created_at, updated_at) VALUES (CAST('{device}' AS UUID), '{owner}', 'test', '{endpoint}', '', 1, 1, NULL, 1, 1)"), &db::SqlTranslator).await.expect("Management Device");
        let policies = Arc::new(DbSelectionPolicyRepo::new(engine.clone()));

        let owner_active = Arc::new(AtomicBool::new(true));
        let active = owner_active.clone();
        let publication_race = Arc::new(AtomicUsize::new(0));
        let active_reads = Arc::new(AtomicUsize::new(0));
        let race = publication_race.clone();
        let reads = active_reads.clone();
        let race_engine = engine.clone();
        let idp = axum::Router::new()
            .route("/oauth2/token", post(|body: String| async move {
                assert!(body.contains("client_secret=management-secret"));
                Json(serde_json::json!({"access_token":"management-token", "token_type":"Bearer", "expires_in":300,
                    "scope":"idp.token.validate idp.user.lookup", "iss":"https://issuer.example"}))
            }))
            .route("/oauth2/introspect", post(|headers: HeaderMap, body: String| async move {
                assert_eq!(headers[AUTHORIZATION], "Bearer management-token");
                assert!(body.contains("storage-token"));
                assert!(!body.contains("management-secret"));
                Json(IntrospectionResponse {
                    claims: StandardClaims {
                        r#type: TokenType::Bearer, r#use: TokenUse::Access, exp: i64::MAX, iat: 0, nbf: 0,
                        iss: "https://issuer.example".into(), aud: "storage-api".into(),
                        client_id: "storage-client".into(), sub: "00000000-0000-0000-0000-000000000002".into(),
                        principal_type: PrincipalType::Client, resource: None, authorization_details: None,
                        scope: vec![MANAGEMENT_REPLICATION_READ_SCOPE.into()],
                    },
                    application_id: Id::from_u128(1).to_string(),
                })
            }))
            .route("/users/{user}/active", get(move |headers: HeaderMap| {
                let active = active.clone();
                let race = race.clone();
                let reads = reads.clone();
                let engine = race_engine.clone();
                async move {
                    assert_eq!(headers[AUTHORIZATION], "Bearer management-token");
                    let read = reads.fetch_add(1, Ordering::SeqCst);
                    match (race.load(Ordering::SeqCst), read) {
                        (1, 1) => return StatusCode::NOT_FOUND,
                        (2, 0) => {
                            engine.client().translate_and_execute(&format!("UPDATE devices SET revoked_at = 9 WHERE id = CAST('{device}' AS UUID)"), &db::SqlTranslator).await.expect("revoked during initial owner check");
                        }
                        (3, 0) => {
                            engine.client().translate_and_execute(&format!("UPDATE devices SET owner_subject = '{}' WHERE id = CAST('{device}' AS UUID)", Id::now_v7()), &db::SqlTranslator).await.expect("transferred during initial owner check");
                        }
                        _ => {}
                    }
                    if active.load(Ordering::SeqCst) { StatusCode::NO_CONTENT } else { StatusCode::NOT_FOUND }
                }
            }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind IdP");
        let base = format!("http://{}", listener.local_addr().expect("IdP address"));
        let server =
            tokio::spawn(async move { axum::serve(listener, idp).await.expect("serve IdP") });
        let state = RouterState::new(
            "http://management.example",
            Arc::new(ManagementService::new(
                DbPermissionRepo::new(engine.clone()),
                DbRoleRepo::new(engine.clone()),
            )),
            policies.clone(),
            Arc::new(
                HostedControlPlane::new_with_services(&base, &base, "https://issuer.example")
                    .expect("control plane")
                    .with_idp_service_client("management-client", "management-secret", "idp-api")
                    .expect("authenticated IdP relationship")
                    .with_local_devices(Arc::new(management_service::replica::DbDeviceRepo::new(
                        engine.clone(),
                    ))),
            ),
            "storage-api",
        );
        let router = crate::router::openapi_router(state, "/")
            .split_for_parts()
            .0;
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/replication/devices/{endpoint}/selections"))
                    .header(AUTHORIZATION, "Bearer storage-token")
                    .body(Body::empty())
                    .expect("missing policy request"),
            )
            .await
            .expect("read missing policy");
        assert_eq!(
            response.status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "missing policy must fail like the local owner evaluator"
        );
        policies
            .set_prevalidated(SelectionPolicy {
                device_id: device,
                owner_subject: owner.clone(),
                application_id: None,
                selected_kind: None,
                selected_id: None,
                admin_allowed: true,
            })
            .await
            .expect("allow Device");
        policies
            .set_prevalidated(SelectionPolicy {
                device_id: device,
                owner_subject: owner.clone(),
                application_id: Some(application),
                selected_kind: Some("database".into()),
                selected_id: Some(resource),
                admin_allowed: true,
            })
            .await
            .expect("select database");
        for step in 0..7 {
            match step {
                1 => {
                    policies
                        .set_admin_allowed_prevalidated(device, &owner, false)
                        .await
                        .expect("restrict Device");
                }
                2 => {
                    policies
                        .set_admin_allowed_prevalidated(device, &owner, true)
                        .await
                        .expect("allow Device");
                    policies
                        .deselect_owned(device, &owner)
                        .await
                        .expect("deselect current resources");
                }
                3 => {
                    owner_active.store(false, Ordering::SeqCst);
                }
                4 => {
                    owner_active.store(true, Ordering::SeqCst);
                    engine.client().translate_and_execute(&format!("UPDATE devices SET owner_subject = '{}' WHERE id = CAST('{device}' AS UUID)", Id::now_v7()), &db::SqlTranslator).await.expect("learn Management transfer");
                }
                5 => {
                    engine.client().translate_and_execute(&format!("UPDATE devices SET revoked_at = 9 WHERE id = CAST('{device}' AS UUID)"), &db::SqlTranslator).await.expect("learn timestamp revocation");
                }
                6 => {
                    engine.client().translate_and_execute(&format!("UPDATE devices SET owner_subject = '{owner}', revoked_at = NULL WHERE id = CAST('{device}' AS UUID)"), &db::SqlTranslator).await.expect("retained cleanup still denies");
                }
                _ => {}
            }
            let response = router
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(format!("/replication/devices/{endpoint}/selections"))
                        .header(AUTHORIZATION, "Bearer storage-token")
                        .body(Body::empty())
                        .expect("read policy request"),
                )
                .await
                .expect("read current policy");
            if matches!(step, 3 | 5) {
                assert!(
                    !response.status().is_success(),
                    "owner change/revocation/pending cleanup must deny"
                );
                continue;
            }
            assert_eq!(response.status(), StatusCode::OK);
            let body = to_bytes(response.into_body(), 65536)
                .await
                .expect("policy response");
            let policy: SelectedResourcesResponse =
                serde_json::from_slice(&body).expect("Device policy contract");
            assert_eq!(policy.device_id, device.to_string());
            assert_eq!(policy.endpoint_id, endpoint);
            if matches!(step, 4 | 6) {
                let cleanup = policy
                    .pending_cleanup
                    .expect("authorized service sees durable fence");
                assert_eq!(cleanup.device_id, device.to_string());
                assert_eq!(cleanup.endpoint_id, endpoint);
                assert_eq!(cleanup.old_owner_subject, owner);
                assert_eq!(cleanup.new_owner_subject, policy.owner_subject);
                assert!(!policy.admin_allowed);
                assert!(policy.resources.is_empty());
                continue;
            }
            assert!(policy.pending_cleanup.is_none());
            assert_eq!(policy.owner_subject, owner);
            assert_eq!(policy.admin_allowed, step != 1);
            assert_eq!(policy.resources.len(), usize::from(step != 2));
            if let Some(selected) = policy.resources.first() {
                assert_eq!(selected.owner_subject, owner);
                assert_eq!(selected.application_id, application.to_string());
                assert_eq!(selected.kind, "database");
                assert_eq!(selected.resource_id, resource.to_string());
            }
        }
        for mode in 1..=3 {
            owner_active.store(true, Ordering::SeqCst);
            engine.client().translate_and_execute(&format!("UPDATE devices SET owner_subject = '{owner}', revoked_at = NULL WHERE id = CAST('{device}' AS UUID)"), &db::SqlTranslator).await.expect("restore approved owner for publication race");
            active_reads.store(0, Ordering::SeqCst);
            publication_race.store(mode, Ordering::SeqCst);
            let response = router
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(format!("/replication/devices/{endpoint}/selections"))
                        .header(AUTHORIZATION, "Bearer storage-token")
                        .body(Body::empty())
                        .expect("publication race request"),
                )
                .await
                .expect("publication race response");
            assert!(
                !response.status().is_success(),
                "final owner recheck must reject race {mode}"
            );
        }
        server.abort();
        let _ = server.await;
        drop(router);
        drop(policies);
        drop(engine);
        fs::remove_file(root).expect("remove isolated policy database");
    }

    #[test]
    fn replication_authorization_requires_client_storage_audience_and_exact_scope() {
        let mut claims = StandardClaims {
            r#type: TokenType::Bearer,
            r#use: TokenUse::Access,
            exp: i64::MAX,
            iat: 0,
            nbf: 0,
            iss: "issuer".into(),
            aud: "storage-service".into(),
            client_id: "storage".into(),
            sub: "storage".into(),
            principal_type: PrincipalType::Client,
            resource: None,
            authorization_details: None,
            scope: vec![
                MANAGEMENT_REPLICATION_READ_SCOPE.into(),
                MANAGEMENT_REPLICATION_ADMIT_SCOPE.into(),
            ],
        };
        assert!(replication_client_authorized(
            &claims,
            "storage-service",
            MANAGEMENT_REPLICATION_READ_SCOPE
        ));
        assert!(replication_client_authorized(
            &claims,
            "storage-service",
            MANAGEMENT_REPLICATION_ADMIT_SCOPE
        ));
        assert!(!replication_client_authorized(
            &claims,
            "other-service",
            MANAGEMENT_REPLICATION_READ_SCOPE
        ));
        claims.principal_type = PrincipalType::User;
        assert!(!replication_client_authorized(
            &claims,
            "storage-service",
            MANAGEMENT_REPLICATION_READ_SCOPE
        ));
        claims.principal_type = PrincipalType::Client;
        claims.scope = vec!["management.replication.reader".into()];
        assert!(!replication_client_authorized(
            &claims,
            "storage-service",
            MANAGEMENT_REPLICATION_READ_SCOPE
        ));
    }

    #[test]
    fn selected_resource_match_is_bound_to_device_owner_application_kind_and_id() {
        let device_id = parse_id("00000000-0000-0000-0000-000000000001");
        let application_id = parse_id("00000000-0000-0000-0000-000000000002");
        let resource_id = parse_id("00000000-0000-0000-0000-000000000003");
        let selected = [StoredSelectedResource {
            device_id,
            owner_subject: "owner".to_owned(),
            application_id,
            kind: "database".to_owned(),
            resource_id,
        }];

        assert!(resource_is_selected(
            &selected,
            device_id,
            "owner",
            application_id,
            "database",
            resource_id,
        ));
        assert!(!resource_is_selected(
            &selected,
            device_id,
            "other-owner",
            application_id,
            "database",
            resource_id,
        ));
        assert!(!resource_is_selected(
            &selected,
            device_id,
            "owner",
            application_id,
            "filesystem",
            resource_id,
        ));
    }

    #[cfg(feature = "cli")]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn live_listener_rejects_unscoped_client_after_idp_introspection() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind IdP test server");
        let address = listener.local_addr().expect("read IdP test address");
        let claims = StandardClaims {
            r#type: TokenType::Bearer,
            r#use: TokenUse::Access,
            exp: i64::MAX,
            iat: 0,
            nbf: 0,
            iss: "https://issuer.example".to_owned(),
            aud: "storage-api".to_owned(),
            client_id: "storage-client".to_owned(),
            sub: "00000000-0000-0000-0000-000000000002".to_owned(),
            principal_type: PrincipalType::Client,
            resource: None,
            authorization_details: None,
            scope: vec![MANAGEMENT_REPLICATION_ADMIT_SCOPE.to_owned()],
        };
        let introspection = serde_json::to_string(&IntrospectionResponse {
            claims,
            application_id: "00000000-0000-0000-0000-000000000001".to_owned(),
        })
        .expect("serialize IdP introspection response");
        let http_server = thread::spawn(move || {
            let (mut token_stream, _) = listener.accept().expect("accept token request");
            let mut token_request = [0; 4096];
            let token_length = token_stream
                .read(&mut token_request)
                .expect("read token request");
            let token_request =
                String::from_utf8_lossy(&token_request[..token_length]).into_owned();
            let token_response = r#"{"access_token":"management-token","token_type":"Bearer","expires_in":300,"scope":"idp.token.validate idp.user.lookup","iss":"https://issuer.example"}"#;
            write!(token_stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{token_response}", token_response.len()).expect("write token response");

            let (mut introspection_stream, _) =
                listener.accept().expect("accept introspection request");
            let mut introspection_request = [0; 4096];
            let introspection_length = introspection_stream
                .read(&mut introspection_request)
                .expect("read introspection request");
            let introspection_request =
                String::from_utf8_lossy(&introspection_request[..introspection_length])
                    .into_owned();
            write!(introspection_stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{introspection}", introspection.len()).expect("write introspection response");
            (token_request, introspection_request)
        });

        let root =
            std::env::temp_dir().join(format!("management-replication-auth-{}", Id::now_v7()));
        fs::create_dir_all(&root).expect("create test database directory");
        let engine = Arc::new(
            db::Database::open(Path::new(&root).join("management.redb"))
                .expect("open management test engine"),
        );
        let base_url = format!("http://{address}");
        let control_plane = Arc::new(
            HostedControlPlane::new_with_services(&base_url, &base_url, "https://issuer.example")
                .expect("create hosted control plane")
                .with_idp_service_client("management-client", "management-secret", "idp-audience")
                .expect("configure IdP service client"),
        );
        let state = RouterState::new(
            "http://management.example",
            Arc::new(ManagementService::new(
                DbPermissionRepo::new(Arc::clone(&engine)),
                DbRoleRepo::new(Arc::clone(&engine)),
            )),
            Arc::new(DbSelectionPolicyRepo::new(Arc::clone(&engine))),
            control_plane,
            "storage-api",
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind Management test listener");
        let management_address = listener.local_addr().expect("read Management address");
        let router = crate::router::openapi_router(state.clone(), "/")
            .split_for_parts()
            .0;
        let management_server = tokio::spawn(async move {
            axum::serve(listener, router)
                .await
                .expect("serve Management test router");
        });
        let response = thread::spawn(move || {
            let mut stream = TcpStream::connect(management_address)
                .expect("connect to Management listener");
            write!(
                stream,
                "GET /replication/devices/endpoint-key/selections HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer storage-token\r\nConnection: close\r\n\r\n"
            )
            .expect("write Management request");
            let mut response = String::new();
            stream
                .read_to_string(&mut response)
                .expect("read Management response");
            response
        })
        .join()
        .expect("join Management request thread");
        assert!(response.starts_with("HTTP/1.1 403"), "{response}");
        management_server.abort();
        let _ = management_server.await;
        let (token_request, introspection_request) =
            http_server.join().expect("join IdP test server");
        assert!(token_request.starts_with("POST /oauth2/token HTTP/1.1"));
        assert!(introspection_request.starts_with("POST /oauth2/introspect HTTP/1.1"));
        assert!(introspection_request.contains("storage-token"));
        assert!(
            introspection_request
                .to_ascii_lowercase()
                .contains("authorization: bearer management-token")
        );

        drop(state);
        drop(engine);
        fs::remove_dir_all(root).expect("remove test database directory");
    }

    #[cfg(feature = "cli")]
    #[tokio::test]
    async fn selections_route_rejects_user_principal_and_wrong_audience() {
        for (principal_type, audience) in [
            (PrincipalType::User, "storage-api"),
            (PrincipalType::Client, "other-api"),
        ] {
            let claims = StandardClaims {
                r#type: TokenType::Bearer,
                r#use: TokenUse::Access,
                exp: i64::MAX,
                iat: 0,
                nbf: 0,
                iss: "https://issuer.example".to_owned(),
                aud: audience.to_owned(),
                client_id: "storage-client".to_owned(),
                sub: "00000000-0000-0000-0000-000000000002".to_owned(),
                principal_type,
                resource: None,
                authorization_details: None,
                scope: vec![MANAGEMENT_REPLICATION_READ_SCOPE.to_owned()],
            };
            verify_selection_rejection(Some(claims)).await;
        }
    }

    #[cfg(feature = "cli")]
    #[tokio::test]
    async fn selections_route_rejects_token_denied_by_idp() {
        verify_selection_rejection(None).await;
    }

    #[cfg(feature = "cli")]
    async fn verify_selection_rejection(claims: Option<StandardClaims>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind IdP test server");
        let address = listener.local_addr().expect("read IdP test address");
        let token_was_introspected = claims.is_some();
        let introspection = claims
            .map(|claims| {
                serde_json::to_string(&IntrospectionResponse {
                    claims,
                    application_id: "00000000-0000-0000-0000-000000000001".to_owned(),
                })
                .expect("serialize IdP introspection response")
            })
            .unwrap_or_else(|| r#"{"error":"invalid_token"}"#.to_owned());
        let status = if token_was_introspected {
            "200 OK"
        } else {
            "401 Unauthorized"
        };
        let http_server = thread::spawn(move || {
            let (mut token_stream, _) = listener.accept().expect("accept token request");
            let mut token_request = [0; 4096];
            token_stream
                .read(&mut token_request)
                .expect("read token request");
            let token_response = r#"{"access_token":"management-token","token_type":"Bearer","expires_in":300,"scope":"idp.token.validate idp.user.lookup","iss":"https://issuer.example"}"#;
            write!(token_stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{token_response}", token_response.len()).expect("write token response");

            let (mut introspection_stream, _) =
                listener.accept().expect("accept introspection request");
            let mut introspection_request = [0; 4096];
            introspection_stream
                .read(&mut introspection_request)
                .expect("read introspection request");
            write!(introspection_stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{introspection}", introspection.len()).expect("write introspection response");
        });

        let root =
            std::env::temp_dir().join(format!("management-replication-rejected-{}", Id::now_v7()));
        fs::create_dir_all(&root).expect("create test database directory");
        let engine = Arc::new(
            db::Database::open(Path::new(&root).join("management.redb"))
                .expect("open management test engine"),
        );
        let base_url = format!("http://{address}");
        let control_plane = Arc::new(
            HostedControlPlane::new_with_services(&base_url, &base_url, "https://issuer.example")
                .expect("create hosted control plane")
                .with_idp_service_client("management-client", "management-secret", "idp-audience")
                .expect("configure IdP service client"),
        );
        let state = RouterState::new(
            "http://management.example",
            Arc::new(ManagementService::new(
                DbPermissionRepo::new(Arc::clone(&engine)),
                DbRoleRepo::new(Arc::clone(&engine)),
            )),
            Arc::new(DbSelectionPolicyRepo::new(Arc::clone(&engine))),
            control_plane,
            "storage-api",
        );
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_static("Bearer storage-token"),
        );

        let error = selected_resources(
            State(state.clone()),
            AxumPath("endpoint-key".to_owned()),
            headers,
        )
        .await
        .expect_err("unauthorized token must not authorize selection reads");
        let expected_error = if token_was_introspected {
            idp_model::contract::ErrorCode::AccessDenied
        } else {
            idp_model::contract::ErrorCode::NotAuthorized
        };
        assert_eq!(error.error, expected_error);
        http_server.join().expect("join IdP test server");

        drop(state);
        drop(engine);
        fs::remove_dir_all(root).expect("remove test database directory");
    }

    fn parse_id(value: &str) -> Id {
        value.parse().expect("valid fixture ID")
    }
}
