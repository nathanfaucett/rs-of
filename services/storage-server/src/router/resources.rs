use of_model::server::storage::resources::CreateResourceRequest;
use of_model::server::storage::resources::DatabaseDetailResponse;
use of_model::server::storage::resources::EnsureDatabaseRequest;
use of_model::server::storage::resources::FileSystemDetailResponse;
use of_model::server::storage::resources::FileSystemResourceResponse;
use of_model::server::storage::resources::SyncConnectRequest;
use of_model::server::storage::resources::SyncSessionResponse;
use of_model::storage::sync::SyncStatus;
use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
    routing::{get, post},
};
use file_system::resource::FileSystemId;
use file_system::resource::FileSystemResource;
use iroh::EndpointId;
use of_model::id::Id;
use of_model::storage::database_resource::DatabaseId;
use of_model::storage::database_resource::DatabaseResource;
use storage_service::scoped_file_system::ScopedFileSystemRuntime;
use uuid::Uuid;

use crate::router::{
    state::RouterState,
    storage_authorization::{
        StorageAuthorization, authorize_storage_token, scoped_file_system_socket_router,
    },
};

#[path = "sql.rs"]
pub mod sql;

#[derive(Clone)]
pub(super) struct ResourceState {
    pub router: RouterState,
    pub databases: Option<Arc<storage_service::database_runtime::DatabaseRuntime>>,
    pub file_systems: Option<Arc<ScopedFileSystemRuntime<EndpointId>>>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct StorageNamespace {
    user_sub: String,
    application_id: Id,
}

impl of_model::storage::namespace::StorageNamespace for StorageNamespace {
    fn user_sub(&self) -> &str {
        &self.user_sub
    }

    fn application_id(&self) -> Id {
        self.application_id
    }
}

#[utoipa::path(post, path = "/storage/databases/ensure", request_body = EnsureDatabaseRequest,
    responses((status = 200, description = "Created or reopened explicit logical database", body = DatabaseResource),
    (status = 400, description = "Invalid creation key", body = sql::DatabaseFailure),
    (status = 401, description = "Invalid authorization", body = sql::DatabaseFailure),
    (status = 403, description = "Write denied", body = sql::DatabaseFailure),
    (status = 409, description = "Local copy fenced", body = sql::DatabaseFailure),
    (status = 410, description = "Identity tombstoned", body = sql::DatabaseFailure),
    (status = 413, description = "Request exceeds 256 KiB", body = sql::DatabaseFailure),
    (status = 422, description = "Malformed request", body = sql::DatabaseFailure),
    (status = 503, description = "Content unavailable", body = sql::DatabaseFailure)),
    security(("authorization" = [])))]
pub async fn ensure_database(
    State(state): State<ResourceState>,
    headers: HeaderMap,
    request: Result<Json<EnsureDatabaseRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<DatabaseResource>, sql::SqlError> {
    let (scope, _, can_write) = namespace(
        authorization(&state, &headers).await.map_err(sql::status)?,
        "storage:databases",
    )
    .map_err(sql::status)?;
    if !can_write {
        return Err(sql::status(StatusCode::FORBIDDEN));
    }
    let Json(request) = request.map_err(|error| sql::status(error.status()))?;
    let resource = state
        .databases
        .as_ref()
        .ok_or_else(|| sql::status(StatusCode::SERVICE_UNAVAILABLE))?
        .ensure(&scope, &request.creation_key, request.name)
        .map_err(|error| {
            sql::status(match error.kind() {
                std::io::ErrorKind::InvalidInput => StatusCode::BAD_REQUEST,
                std::io::ErrorKind::AlreadyExists => StatusCode::GONE,
                std::io::ErrorKind::PermissionDenied => StatusCode::CONFLICT,
                _ => StatusCode::SERVICE_UNAVAILABLE,
            })
        })?;
    authorize_response(&state, &headers, &scope, "storage:databases", true)
        .await
        .map_err(sql::status)?;
    Ok(Json(resource))
}

pub fn resource_router(
    router: RouterState,
    databases: Option<Arc<storage_service::database_runtime::DatabaseRuntime>>,
    file_systems: Option<Arc<ScopedFileSystemRuntime<EndpointId>>>,
) -> Router {
    let state = ResourceState {
        router,
        databases,
        file_systems,
    };
    let mut router = Router::new()
        .route("/storage/databases/ensure", post(ensure_database))
        .route(
            "/storage/databases/{database_id}/sql",
            post(sql::execute_sql),
        )
        .layer(axum::extract::DefaultBodyLimit::max(256 * 1024))
        .route(
            "/storage/databases",
            post(create_database).get(list_databases),
        )
        .route(
            "/storage/databases/{database_id}",
            get(get_database).delete(delete_database),
        )
        .route(
            "/storage/filesystems",
            post(create_file_system).get(list_file_systems),
        )
        .route(
            "/storage/filesystems/{filesystem_id}",
            get(get_file_system).delete(delete_file_system),
        )
        .route(
            "/storage/filesystems/{filesystem_id}/residency",
            get(super::residency::get_file_system_residency)
                .put(super::residency::put_file_system_residency),
        )
        .route("/sync/connect", post(sync_connect))
        .with_state(state.clone());
    if let (Some(idp_client), Some(file_systems)) = (state.router.idp_client, state.file_systems) {
        router = router.merge(scoped_file_system_socket_router(idp_client, file_systems));
    }
    router
}

pub async fn authorization(
    state: &ResourceState,
    headers: &HeaderMap,
) -> Result<StorageAuthorization, StatusCode> {
    let token = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|token| !token.is_empty())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let idp_client = state
        .router
        .idp_client
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    authorize_storage_token(idp_client, token)
        .await
        .map_err(|error| match error {
            crate::router::storage_authorization::StorageAuthorizationError::InvalidToken => {
                StatusCode::UNAUTHORIZED
            }
            crate::router::storage_authorization::StorageAuthorizationError::ServiceUnavailable => {
                StatusCode::SERVICE_UNAVAILABLE
            }
        })
}

pub fn namespace(
    authorization: StorageAuthorization,
    resource: &str,
) -> Result<(StorageNamespace, bool, bool), StatusCode> {
    if authorization.resource != resource {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let application_id = authorization
        .application_id
        .parse::<Id>()
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    Ok((
        StorageNamespace {
            user_sub: authorization.subject,
            application_id,
        },
        authorization.can_read,
        authorization.can_write,
    ))
}

pub async fn authorize_response(
    state: &ResourceState,
    headers: &HeaderMap,
    scope: &StorageNamespace,
    resource: &str,
    write: bool,
) -> Result<(), StatusCode> {
    let (current_scope, can_read, can_write) =
        namespace(authorization(state, headers).await?, resource)?;
    if &current_scope != scope {
        return Err(StatusCode::UNAUTHORIZED);
    }
    if !(if write { can_write } else { can_read }) {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(())
}

#[utoipa::path(
    post,
    path = "/storage/databases",
    request_body = CreateResourceRequest,
    responses(
        (status = 201, description = "Database resource created", body = DatabaseResource),
        (status = 401, description = "Invalid storage authorization"),
        (status = 403, description = "Token does not permit writes"),
        (status = 503, description = "Storage runtime or IdP unavailable")
    ),
    security(("authorization" = []))
)]
pub async fn create_database(
    State(state): State<ResourceState>,
    headers: HeaderMap,
    Json(request): Json<CreateResourceRequest>,
) -> Result<(StatusCode, Json<DatabaseResource>), StatusCode> {
    let (scope, _, can_write) =
        namespace(authorization(&state, &headers).await?, "storage:databases")?;
    if !can_write {
        return Err(StatusCode::FORBIDDEN);
    }
    let (resource, _) = state
        .databases
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?
        .create(&scope, request.name)
        .map_err(|error| {
            log::error!("Storage database creation failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    authorize_response(&state, &headers, &scope, "storage:databases", true).await?;
    Ok((StatusCode::CREATED, Json(resource)))
}

#[utoipa::path(
    get,
    path = "/storage/databases",
    responses(
        (status = 200, description = "List of database resources", body = [DatabaseResource]),
        (status = 401, description = "Invalid storage authorization"),
        (status = 403, description = "Token does not permit reads"),
        (status = 503, description = "Storage runtime or IdP unavailable")
    ),
    security(("authorization" = []))
)]
pub async fn list_databases(
    State(state): State<ResourceState>,
    headers: HeaderMap,
) -> Result<Json<Vec<DatabaseResource>>, StatusCode> {
    let (scope, can_read, _) =
        namespace(authorization(&state, &headers).await?, "storage:databases")?;
    if !can_read {
        return Err(StatusCode::FORBIDDEN);
    }
    let resources = state
        .databases
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?
        .list(&scope)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    authorize_response(&state, &headers, &scope, "storage:databases", false).await?;
    Ok(Json(resources))
}

#[utoipa::path(
    get,
    path = "/storage/databases/{database_id}",
    params(("database_id" = String, Path, description = "Database resource ID")),
    responses(
        (status = 200, description = "Database resource", body = DatabaseDetailResponse),
        (status = 401, description = "Invalid storage authorization"),
        (status = 403, description = "Token does not permit reads"),
        (status = 404, description = "Database not found"),
        (status = 503, description = "Storage runtime or IdP unavailable")
    ),
    security(("authorization" = []))
)]
pub async fn get_database(
    State(state): State<ResourceState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<DatabaseDetailResponse>, StatusCode> {
    let database_id = DatabaseId::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let (scope, can_read, _) =
        namespace(authorization(&state, &headers).await?, "storage:databases")?;
    if !can_read {
        return Err(StatusCode::FORBIDDEN);
    }
    let resource = state
        .databases
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?
        .get(&scope, database_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    authorize_response(&state, &headers, &scope, "storage:databases", false).await?;
    resource
        .map(|resource| {
            Json(DatabaseDetailResponse {
                id: resource.id,
                name: resource.name,
                application_id: scope.application_id.to_string(),
            })
        })
        .ok_or(StatusCode::NOT_FOUND)
}

#[utoipa::path(
    delete,
    path = "/storage/databases/{database_id}",
    params(("database_id" = String, Path, description = "Database resource ID")),
    responses(
        (status = 204, description = "Database resource tombstoned"),
        (status = 401, description = "Invalid storage authorization"),
        (status = 403, description = "Token does not permit writes"),
        (status = 404, description = "Database resource not found"),
        (status = 503, description = "Storage runtime or IdP unavailable")
    ),
    security(("authorization" = []))
)]
pub async fn delete_database(
    State(state): State<ResourceState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let database_id = DatabaseId::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let (scope, _, can_write) =
        namespace(authorization(&state, &headers).await?, "storage:databases")?;
    if !can_write {
        return Err(StatusCode::FORBIDDEN);
    }
    state
        .databases
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?
        .delete(&scope, database_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    authorize_response(&state, &headers, &scope, "storage:databases", true).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/storage/filesystems",
    request_body = CreateResourceRequest,
    responses(
        (status = 201, description = "Filesystem resource created", body = FileSystemResourceResponse),
        (status = 401, description = "Invalid storage authorization"),
        (status = 403, description = "Token does not permit writes"),
        (status = 503, description = "Storage runtime or IdP unavailable")
    ),
    security(("authorization" = []))
)]
pub async fn create_file_system(
    State(state): State<ResourceState>,
    headers: HeaderMap,
    Json(request): Json<CreateResourceRequest>,
) -> Result<(StatusCode, Json<FileSystemResourceResponse>), StatusCode> {
    let (scope, _, can_write) = namespace(
        authorization(&state, &headers).await?,
        "storage:filesystems",
    )?;
    if !can_write {
        return Err(StatusCode::FORBIDDEN);
    }
    let resource = state
        .file_systems
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?
        .create_resource(&scope, request.name)
        .await
        .map_err(|error| {
            log::error!("Storage filesystem creation failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    authorize_response(&state, &headers, &scope, "storage:filesystems", true).await?;
    Ok((StatusCode::CREATED, Json(file_system_response(resource))))
}

#[utoipa::path(
    get,
    path = "/storage/filesystems",
    responses(
        (status = 200, description = "List of filesystem resources", body = [FileSystemResourceResponse]),
        (status = 401, description = "Invalid storage authorization"),
        (status = 403, description = "Token does not permit reads"),
        (status = 503, description = "Storage runtime or IdP unavailable")
    ),
    security(("authorization" = []))
)]
pub async fn list_file_systems(
    State(state): State<ResourceState>,
    headers: HeaderMap,
) -> Result<Json<Vec<FileSystemResourceResponse>>, StatusCode> {
    let (scope, can_read, _) = namespace(
        authorization(&state, &headers).await?,
        "storage:filesystems",
    )?;
    if !can_read {
        return Err(StatusCode::FORBIDDEN);
    }
    let resources = state
        .file_systems
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?
        .list_resources(&scope)
        .await
        .map_err(|error| {
            log::error!("Storage filesystem list failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    authorize_response(&state, &headers, &scope, "storage:filesystems", false).await?;
    Ok(Json(
        resources.into_iter().map(file_system_response).collect(),
    ))
}

#[utoipa::path(
    get,
    path = "/storage/filesystems/{filesystem_id}",
    params(("filesystem_id" = String, Path, description = "Filesystem resource ID")),
    responses(
        (status = 200, description = "Filesystem resource", body = FileSystemDetailResponse),
        (status = 401, description = "Invalid storage authorization"),
        (status = 403, description = "Token does not permit reads"),
        (status = 404, description = "Filesystem not found"),
        (status = 503, description = "Storage runtime or IdP unavailable")
    ),
    security(("authorization" = []))
)]
pub async fn get_file_system(
    State(state): State<ResourceState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<FileSystemDetailResponse>, StatusCode> {
    let (scope, can_read, _) = namespace(
        authorization(&state, &headers).await?,
        "storage:filesystems",
    )?;
    if !can_read {
        return Err(StatusCode::FORBIDDEN);
    }
    let filesystem_id = FileSystemId::parse(&id).map_err(|_| StatusCode::NOT_FOUND)?;
    let resource = state
        .file_systems
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?
        .list_resources(&scope)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .find(|resource| resource.id == filesystem_id);
    authorize_response(&state, &headers, &scope, "storage:filesystems", false).await?;
    resource
        .map(|resource| {
            Json(FileSystemDetailResponse {
                id: resource.id.as_uuid().to_string(),
                name: resource.name,
                application_id: scope.application_id.to_string(),
            })
        })
        .ok_or(StatusCode::NOT_FOUND)
}

#[utoipa::path(
    delete,
    path = "/storage/filesystems/{filesystem_id}",
    params(("filesystem_id" = String, Path, description = "Filesystem resource ID")),
    responses(
        (status = 204, description = "Filesystem deleted"),
        (status = 401, description = "Invalid storage authorization"),
        (status = 403, description = "Token does not permit writes"),
        (status = 404, description = "Filesystem not found"),
        (status = 503, description = "Storage runtime or IdP unavailable")
    ),
    security(("authorization" = []))
)]
pub async fn delete_file_system(
    State(state): State<ResourceState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let (scope, _, can_write) = namespace(
        authorization(&state, &headers).await?,
        "storage:filesystems",
    )?;
    if !can_write {
        return Err(StatusCode::FORBIDDEN);
    }
    let filesystem_id = FileSystemId::parse(&id).map_err(|_| StatusCode::NOT_FOUND)?;
    let deleted = state
        .file_systems
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?
        .delete_resource(&scope, filesystem_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    authorize_response(&state, &headers, &scope, "storage:filesystems", true).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http::header::AUTHORIZATION;
    use of_model::storage::namespace::StorageNamespace;
    use tower::ServiceExt;

    #[derive(Clone)]
    struct TestNamespace {
        user_sub: String,
        application_id: Id,
    }

    impl StorageNamespace for TestNamespace {
        fn user_sub(&self) -> &str {
            &self.user_sub
        }
        fn application_id(&self) -> Id {
            self.application_id
        }
    }

    fn router() -> Router {
        resource_router(
            RouterState::new("https://test.example", "endpoint"),
            None,
            None,
        )
    }

    #[tokio::test]
    async fn rejects_missing_bearer_token() {
        let response = router()
            .oneshot(
                Request::builder()
                    .uri("/storage/databases")
                    .method("GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn fails_closed_when_idp_client_is_not_configured() {
        let response = router()
            .oneshot(
                Request::builder()
                    .uri("/storage/databases")
                    .method("GET")
                    .header(AUTHORIZATION, "Bearer token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}

#[utoipa::path(
    post,
    path = "/sync/connect",
    request_body = SyncConnectRequest,
    responses(
        (status = 200, description = "Sync session established", body = SyncSessionResponse),
        (status = 401, description = "Invalid storage authorization"),
        (status = 403, description = "Token does not permit writes"),
        (status = 503, description = "Storage runtime or IdP unavailable")
    ),
    security(("authorization" = []))
)]
pub async fn sync_connect(
    State(state): State<ResourceState>,
    headers: HeaderMap,
    Json(request): Json<SyncConnectRequest>,
) -> Result<Json<SyncSessionResponse>, StatusCode> {
    let (scope, can_read, can_write) =
        namespace(authorization(&state, &headers).await?, "storage:databases")?;
    if !(can_read || can_write) {
        return Err(StatusCode::FORBIDDEN);
    }

    // Verify the resource exists and user has access
    let databases = state
        .databases
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let resource = databases
        .get(&scope, request.resource_id.into())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if resource.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    // Verify target device has this resource selected for sync via Management Service
    let source_device_id =
        Id::parse_str(&state.router.endpoint_id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let target_device_id = Id::from(request.target_device);
    let resource_id = request.resource_id;

    let management_client = state
        .router
        .management_client
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let has_selection = management_client
        .has_selection(target_device_id, resource_id)
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    if !has_selection {
        return Err(StatusCode::NOT_FOUND);
    }

    // Also verify source device has selection (should be true if they're calling this)
    let source_has_selection = management_client
        .has_selection(source_device_id, resource_id)
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    if !source_has_selection {
        return Err(StatusCode::FORBIDDEN);
    }

    let session_id = Uuid::new_v4();
    let response = SyncSessionResponse {
        session_id,
        resource_id: request.resource_id,
        source_device: scope.application_id.into(),
        target_device: request.target_device,
        status: of_model::storage::sync::SyncStatus::Connecting,
        bytes_sent: 0,
        bytes_received: 0,
    };

    Ok(Json(response))
}

fn file_system_response(resource: FileSystemResource) -> FileSystemResourceResponse {
    FileSystemResourceResponse {
        id: resource.id.as_uuid().to_string(),
        name: resource.name,
    }
}
