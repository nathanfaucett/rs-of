use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, header::AUTHORIZATION},
};
use idp_model::{
    contract::{
        ErrorCode, ErrorResponse, IdentityAction, IdentityResource, MANAGEMENT_DEVICE_LIST_SCOPE,
        MANAGEMENT_DEVICE_LOOKUP_SCOPE, PermissionTarget,
    },
    model::Id,
};
use iroh::EndpointId;
use management_service::{
    ApprovedDeviceEndpoints, DeviceEndpointIdentity, DeviceEnrollment, DeviceEnrollmentRequest,
    DeviceInfo, PairingAcceptance, TrustedDevice, UpdateDeviceRequest,
};
use management_service::{DeviceEnrollmentService, DeviceRepo};
use model::contract::PrincipalType;

use crate::router::{
    RouterState,
    middleware::{ManagementAuthorization, require_identity_permission},
};

#[utoipa::path(put, path = "/devices/{device_id}/owner", params(("device_id" = String, Path)), request_body = management_service::DeviceTransferRequest,
    responses((status = 204, description = "Ownership updated; each Node must finish learned local cleanup before new-owner use")), security(("authorization" = [])))]
pub(crate) async fn transfer_device(
    State(state): State<RouterState>,
    Path(device_id): Path<Id>,
    actor: ManagementAuthorization,
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
    if request.owner_subject.is_nil()
        || request.owner_subject.to_string() == actor.subject.to_string()
    {
        return Err(ErrorResponse::new(ErrorCode::InvalidRequest));
    }
    let identity = trusted_device_identity(&state, device_id).await?;
    if identity.owner_subject != actor.subject.to_string() {
        return Err(ErrorResponse::new(ErrorCode::AccessDenied));
    }
    state
        .control_plane
        .active_user(request.owner_subject)
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::AccessDenied))?;
    if !state
        .control_plane
        .device_repo()
        .map_err(unavailable)?
        .transfer_owned(
            device_id,
            &actor.subject.to_string(),
            &request.owner_subject.to_string(),
        )
        .await
        .map_err(ErrorResponse::from)?
    {
        return Err(ErrorResponse::new(ErrorCode::AccessDenied));
    }
    state
        .selection_policies
        .reconcile_owner_prevalidated(device_id, &request.owner_subject.to_string())
        .await
        .map_err(ErrorResponse::from)?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

const DEVICE_LOOKUP_SCOPE: &str = MANAGEMENT_DEVICE_LOOKUP_SCOPE;
const DEVICE_LIST_SCOPE: &str = MANAGEMENT_DEVICE_LIST_SCOPE;

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
    let (claims, _) = state
        .control_plane
        .validate_actor_token(token)
        .await
        .map_err(service_auth_error)?;
    if !can_lookup_device_endpoint(
        claims.principal_type,
        &claims.aud,
        &claims.scope,
        &state.storage_audience,
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
        .control_plane
        .device_repo()
        .map_err(unavailable)?
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
    let (claims, _) = state
        .control_plane
        .validate_actor_token(token)
        .await
        .map_err(service_auth_error)?;
    if !can_lookup_device_endpoint(
        claims.principal_type,
        &claims.aud,
        &claims.scope,
        &state.storage_audience,
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
    state
        .control_plane
        .get_storage_device_identity(device_id)
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::AccessDenied))
}

fn devices(
    state: &RouterState,
) -> Result<std::sync::Arc<management_service::replica::DbDeviceRepo>, ErrorResponse> {
    state
        .control_plane
        .device_repo()
        .cloned()
        .map_err(unavailable)
}

fn service_auth_error(error: String) -> ErrorResponse {
    if error == "actor token was rejected by IdP" {
        ErrorResponse::new(ErrorCode::NotAuthorized)
    } else {
        unavailable(error)
    }
}

fn unavailable(_: String) -> ErrorResponse {
    ErrorResponse::new(ErrorCode::TemporarilyUnavailable)
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
    let (claims, _) = state
        .control_plane
        .validate_actor_token(token)
        .await
        .map_err(service_auth_error)?;
    if !can_list_device_endpoints(
        claims.principal_type,
        &claims.aud,
        &claims.scope,
        &state.storage_audience,
    ) {
        return Err(ErrorResponse::new(ErrorCode::AccessDenied));
    }
    let endpoint_ids = state
        .control_plane
        .approved_device_endpoints()
        .await
        .map_err(unavailable)?;
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

#[utoipa::path(
    get,
    path = "/devices/trusted",
    responses((status = 200, description = "Approved devices", body = [TrustedDevice])),
    security(("authorization" = []))
)]
pub(crate) async fn trusted_devices(
    State(state): State<RouterState>,
    actor: ManagementAuthorization,
) -> Result<Json<Vec<TrustedDevice>>, ErrorResponse> {
    require_identity_permission(
        &state,
        &actor,
        IdentityAction::DevicesRead,
        PermissionTarget::Installation {
            resource: IdentityResource::Device { id: None },
        },
    )
    .await?;
    let mut devices = Vec::new();
    for device in state
        .control_plane
        .device_repo()
        .map_err(unavailable)?
        .list()
        .await
        .map_err(ErrorResponse::from)?
    {
        if device.owner_subject == actor.subject.to_string()
            && device.state == management_service::DeviceState::Approved
            && device.revoked_at.is_none()
        {
            trusted_device_identity(&state, device.id).await?;
            devices.push(TrustedDevice {
                public_key: device.public_key,
                address: device.address,
            });
        }
    }
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
    actor: ManagementAuthorization,
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
    DeviceEnrollmentService::new(devices(&state)?)
        .enroll(actor.subject.to_string(), request)
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
    actor: ManagementAuthorization,
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
        .control_plane
        .pairing_acceptance()
        .map_err(unavailable)?
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
    actor: ManagementAuthorization,
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
        .control_plane
        .pairing_acceptance()
        .map_err(unavailable)?
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
    actor: ManagementAuthorization,
) -> Result<Json<Vec<DeviceInfo>>, ErrorResponse> {
    require_identity_permission(
        &state,
        &actor,
        IdentityAction::DevicesRead,
        PermissionTarget::Installation {
            resource: IdentityResource::Device { id: None },
        },
    )
    .await?;
    DeviceEnrollmentService::new(devices(&state)?)
        .list(&actor.subject.to_string())
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
    actor: ManagementAuthorization,
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
    DeviceEnrollmentService::new(devices(&state)?)
        .rename(&actor.subject.to_string(), device_id, request)
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
    actor: ManagementAuthorization,
) -> Result<axum::http::StatusCode, ErrorResponse> {
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
    DeviceEnrollmentService::new(devices(&state)?)
        .revoke(
            &actor.subject.to_string(),
            device_id,
            &state
                .control_plane
                .device_identity()
                .map(|identity| identity.endpoint_id().to_string())
                .unwrap_or_default(),
        )
        .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}
