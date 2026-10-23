use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};
use idp_model::{
    contract::{
        ApplicationRegistration, ClientProfile, ClientRegistration, ClientType, EntityType,
        GrantType, IdentityAction, IdentityResource, MANAGEMENT_PERMISSION_EVALUATE_SCOPE,
        PermissionAuditIdentity, PermissionEvaluationRequest, PermissionEvaluationResponse,
        PermissionSubject, PermissionTarget, TokenEndpointAuthMethod,
    },
    model::Id,
};
use idp_service::{oauth2::NodeSignerBinding, repo::ClientRepo};
use model::contract::PrincipalType;
use tower::ServiceExt;

use crate::{
    AppConfig, ManagementPermissionClient, ManagementPermissionConfig, RouterState,
    node_join_tests::{approval_fixture, user_token},
};

fn config(uri: &str) -> ManagementPermissionConfig {
    ManagementPermissionConfig {
        api_base_uri: uri.into(),
        client_id: "idp-evaluator".into(),
        client_secret: "protected-test-secret".into(),
    }
}

#[test]
fn permission_evaluator_configuration_requires_https_and_credentials() {
    for uri in [
        "http://management.test",
        "https://user:secret@management.test",
        "https://management.test?query=1",
        "https://management.test#fragment",
        "not a URL",
    ] {
        assert!(ManagementPermissionClient::new(&config(uri)).is_err());
    }
    assert!(
        ManagementPermissionClient::new(&config("https://management.test/management/")).is_ok()
    );
    let mut invalid = config("https://management.test");
    invalid.client_id.clear();
    assert!(ManagementPermissionClient::new(&invalid).is_err());
    invalid.client_id = "idp-evaluator".into();
    invalid.client_secret.clear();
    assert!(ManagementPermissionClient::new(&invalid).is_err());
    assert!(AppConfig::default().management_permissions.is_none());
    let configured = config("https://management.test");
    assert!(!format!("{configured:?}").contains("protected-test-secret"));
    assert!(
        !serde_json::to_string(&configured)
            .expect("public config")
            .contains("protected-test-secret")
    );
}

#[derive(Clone)]
struct Evaluator {
    idp: RouterState,
    actor: Id,
    mode: Arc<AtomicUsize>,
    calls: Arc<AtomicUsize>,
}

async fn evaluate(
    State(state): State<Evaluator>,
    headers: HeaderMap,
    Json(request): Json<PermissionEvaluationRequest>,
) -> Response {
    state.calls.fetch_add(1, Ordering::SeqCst);
    let bearer = headers
        .get("authorization")
        .expect("service authorization")
        .to_str()
        .expect("header text")
        .strip_prefix("Bearer ")
        .expect("bearer");
    let claims = state
        .idp
        .oauth2_service
        .validate_access_token(bearer, management_service::MANAGEMENT_APPLICATION_URI)
        .await
        .expect("real authenticated evaluator token");
    assert_eq!(claims.principal_type, PrincipalType::Client);
    assert_eq!(claims.client_id, "idp-evaluator");
    assert_eq!(claims.scope, [MANAGEMENT_PERMISSION_EVALUATE_SCOPE]);
    assert_eq!(request.subject, PermissionSubject::User { id: state.actor });
    assert_eq!(request.action, IdentityAction::ApplicationsRead);
    assert_eq!(
        request.target,
        PermissionTarget::Installation {
            resource: IdentityResource::Application { id: None }
        }
    );
    assert!(!request.request_id.is_nil());
    let mode = state.mode.load(Ordering::SeqCst);
    if mode == 8 {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    if mode == 9 {
        return (StatusCode::OK, "not JSON").into_response();
    }
    if mode == 10 {
        return (
            StatusCode::TEMPORARY_REDIRECT,
            [("location", "/management/permissions/evaluate")],
        )
            .into_response();
    }
    if mode == 11 {
        return StatusCode::FORBIDDEN.into_response();
    }

    if mode == 13 {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let mut response = PermissionEvaluationResponse {
        audit: PermissionAuditIdentity {
            service_subject: claims.sub.parse().expect("service subject"),
            service_client_id: claims.client_id,
            actor: request.subject.clone(),
        },
        request,
        allowed: mode != 1,
    };
    match mode {
        2 => response.request.request_id = Id::now_v7(),
        3 => response.request.subject = PermissionSubject::User { id: Id::now_v7() },
        4 => response.request.action = IdentityAction::ApplicationsCreate,
        5 => {
            response.request.target = PermissionTarget::Installation {
                resource: IdentityResource::Application {
                    id: Some(Id::now_v7()),
                },
            }
        }
        6 => response.audit.service_client_id = "other-client".into(),
        7 => response.audit.actor = PermissionSubject::User { id: Id::now_v7() },
        12 => response.audit.service_subject = Id::now_v7(),
        _ => {}
    }

    Json(response).into_response()
}

struct TlsConfig {
    root: PathBuf,
    address: SocketAddr,
}
impl api::LocalTlsConfig for TlsConfig {
    fn tls_directory(&self) -> PathBuf {
        self.root.clone()
    }
    fn tls_address(&self) -> SocketAddr {
        self.address
    }
}

async fn applications(state: RouterState, bearer: &str) -> StatusCode {
    let router: axum::Router = crate::openapi_router(state, "").into();
    router
        .oneshot(
            axum::http::Request::builder()
                .uri("/applications")
                .header("authorization", format!("Bearer {bearer}"))
                .body(axum::body::Body::empty())
                .expect("protected API request"),
        )
        .await
        .expect("API response")
        .status()
}

#[cfg(unix)]
#[tokio::test]
async fn independent_permission_http_binds_exact_request_and_fails_closed() {
    tokio::time::timeout(std::time::Duration::from_secs(120), async {
        let fixture = approval_fixture().await;
        let user = user_token(&fixture, "idp-services").await;
        let mut state = fixture.state.clone();
        state.local_permissions = None;
        assert_eq!(
            applications(state.clone(), &user).await,
            StatusCode::FORBIDDEN
        );
        let registration = ClientRegistration {
            application: ApplicationRegistration {
                name: Some("Management".into()),
                uri: management_service::MANAGEMENT_APPLICATION_URI.into(),
                description: None,
            },
            client_id: Some("idp-evaluator".into()),
            client_secret: Some("protected-test-secret".into()),
            client_id_issued_at: None,
            client_secret_expires_at: None,
            client_name: "IdP evaluator".into(),
            client_uri: None,
            logo_uri: None,
            contacts: vec![],
            terms_of_service_uri: None,
            policy_uri: None,
            client_type: ClientType::Confidential,
            profile: ClientProfile::Web,
            redirect_uris: vec![],
            allowed_grant_types: vec![GrantType::ClientCredentials],
            response_types: vec![],
            allowed_scopes: vec![MANAGEMENT_PERMISSION_EVALUATE_SCOPE.into()],
            allowed_audiences: vec![management_service::MANAGEMENT_APPLICATION_URI.into()],
            token_endpoint_auth_method: TokenEndpointAuthMethod::ClientSecretBasic,
            software_statement: None,
            software_id: None,
            software_version: None,
        };
        let client = state
            .oauth2_service
            .client_repo
            .create_client(registration)
            .await
            .expect("existing evaluator relationship");
        state
            .oauth2_service
            .key_service
            .ensure_entity_master_key(EntityType::Client, client.id, "protected-test-secret")
            .expect("Client master key");
        state
            .oauth2_service
            .key_service
            .create_key(
                None,
                EntityType::Client,
                client.id,
                true,
                "Evaluator key".into(),
                None,
            )
            .await
            .expect("Client key");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("isolated TLS listener");
        let address = listener.local_addr().expect("TLS address");
        let tls = api::LocalTls::create(&TlsConfig {
            root: fixture.root.join("evaluator-tls"),
            address,
        })
        .expect("isolated TLS material");
        let mode = Arc::new(AtomicUsize::new(0));
        let calls = Arc::new(AtomicUsize::new(0));
        let evaluator = Evaluator {
            idp: state.clone(),
            actor: fixture.owner,
            mode: mode.clone(),
            calls: calls.clone(),
        };
        let router = axum::Router::new()
            .route("/management/permissions/evaluate", post(evaluate))
            .with_state(evaluator);
        let (stop_evaluator, evaluator_stopped) = tokio::sync::oneshot::channel::<()>();
        let task = tokio::spawn(async move {
            axum::serve(tls.listener(listener), router)
                .with_graceful_shutdown(async move {
                    let _ = evaluator_stopped.await;
                })
                .await
                .expect("HTTPS evaluator listener");
        });
        let remote_config = config(&format!("https://{address}/management/"));
        let remote = ManagementPermissionClient::new(&remote_config).expect("HTTPS client");
        state = state.with_management_permissions(remote.clone());
        assert_eq!(
            applications(state.clone(), &user).await,
            StatusCode::SERVICE_UNAVAILABLE,
            "untrusted CA must not be accepted"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        let ca = std::fs::read(fixture.root.join("evaluator-tls/ca.pem")).expect("isolated CA");
        state = state.with_management_permissions(
            remote
                .with_test_root(&ca)
                .expect("test-only isolated trust"),
        );
        assert_eq!(applications(state.clone(), &user).await, StatusCode::OK);
        let mut local = state.clone();
        local.local_permissions = fixture.state.local_permissions.clone();
        let before = calls.load(Ordering::SeqCst);
        assert_eq!(applications(local, &user).await, StatusCode::FORBIDDEN);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            before,
            "local denial must not fall back to a remote allow"
        );
        for response_mode in 1..=13 {
            mode.store(response_mode, Ordering::SeqCst);
            let before = calls.load(Ordering::SeqCst);
            assert_eq!(
                applications(state.clone(), &user).await,
                if matches!(response_mode, 1 | 11 | 13) {
                    StatusCode::FORBIDDEN
                } else {
                    StatusCode::SERVICE_UNAVAILABLE
                },
                "response mode {response_mode}"
            );
            assert_eq!(
                calls.load(Ordering::SeqCst),
                before + 1,
                "redirects must not be followed"
            );
        }
        let before = calls.load(Ordering::SeqCst);
        let wrong_audience = user_token(&fixture, "wrong-audience").await;
        assert!(
            !applications(state.clone(), &wrong_audience)
                .await
                .is_success()
        );
        assert!(
            !applications(state.clone(), "forged-token")
                .await
                .is_success()
        );
        assert_eq!(calls.load(Ordering::SeqCst), before);
        let mut bad_credential = remote_config.clone();
        bad_credential.client_secret = "wrong-secret".into();
        let bad_state = state.clone().with_management_permissions(
            ManagementPermissionClient::new(&bad_credential)
                .expect("valid transport config")
                .with_test_root(&ca)
                .expect("isolated trust"),
        );
        assert_eq!(
            applications(bad_state, &user).await,
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            before,
            "failed Client authentication must not call Management"
        );
        stop_evaluator.send(()).expect("stop evaluator listener");
        task.await.expect("evaluator HTTPS shutdown");
        assert_eq!(
            applications(state.clone(), &user).await,
            StatusCode::SERVICE_UNAVAILABLE
        );
        fixture.server.endpoint().close().await;
        let root = fixture.root.clone();
        drop(state);
        drop(fixture);
        std::fs::remove_dir_all(root).expect("isolated fixture cleanup");
    })
    .await
    .expect("bounded independent permission HTTP scenario");
}

#[tokio::test]
async fn local_permission_http_remains_offline_and_does_not_fallback() {
    tokio::time::timeout(std::time::Duration::from_secs(120), async {
        use management_service::ManagementService;
        use management_service::replica::DbPermissionRepo;
        use management_service::replica::DbRoleRepo;

        let fixture = approval_fixture().await;
        let user = user_token(&fixture, "idp-services").await;
        let state = fixture.state.clone().with_management_permissions(
            ManagementPermissionClient::new(&config("https://127.0.0.1:1/management"))
                .expect("unreachable remote must be irrelevant to local policy"),
        );
        assert_eq!(
            applications(state.clone(), &user).await,
            StatusCode::FORBIDDEN
        );
        let policy = ManagementService::new(
            DbPermissionRepo::new(fixture.management.client()),
            DbRoleRepo::new(fixture.management.client()),
        );
        let role = policy
            .create_role(Id::nil(), "application reader", None)
            .await
            .expect("local role");
        let permission = policy
            .create_permission(
                Id::nil(),
                IdentityAction::ApplicationsRead.permission(),
                None,
            )
            .await
            .expect("exact local action");
        policy
            .add_permission_to_role(Id::nil(), role.id, permission.id)
            .await
            .expect("local permission grant");
        policy
            .add_role_to_user(Id::nil(), fixture.owner, role.id)
            .await
            .expect("local actor grant");
        assert_eq!(
            applications(state.clone(), &user).await,
            StatusCode::OK,
            "local policy requires no live remote service"
        );
        let mut remote_only = state.clone();
        remote_only.local_permissions = None;
        assert_eq!(
            applications(remote_only, &user).await,
            StatusCode::SERVICE_UNAVAILABLE,
            "remote failure must not read the fixture's Management database"
        );
        fixture.server.endpoint().close().await;
        let root = fixture.root.clone();
        drop(policy);
        drop(state);
        drop(fixture);
        std::fs::remove_dir_all(root).expect("local fixture cleanup");
    })
    .await
    .expect("bounded local permission HTTP scenario");
}

#[tokio::test]
async fn permission_runtime_rejects_remote_database_configuration_before_opening_it() {
    tokio::time::timeout(std::time::Duration::from_secs(120), async {
    let fixture = approval_fixture().await;
    let directory = fixture.root.join("must-not-open");
    let mut configuration = AppConfig {
        data_dir: fixture.root.to_string_lossy().into_owned(),
        management_data_dir: Some(directory.to_string_lossy().into_owned()),
        management_permissions: Some(config("https://management.test")),
        ..AppConfig::default()
    };
    let error = crate::build_runtime(
        &configuration,
        fixture.state.engine.clone(), None,
        fixture.state.transport_identity().expect("fixture transport").clone(),
        fixture.server.clone(),
        None,
    )
    .await
    .err()
    .expect("ambiguous policy sources must be rejected");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert!(
        !directory.exists(),
        "remote configuration must not open Management state"
    );
    configuration.management_data_dir = None;
    configuration.management_permissions = Some(config("http://management.test"));
    let error = crate::build_runtime(
        &configuration,
        fixture.state.engine.clone(), None,
        fixture.state.transport_identity().expect("fixture transport").clone(),
        fixture.server.clone(),
        None,
    )
    .await
    .err()
    .expect("HTTP evaluation must be rejected before runtime construction");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    configuration.management_data_dir = Some(directory.to_string_lossy().into_owned());
    configuration.oauth2.local_signer = Some(NodeSignerBinding {
        member_id: Id::now_v7(), key_id: Id::now_v7(), endpoint_id: "deliberately-mismatched-local-endpoint".into(),
    });
    let error = crate::build_runtime(&configuration, fixture.state.engine.clone(), None, fixture.state.transport_identity().expect("fixture transport").clone(), fixture.server.clone(), Some(management_service::LocalPermissions::new(fixture.management.client())))
        .await.err().expect("local runtime validates the actual signer without validating unused remote configuration");
    assert_eq!(error.to_string(), "invalid input: local node signer is not approved and active");
    assert!(!directory.exists());
    fixture.server.endpoint().close().await;
    let root = fixture.root.clone();
    drop(fixture);
    std::fs::remove_dir_all(root).expect("runtime fixture cleanup");
    }).await.expect("bounded runtime configuration scenario");
}
