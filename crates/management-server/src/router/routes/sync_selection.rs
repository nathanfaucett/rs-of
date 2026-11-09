//! WebSocket sync selection routes.

use axum::{
    Json,
    extract::{Path, State},
};
use idp_model::model::Id;
use management_service::{
    ManagementService, SyncSelection, SyncSelectionRequest, SyncSelectionResponse,
};
use crate::router::{RouterState, middleware::ManagementAuthorization, ErrorResponse};
use crate::router::routes::roles::require_application_permission;

const SYNC_SELECTION_READ_PERMISSION: &str = "sync.selection.read";
const SYNC_SELECTION_WRITE_PERMISSION: &str = "sync.selection.write";

#[utoipa::path(
    post,
    path = "/management/devices/{device_id}/sync-selections",
    params(("device_id" = String, Path, description = "Device ID")),
    request_body = SyncSelectionRequest,
    responses(
        (status = 200, description = "Sync selection updated", body = SyncSelectionResponse),
        (status = 401, description = "Invalid bearer token"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Device not found"),
        (status = 503, description = "Service unavailable")
    ),
    security(("authorization" = []))
)]
pub(crate) async fn set_sync_selection(
    State(state): State<RouterState>,
    Path(device_id_str): Path<String>,
    authorization: ManagementAuthorization,
    Json(request): Json<SyncSelectionRequest>,
) -> Result<Json<SyncSelectionResponse>, ErrorResponse> {
    let device_id = Id::parse(&device_id_str).map_err(|_| ErrorResponse::new(idp_model::contract::ErrorCode::InvalidRequest))?;

    require_application_permission(
        state.management_service.as_ref(),
        &authorization,
        authorization.application_id,
        SYNC_SELECTION_WRITE_PERMISSION,
    )
    .await?;

    let service = ManagementService::new(
        state.permission_repo.clone(),
        state.role_repo.clone(),
    );

    if request.select {
        service
            .select_resource_for_sync(
                device_id,
                request.resource_id,
                authorization.application_id,
                &authorization.subject.to_string(),
            )
            .await
            .map_err(|_| ErrorResponse::new(idp_model::contract::ErrorCode::ServiceUnavailable))?;
    } else {
        service
            .deselect_resource_for_sync(device_id, request.resource_id)
            .await
            .map_err(|_| ErrorResponse::new(idp_model::contract::ErrorCode::ServiceUnavailable))?;
    }

    let selections = service
        .list_sync_selections_for_device(device_id)
        .await
        .map_err(|_| ErrorResponse::new(idp_model::contract::ErrorCode::ServiceUnavailable))?;

    Ok(Json(SyncSelectionResponse { selections }))
}

#[utoipa::path(
    get,
    path = "/management/devices/{device_id}/sync-selections",
    params(("device_id" = String, Path, description = "Device ID")),
    responses(
        (status = 200, description = "Sync selections for device", body = SyncSelectionResponse),
        (status = 401, description = "Invalid bearer token"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Device not found"),
        (status = 503, description = "Service unavailable")
    ),
    security(("authorization" = []))
)]
pub(crate) async fn list_sync_selections(
    State(state): State<RouterState>,
    Path(device_id_str): Path<String>,
    authorization: ManagementAuthorization,
) -> Result<Json<SyncSelectionResponse>, ErrorResponse> {
    let device_id = Id::parse(&device_id_str).map_err(|_| ErrorResponse::new(idp_model::contract::ErrorCode::InvalidRequest))?;

    require_application_permission(
        state.management_service.as_ref(),
        &authorization,
        authorization.application_id,
        SYNC_SELECTION_READ_PERMISSION,
    )
    .await?;

    let service = ManagementService::new(
        state.permission_repo.clone(),
        state.role_repo.clone(),
    );

    let selections = service
        .list_sync_selections_for_device(device_id)
        .await
        .map_err(|_| ErrorResponse::new(idp_model::contract::ErrorCode::ServiceUnavailable))?;

    Ok(Json(SyncSelectionResponse { selections }))
}

#[utoipa::path(
    get,
    path = "/management/resources/{resource_id}/sync-targets",
    params(("resource_id" = String, Path, description = "Resource ID")),
    responses(
        (status = 200, description = "Devices with resource selected for sync", body = SyncSelectionResponse),
        (status = 401, description = "Invalid bearer token"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Resource not found"),
        (status = 503, description = "Service unavailable")
    ),
    security(("authorization" = []))
)]
pub(crate) async fn list_sync_targets(
    State(state): State<RouterState>,
    Path(resource_id_str): Path<String>,
    authorization: ManagementAuthorization,
) -> Result<Json<SyncSelectionResponse>, ErrorResponse> {
    let resource_id = Id::parse(&resource_id_str).map_err(|_| ErrorResponse::new(idp_model::contract::ErrorCode::InvalidRequest))?;

    require_application_permission(
        state.management_service.as_ref(),
        &authorization,
        authorization.application_id,
        SYNC_SELECTION_READ_PERMISSION,
    )
    .await?;

    let service = ManagementService::new(
        state.permission_repo.clone(),
        state.role_repo.clone(),
    );

    let selections = service
        .list_sync_targets_for_resource(resource_id)
        .await
        .map_err(|_| ErrorResponse::new(idp_model::contract::ErrorCode::ServiceUnavailable))?;

    Ok(Json(SyncSelectionResponse { selections }))
}
