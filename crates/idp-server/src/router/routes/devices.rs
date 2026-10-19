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
    ApprovedDeviceEndpoints, DeviceEndpointIdentity, DeviceRepo, DeviceState,
    PairingAcceptanceController,
};
use model::contract::PrincipalType;

use crate::router::{
    RouterState,
    middleware::{StandardAuthorization, authorize_bearer_client, require_identity_permission},
};

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
    Ok(Json(DeviceEndpointIdentity {
        device_id: device.id,
        owner_subject: device.owner_subject,
        endpoint_id: device.public_key,
    }))
}

const DEVICE_LOOKUP_SCOPE: &str = IDP_DEVICE_LOOKUP_SCOPE;
const DEVICE_LIST_SCOPE: &str = IDP_DEVICE_LIST_SCOPE;

#[utoipa::path(
    get,
    path = "/devices/endpoints",
    params(("endpoint_id" = String, Path, description = "Iroh endpoint public key")),
    responses(
        (status = 200, description = "Approved endpoint identity list", body = ApprovedDeviceEndpoints),
        (status = 401, description = "Invalid client access token"),
        (status = 403, description = "Client lacks device list permission"),
        (status = 404, description = "No approved devices found")
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
    let endpoint_ids: Vec<String> = state
        .devices
        .list()
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::ServerError))?
        .into_iter()
        .filter(|device| device.state == management_service::DeviceState::Approved)
        .map(|device| device.public_key)
        .collect();
    Ok(Json(ApprovedDeviceEndpoints { endpoint_ids }))
}

fn can_list_device_endpoints(
    principal_type: PrincipalType,
    aud: &str,
    scopes: &[String],
    service_audience: &str,
) -> bool {
    scopes.iter().any(|scope| {
        scope == IDP_DEVICE_LIST_SCOPE
            || (aud == service_audience && principal_type == PrincipalType::Client)
    })
}

fn can_lookup_device_endpoint(
    principal_type: PrincipalType,
    aud: &str,
    scopes: &[String],
    service_audience: &str,
) -> bool {
    scopes.iter().any(|scope| {
        scope == IDP_DEVICE_LOOKUP_SCOPE
            || (aud == service_audience && principal_type == PrincipalType::Client)
    })
}
