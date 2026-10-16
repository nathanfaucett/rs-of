use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, header::AUTHORIZATION},
};
use idp_model::{
    contract::{
        ErrorCode, ErrorResponse, IDP_DEVICE_LIST_SCOPE, IDP_DEVICE_LOOKUP_SCOPE, IdentityAction,
        IdentityResource, PermissionTarget,
    },
    model::Id,
};
use iroh::EndpointId;
use management_service::{
    ApprovedDeviceEndpoints, DeviceEndpointIdentity, DeviceEnrollment, DeviceEnrollmentRequest,
    DeviceInfo, DeviceState, PairingAcceptance, TrustedDevice, UpdateDeviceRequest,
};
use management_service::{DeviceEnrollmentService, DeviceRepo, PairingAcceptanceController};
use model::contract::PrincipalType;

use crate::router::{
    RouterState,
    middleware::{StandardAuthorization, authorize_bearer_client, require_identity_permission},
};

#[cfg(test)]
#[path = "device_policy_tests.rs"]
mod policy_tests;

#[utoipa::path(put, path = "/devices/{device_id}/owner", params(("device_id" = String, Path)), request_body = management_service::DeviceTransferRequest,
    responses((status = 204, description = "Ownership updated; each Node must finish learned local cleanup before new-owner use")), security(("authorization" = [])))]
pub(crate) async fn transfer_device(
    State(state): State<RouterState>,
    Path(device_id): Path<Id>,
    actor: StandardAuthorization,
    Json(request): Json<management_service::DeviceTransferRequest>,
) -> Result<axum::http::StatusCode, ErrorResponse> {
    require_identity_permission(
        &state,
        &actor,
        IdentityAction::DevicesTransfer,
        PermissionTarget::Installation {
            resource: IdentityResource::Device {
                id: Some(device_id),
            },
        },
    )
    .await?;
    if request.owner_subject.is_nil() || request.owner_subject.to_string() == actor.claims.sub {
        return Err(ErrorResponse::new(ErrorCode::InvalidRequest));
    }
    let identity = trusted_device_identity(&state, device_id).await?;
    if identity.owner_subject != actor.claims.sub {
        return Err(ErrorResponse::new(ErrorCode::AccessDenied));
    }
    state
        .oauth2_service
        .find_user_info(request.owner_subject)
        .await?;
    if !state
        .devices
        .transfer_owned(
            device_id,
            &actor.claims.sub,
            &request.owner_subject.to_string(),
        )
        .await
        .map_err(ErrorResponse::from)?
    {
        return Err(ErrorResponse::new(ErrorCode::AccessDenied));
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}

const DEVICE_LOOKUP_SCOPE: &str = IDP_DEVICE_LOOKUP_SCOPE;
const DEVICE_LIST_SCOPE: &str = IDP_DEVICE_LIST_SCOPE;

#[utoipa::path(
    get,
    path = "/devices/endpoints/{endpoint_id}",
    params(("endpoint_id" = String, Path, description = "Iroh endpoint public key")),
    responses(
        (status = 200, description = "Approved endpoint identity", body = DeviceEndpointIdentity),
        (status = 401, description = "Invalid client access token"),
        (status = 403, description = "Client lacks device lookup permission"),
        (status = 404, description = "Endpoint is not an approved device")
    ),
    security(("authorization" = []))
)]
pub(crate) async fn lookup_device_endpoint(
    State(state): State<RouterState>,
    Path(endpoint_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<DeviceEndpointIdentity>, ErrorResponse> {
    let token = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ErrorResponse::new(ErrorCode::NotAuthorized))?;
    let caller = authorize_bearer_client(&state, token).await?;
    if !can_lookup_device_endpoint(
        caller.claims.principal_type,
        &caller.claims.aud,
        &caller.claims.scope,
        &state.service_audience,
    ) {
        return Err(ErrorResponse::new(ErrorCode::AccessDenied));
    }
    let parsed_endpoint = endpoint_id
        .parse::<EndpointId>()
        .map_err(|_| ErrorResponse::new(ErrorCode::InvalidRequest))?;
    if parsed_endpoint.to_string() != endpoint_id {
        return Err(ErrorResponse::new(ErrorCode::InvalidRequest));
    }
    let device = state
        .devices
        .find_approved_by_public_key(&endpoint_id)
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::ServerError))?
        .ok_or_else(|| ErrorResponse::new(ErrorCode::NotFound))?;
    trusted_device_identity(&state, device.id).await.map(Json)
}

#[utoipa::path(
    get,
    path = "/devices/identities/{device_id}",
    params(("device_id" = String, Path, description = "Device ID")),
    responses((status = 200, description = "Locally trusted Device owner", body = DeviceEndpointIdentity)),
    security(("authorization" = []))
)]
pub(crate) async fn lookup_device_identity(
    State(state): State<RouterState>,
    Path(device_id): Path<Id>,
    headers: HeaderMap,
) -> Result<Json<DeviceEndpointIdentity>, ErrorResponse> {
    let token = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ErrorResponse::new(ErrorCode::NotAuthorized))?;
    let caller = authorize_bearer_client(&state, token).await?;
    if !can_lookup_device_endpoint(
        caller.claims.principal_type,
        &caller.claims.aud,
        &caller.claims.scope,
        &state.service_audience,
    ) || device_id.is_nil()
    {
        return Err(ErrorResponse::new(ErrorCode::AccessDenied));
    }
    trusted_device_identity(&state, device_id).await.map(Json)
}

async fn trusted_device_identity(
    state: &RouterState,
    device_id: Id,
) -> Result<DeviceEndpointIdentity, ErrorResponse> {
    let candidate = state
        .devices
        .list()
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::ServerError))?
        .into_iter()
        .find(|device| device.id == device_id)
        .ok_or_else(|| ErrorResponse::new(ErrorCode::NotFound))?;
    let device = state
        .devices
        .find_approved_by_public_key(&candidate.public_key)
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::AccessDenied))?
        .filter(|device| device.id == device_id)
        .ok_or_else(|| ErrorResponse::new(ErrorCode::NotFound))?;
    let endpoint = device
        .public_key
        .parse::<EndpointId>()
        .map_err(|_| ErrorResponse::new(ErrorCode::AccessDenied))?;
    if endpoint.to_string() != device.public_key {
        return Err(ErrorResponse::new(ErrorCode::AccessDenied));
    }
    let owner = device
        .owner_subject
        .parse::<Id>()
        .map_err(|_| ErrorResponse::new(ErrorCode::AccessDenied))?;
    if owner.is_nil() || owner.to_string() != device.owner_subject {
        return Err(ErrorResponse::new(ErrorCode::AccessDenied));
    }
    state.oauth2_service.find_user_info(owner).await?;
    Ok(DeviceEndpointIdentity {
        device_id,
        owner_subject: device.owner_subject,
        endpoint_id: device.public_key,
    })
}

#[utoipa::path(
    get,
    path = "/devices/endpoints",
    responses(
        (status = 200, description = "Approved endpoint IDs", body = ApprovedDeviceEndpoints),
        (status = 401, description = "Invalid client access token"),
        (status = 403, description = "Client lacks device-list permission")
    ),
    security(("authorization" = []))
)]
pub(crate) async fn list_approved_device_endpoints(
    State(state): State<RouterState>,
    headers: HeaderMap,
) -> Result<Json<ApprovedDeviceEndpoints>, ErrorResponse> {
    let token = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ErrorResponse::new(ErrorCode::NotAuthorized))?;
    let caller = authorize_bearer_client(&state, token).await?;
    if !can_list_device_endpoints(
        caller.claims.principal_type,
        &caller.claims.aud,
        &caller.claims.scope,
        &state.service_audience,
    ) {
        return Err(ErrorResponse::new(ErrorCode::AccessDenied));
    }
    let mut endpoint_ids = Vec::new();
    for device in state
        .devices
        .list()
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::ServerError))?
        .into_iter()
        .filter(|device| device.state == DeviceState::Approved && device.revoked_at.is_none())
    {
        endpoint_ids.push(
            trusted_device_identity(&state, device.id)
                .await?
                .endpoint_id,
        );
    }
    Ok(Json(ApprovedDeviceEndpoints { endpoint_ids }))
}

fn can_list_device_endpoints(
    principal_type: PrincipalType,
    audience: &str,
    scopes: &[String],
    service_audience: &str,
) -> bool {
    principal_type == PrincipalType::Client
        && audience == service_audience
        && scopes.iter().any(|scope| scope == DEVICE_LIST_SCOPE)
}

fn can_lookup_device_endpoint(
    principal_type: PrincipalType,
    audience: &str,
    scopes: &[String],
    service_audience: &str,
) -> bool {
    principal_type == PrincipalType::Client
        && audience == service_audience
        && scopes.iter().any(|scope| scope == DEVICE_LOOKUP_SCOPE)
}

#[cfg(test)]
mod tests {
    use model::contract::PrincipalType;

    use super::{DEVICE_LIST_SCOPE, can_list_device_endpoints, can_lookup_device_endpoint};

    #[test]
    fn device_lookup_requires_client_principal_audience_and_scope() {
        let scopes = vec!["idp.device.lookup".to_owned()];

        assert!(can_lookup_device_endpoint(
            PrincipalType::Client,
            "idp-service",
            &scopes,
            "idp-service"
        ));
        assert!(!can_lookup_device_endpoint(
            PrincipalType::User,
            "idp-service",
            &scopes,
            "idp-service"
        ));
        assert!(!can_lookup_device_endpoint(
            PrincipalType::Client,
            "other-service",
            &scopes,
            "idp-service"
        ));
        assert!(!can_lookup_device_endpoint(
            PrincipalType::Client,
            "idp-service",
            &[],
            "idp-service"
        ));
    }

    #[test]
    fn device_list_requires_its_own_client_scope() {
        let scopes = vec!["idp.device.lookup".to_owned()];
        assert!(!can_list_device_endpoints(
            PrincipalType::Client,
            "idp-service",
            &scopes,
            "idp-service"
        ));
        let scopes = vec![DEVICE_LIST_SCOPE.to_owned()];
        assert!(can_list_device_endpoints(
            PrincipalType::Client,
            "idp-service",
            &scopes,
            "idp-service"
        ));
        assert!(!can_list_device_endpoints(
            PrincipalType::User,
            "idp-service",
            &scopes,
            "idp-service"
        ));
    }
}

#[utoipa::path(
    get,
    path = "/devices/trusted",
    responses((status = 200, description = "Approved devices", body = [TrustedDevice])),
    security(("authorization" = []))
)]
pub(crate) async fn trusted_devices(
    State(state): State<RouterState>,
    StandardAuthorization { claims, .. }: StandardAuthorization,
) -> Result<Json<Vec<TrustedDevice>>, ErrorResponse> {
    if !claims.scope.iter().any(|scope| scope == "storage") {
        return Err(ErrorResponse::new(ErrorCode::AccessDenied));
    }
    let devices = state
        .devices
        .list_approved(&claims.sub)
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::ServerError))?;
    Ok(Json(devices))
}

#[utoipa::path(
    post,
    path = "/devices/enrollments",
    request_body = DeviceEnrollmentRequest,
    responses((status = 200, description = "Device enrollment", body = DeviceEnrollment)),
    security(("authorization" = []))
)]
pub(crate) async fn enroll_device(
    State(state): State<RouterState>,
    actor: StandardAuthorization,
    Json(request): Json<DeviceEnrollmentRequest>,
) -> Result<Json<DeviceEnrollment>, ErrorResponse> {
    require_identity_permission(
        &state,
        &actor,
        IdentityAction::DevicesEnroll,
        PermissionTarget::Installation {
            resource: IdentityResource::Device { id: None },
        },
    )
    .await?;
    DeviceEnrollmentService::new(state.devices)
        .enroll(actor.claims.sub, request)
        .await
        .map(Json)
}

#[utoipa::path(
    get,
    path = "/devices/pairing-accepting",
    responses((status = 200, description = "Pairing acceptance state", body = PairingAcceptance)),
    security(("authorization" = []))
)]
pub(crate) async fn pairing_acceptance(
    State(state): State<RouterState>,
    actor: StandardAuthorization,
) -> Result<Json<PairingAcceptance>, ErrorResponse> {
    require_identity_permission(
        &state,
        &actor,
        IdentityAction::DevicePairingRead,
        PermissionTarget::Installation {
            resource: IdentityResource::DevicePairing,
        },
    )
    .await?;
    state
        .pairing_acceptance
        .pairing_accepting()
        .map(|accepting| Json(PairingAcceptance { accepting }))
        .map_err(|_| ErrorResponse::new(ErrorCode::ServerError))
}

#[utoipa::path(
    put,
    path = "/devices/pairing-accepting",
    request_body = PairingAcceptance,
    responses((status = 200, description = "Pairing acceptance state", body = PairingAcceptance)),
    security(("authorization" = []))
)]
pub(crate) async fn set_pairing_acceptance(
    State(state): State<RouterState>,
    actor: StandardAuthorization,
    Json(PairingAcceptance { accepting }): Json<PairingAcceptance>,
) -> Result<Json<PairingAcceptance>, ErrorResponse> {
    require_identity_permission(
        &state,
        &actor,
        IdentityAction::DevicePairingUpdate,
        PermissionTarget::Installation {
            resource: IdentityResource::DevicePairing,
        },
    )
    .await?;
    state
        .pairing_acceptance
        .set_pairing_accepting(accepting)
        .map(|()| Json(PairingAcceptance { accepting }))
        .map_err(|_| ErrorResponse::new(ErrorCode::ServerError))
}

#[utoipa::path(
    get,
    path = "/devices",
    responses((status = 200, description = "Devices", body = [DeviceInfo])),
    security(("authorization" = []))
)]
pub(crate) async fn list_devices(
    State(state): State<RouterState>,
    StandardAuthorization { claims, .. }: StandardAuthorization,
) -> Result<Json<Vec<DeviceInfo>>, ErrorResponse> {
    DeviceEnrollmentService::new(state.devices)
        .list(&claims.sub)
        .await
        .map(Json)
}

#[utoipa::path(
    patch,
    path = "/devices/{id}",
    params(("id" = String, Path, description = "Device ID")),
    request_body = UpdateDeviceRequest,
    responses((status = 200, description = "Updated device", body = DeviceInfo)),
    security(("authorization" = []))
)]
pub(crate) async fn update_device(
    State(state): State<RouterState>,
    Path(device_id): Path<Id>,
    actor: StandardAuthorization,
    Json(request): Json<UpdateDeviceRequest>,
) -> Result<Json<DeviceInfo>, ErrorResponse> {
    require_identity_permission(
        &state,
        &actor,
        IdentityAction::DevicesRename,
        PermissionTarget::Installation {
            resource: IdentityResource::Device {
                id: Some(device_id),
            },
        },
    )
    .await?;
    DeviceEnrollmentService::new(state.devices)
        .rename(&actor.claims.sub, device_id, request)
        .await
        .map(Json)
}

#[utoipa::path(
    delete,
    path = "/devices/{id}",
    params(("id" = String, Path, description = "Device ID")),
    responses((status = 204, description = "Revoked device")),
    security(("authorization" = []))
)]
pub(crate) async fn revoke_device(
    State(state): State<RouterState>,
    Path(device_id): Path<Id>,
    actor: StandardAuthorization,
) -> Result<(), ErrorResponse> {
    require_identity_permission(
        &state,
        &actor,
        IdentityAction::DevicesRevoke,
        PermissionTarget::Installation {
            resource: IdentityResource::Device {
                id: Some(device_id),
            },
        },
    )
    .await?;
    DeviceEnrollmentService::new(state.devices)
        .revoke(
            &actor.claims.sub,
            device_id,
            &state.device_identity.endpoint_id().to_string(),
        )
        .await
}

#[cfg(test)]
mod admission_tests {
    use idp_model::model::Id;
    use management_service::DeviceRepo;

    #[tokio::test]
    async fn device_identity_http_admits_approved_endpoint_without_signer_and_rechecks_revocation()
    {
        use idp_model::contract::{
            ApplicationRegistration, ClientProfile, ClientRegistration, ClientType, GrantType,
            TokenEndpointAuthMethod,
        };
        use idp_service::repo::ClientRepo;
        let fixture = crate::node_join_tests::approval_fixture().await;
        fixture
            .state
            .oauth2_service
            .client_repo
            .create_client(ClientRegistration {
                application: ApplicationRegistration {
                    name: Some("Management lookup".into()),
                    uri: "https://management-lookup.test".into(),
                    description: None,
                },
                client_id: Some("management-lookup".into()),
                client_secret: Some("lookup-secret".into()),
                client_id_issued_at: None,
                client_secret_expires_at: None,
                client_name: "Management lookup".into(),
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
                allowed_scopes: vec!["idp.token.validate".into(), "idp.device.lookup".into()],
                allowed_audiences: vec!["idp-services".into()],
                token_endpoint_auth_method: TokenEndpointAuthMethod::ClientSecretPost,
                software_statement: None,
                software_id: None,
                software_version: None,
            })
            .await
            .expect("least-privilege local service client");

        let endpoint = iroh::SecretKey::generate().public();
        let device = fixture
            .state
            .devices
            .create(
                fixture.owner.to_string(),
                "Device only".into(),
                endpoint.to_string(),
                String::new(),
                b"http-enrollment".to_vec(),
                i64::MAX,
            )
            .await
            .expect("pending Device");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("local owner listener");
        let address = listener.local_addr().expect("listener address");
        let router: axum::Router = crate::openapi_router(fixture.state.clone(), "").into();
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.expect("local IdP HTTP");
        });
        let local = format!("http://{address}");
        let control = management_service::HostedControlPlane::new_with_services(
            &local,
            &local,
            &fixture.issuer,
        )
        .expect("local control plane")
        .with_idp_service_client("management-lookup", "lookup-secret", "idp-services")
        .expect("scoped service");

        assert!(
            control
                .get_storage_device_identity(device.id)
                .await
                .is_err(),
            "pending Device denies"
        );
        fixture
            .state
            .devices
            .approve(device.id, b"http-enrollment")
            .await
            .expect("approve endpoint")
            .expect("approved Device");
        let identity = control
            .get_storage_device_identity(device.id)
            .await
            .expect("Device needs no signer");
        assert_eq!(identity.device_id, device.id);
        assert_eq!(identity.owner_subject, fixture.owner.to_string());
        assert_eq!(identity.endpoint_id, endpoint.to_string());
        assert!(
            control
                .get_storage_device_identity(Id::now_v7())
                .await
                .is_err()
        );
        assert!(
            fixture
                .state
                .devices
                .revoke_self(&endpoint.to_string())
                .await
                .expect("learn Device revocation")
        );
        assert!(
            control
                .get_storage_device_identity(device.id)
                .await
                .is_err(),
            "existing service token cannot bypass revocation"
        );
        fixture
            .state
            .engine
            .client()
            .translate_and_execute("DROP INDEX devices_public_key", &db::SqlTranslator)
            .await
            .expect("allow ambiguous endpoint fixture");
        let duplicate = fixture
            .state
            .devices
            .create(
                fixture.owner.to_string(),
                "Reused endpoint".into(),
                endpoint.to_string(),
                String::new(),
                b"duplicate-enrollment".to_vec(),
                i64::MAX,
            )
            .await
            .expect("duplicate endpoint Device");
        fixture
            .state
            .devices
            .approve(duplicate.id, b"duplicate-enrollment")
            .await
            .expect("approve duplicate Device")
            .expect("approved duplicate");
        assert!(
            control
                .get_storage_device_identity(duplicate.id)
                .await
                .is_err(),
            "an approved Device cannot bypass a duplicate revoked endpoint binding"
        );
        server.abort();
        let _ = server.await;
        fixture.server.endpoint().close().await;
        let root = fixture.root.clone();
        drop(fixture);
        std::fs::remove_dir_all(root).expect("remove isolated HTTP fixture");
    }
}
