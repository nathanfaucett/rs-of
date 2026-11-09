use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
    routing::{get, post},
};
use idp_model::model::Id;
use iroh::EndpointId;
use serde::{Deserialize, Serialize};
use storage_service::{
    DatabaseId, DatabaseResource, FileSystemId, FileSystemResource, ScopedFileSystemRuntime,
};

use crate::{
    RouterState, StorageAuthorization, authorize_storage_token, scoped_file_system_socket_router,
};

#[path = "residency.rs"]
pub(super) mod residency;
#[path = "sql.rs"]
pub(super) mod sql;

#[derive(Clone)]
pub(super) struct ResourceState {
    pub router: RouterState,
    pub databases: Option<Arc<storage_service::DatabaseRuntime>>,
    pub file_systems: Option<Arc<ScopedFileSystemRuntime<EndpointId>>>,
}

#[derive(Debug, Eq, PartialEq)]
struct StorageNamespace {
    user_sub: String,
    application_id: Id,
}

impl storage_model::StorageNamespace for StorageNamespace {
    fn user_sub(&self) -> &str {
        &self.user_sub
    }

    fn application_id(&self) -> Id {
        self.application_id
    }
}

#[derive(Deserialize, utoipa::ToSchema)]
pub(super) struct CreateResourceRequest {
    name: Option<String>,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct EnsureDatabaseRequest {
    /// Case-sensitive application-chosen identity; 1–128 UTF-8 bytes.
    #[schema(min_length = 1, max_length = 128)]
    creation_key: String,
    name: Option<String>,
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
pub(super) async fn ensure_database(
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

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct FileSystemResourceResponse {
    id: String,
    name: Option<String>,
}

impl From<FileSystemResource> for FileSystemResourceResponse {
    fn from(resource: FileSystemResource) -> Self {
        Self {
            id: resource.id.as_uuid().to_string(),
            name: resource.name,
        }
    }
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct DatabaseDetailResponse {
    #[schema(value_type = String)]
    id: DatabaseId,
    name: Option<String>,
    application_id: String,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct FileSystemDetailResponse {
    id: String,
    name: Option<String>,
    application_id: String,
}

pub fn resource_router(
    router: RouterState,
    databases: Option<Arc<storage_service::DatabaseRuntime>>,
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
            get(residency::get_file_system_residency).put(residency::put_file_system_residency),
        )
        .route("/sync/connect", post(sync_connect))
        .with_state(state.clone());
    if let (Some(idp_client), Some(file_systems)) = (state.router.idp_client, state.file_systems) {
        router = router.merge(scoped_file_system_socket_router(idp_client, file_systems));
    }
    router
}

async fn authorization(
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
            crate::StorageAuthorizationError::InvalidToken => StatusCode::UNAUTHORIZED,
            crate::StorageAuthorizationError::ServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
        })
}

fn namespace(
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

async fn authorize_response(
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
pub(super) async fn create_database(
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
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    authorize_response(&state, &headers, &scope, "storage:databases", true).await?;
    Ok((StatusCode::CREATED, Json(resource)))
}

#[utoipa::path(
    get,
    path = "/storage/databases",
    responses(
        (status = 200, description = "Namespace database resources", body = [DatabaseResource]),
        (status = 401, description = "Invalid storage authorization"),
        (status = 403, description = "Token does not permit reads"),
        (status = 503, description = "Storage runtime or IdP unavailable")
    ),
    security(("authorization" = []))
)]
pub(super) async fn list_databases(
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
        (status = 404, description = "Database resource not found"),
        (status = 503, description = "Storage runtime or IdP unavailable")
    ),
    security(("authorization" = []))
)]
pub(super) async fn get_database(
    State(state): State<ResourceState>,
    headers: HeaderMap,
    Path(database_id): Path<DatabaseId>,
) -> Result<Json<DatabaseDetailResponse>, StatusCode> {
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
pub(super) async fn delete_database(
    State(state): State<ResourceState>,
    headers: HeaderMap,
    Path(database_id): Path<DatabaseId>,
) -> Result<StatusCode, StatusCode> {
    let (scope, _, can_write) =
        namespace(authorization(&state, &headers).await?, "storage:databases")?;
    if !can_write {
        return Err(StatusCode::FORBIDDEN);
    }
    let deleted = state
        .databases
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?
        .delete(&scope, database_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    authorize_response(&state, &headers, &scope, "storage:databases", true).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
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
pub(super) async fn create_file_system(
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
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    authorize_response(&state, &headers, &scope, "storage:filesystems", true).await?;
    Ok((StatusCode::CREATED, Json(resource.into())))
}

#[utoipa::path(
    get,
    path = "/storage/filesystems",
    responses(
        (status = 200, description = "Namespace filesystem resources", body = [FileSystemResourceResponse]),
        (status = 401, description = "Invalid storage authorization"),
        (status = 403, description = "Token does not permit reads"),
        (status = 503, description = "Storage runtime or IdP unavailable")
    ),
    security(("authorization" = []))
)]
pub(super) async fn list_file_systems(
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
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    authorize_response(&state, &headers, &scope, "storage:filesystems", false).await?;
    Ok(Json(resources.into_iter().map(Into::into).collect()))
}

#[utoipa::path(
    get,
    path = "/storage/filesystems/{filesystem_id}",
    params(("filesystem_id" = String, Path, description = "Filesystem resource ID")),
    responses(
        (status = 200, description = "Filesystem resource", body = FileSystemDetailResponse),
        (status = 401, description = "Invalid storage authorization"),
        (status = 403, description = "Token does not permit reads"),
        (status = 404, description = "Filesystem resource not found"),
        (status = 503, description = "Storage runtime or IdP unavailable")
    ),
    security(("authorization" = []))
)]
pub(super) async fn get_file_system(
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
    let id = FileSystemId::parse(&id).map_err(|_| StatusCode::NOT_FOUND)?;
    let resource = state
        .file_systems
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?
        .list_resources(&scope)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .find(|resource| resource.id == id);
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
        (status = 204, description = "Filesystem resource tombstoned"),
        (status = 401, description = "Invalid storage authorization"),
        (status = 403, description = "Token does not permit writes"),
        (status = 404, description = "Filesystem resource not found"),
        (status = 503, description = "Storage runtime or IdP unavailable")
    ),
    security(("authorization" = []))
)]
pub(super) async fn delete_file_system(
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
    let id = FileSystemId::parse(&id).map_err(|_| StatusCode::NOT_FOUND)?;
    let deleted = state
        .file_systems
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?
        .delete_resource(&scope, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    authorize_response(&state, &headers, &scope, "storage:filesystems", true).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    use axum::{
        body::Body,
        http::{Method, Request, StatusCode, header::AUTHORIZATION},
    };
    use idp_model::contract::IntrospectionResponse;
    use model::contract::{
        AuthorizationDetail, PrincipalType, StandardClaims, StorageAuthorizationAction,
        StorageAuthorizationDetail, TokenType, TokenUse,
    };
    use tower::ServiceExt;

    use crate::{IdpClient, RouterState, resource_router};

    fn router() -> axum::Router {
        resource_router(RouterState::new("http://storage.local"), None, None)
    }

    fn live_idp_client(introspection_status: &str, claims: StandardClaims) -> IdpClient {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind IdP HTTP stub");
        let address = listener.local_addr().expect("read IdP HTTP stub address");
        let introspection_body = serde_json::to_string(&IntrospectionResponse {
            claims,
            application_id: "00000000-0000-0000-0000-000000000001".to_owned(),
        })
        .expect("serialize introspection response");
        let introspection_status = introspection_status.to_owned();
        thread::spawn(move || {
            for (status, body) in [
                (
                    "200 OK".to_owned(),
                    r#"{"access_token":"service-token","token_type":"Bearer","expires_in":3600,"scope":"idp.token.validate","iss":"http://127.0.0.1"}"#.to_owned(),
                ),
                (introspection_status, introspection_body),
            ] {
                let (mut stream, _) = listener.accept().expect("accept IdP request");
                let mut request = [0; 4096];
                stream.read(&mut request).expect("read IdP request");
                write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())
                    .expect("write IdP response");
            }
        });
        IdpClient::new(
            &format!("http://{address}/idp/"),
            "storage-client",
            "storage-secret",
            "http://127.0.0.1",
            "storage:filesystems",
        )
        .expect("create IdP client")
    }

    fn storage_claims(
        principal_type: PrincipalType,
        audience: &str,
        actions: Vec<StorageAuthorizationAction>,
    ) -> StandardClaims {
        StandardClaims {
            r#type: TokenType::Bearer,
            r#use: TokenUse::Access,
            exp: i64::MAX,
            iat: 1,
            nbf: 1,
            iss: "http://127.0.0.1".to_owned(),
            aud: audience.to_owned(),
            client_id: "desktop".to_owned(),
            sub: "user-1".to_owned(),
            principal_type,
            resource: Some("storage:filesystems".to_owned()),
            authorization_details: Some(vec![AuthorizationDetail::Storage(
                StorageAuthorizationDetail { actions },
            )]),
            scope: Vec::new(),
        }
    }

    async fn request_status(client: IdpClient, method: Method) -> StatusCode {
        let app = resource_router(
            RouterState::new("http://storage.local").with_idp_client(client),
            None,
            None,
        );
        app.oneshot(
            Request::builder()
                .method(method)
                .uri("/storage/filesystems")
                .header(AUTHORIZATION, "Bearer user-token")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .expect("build storage request"),
        )
        .await
        .expect("storage request must complete")
        .status()
    }

    #[tokio::test]
    async fn rejects_client_principal_and_wrong_resource_audience() {
        let client_token = live_idp_client(
            "200 OK",
            storage_claims(
                PrincipalType::Client,
                "storage:filesystems",
                vec![StorageAuthorizationAction::Read],
            ),
        );
        assert_eq!(
            request_status(client_token, Method::GET).await,
            StatusCode::UNAUTHORIZED
        );

        let wrong_audience = live_idp_client(
            "200 OK",
            storage_claims(
                PrincipalType::User,
                "storage:databases",
                vec![StorageAuthorizationAction::Read],
            ),
        );
        assert_eq!(
            request_status(wrong_audience, Method::GET).await,
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn denies_write_when_token_only_allows_reads() {
        let client = live_idp_client(
            "200 OK",
            storage_claims(
                PrincipalType::User,
                "storage:filesystems",
                vec![StorageAuthorizationAction::Read],
            ),
        );
        assert_eq!(
            request_status(client, Method::POST).await,
            StatusCode::FORBIDDEN
        );
    }

    #[tokio::test]
    async fn maps_revoked_token_and_idp_outage_to_fail_closed_statuses() {
        let revoked = live_idp_client(
            "401 Unauthorized",
            storage_claims(
                PrincipalType::User,
                "storage:filesystems",
                vec![StorageAuthorizationAction::Read],
            ),
        );
        assert_eq!(
            request_status(revoked, Method::GET).await,
            StatusCode::UNAUTHORIZED
        );

        let unavailable = live_idp_client(
            "503 Service Unavailable",
            storage_claims(
                PrincipalType::User,
                "storage:filesystems",
                vec![StorageAuthorizationAction::Read],
            ),
        );
        assert_eq!(
            request_status(unavailable, Method::GET).await,
            StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[tokio::test]
    async fn routes_work_under_a_host_prefix_without_repeating_storage_prefix() {
        let response = axum::Router::new()
            .nest("/host", router())
            .oneshot(
                Request::builder()
                    .uri("/host/storage/databases")
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("request must complete");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn rejects_missing_bearer_token() {
        let response = router()
            .oneshot(
                Request::builder()
                    .uri("/storage/databases")
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("request must complete");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn fails_closed_when_idp_client_is_not_configured() {
        let response = router()
            .oneshot(
                Request::builder()
                    .uri("/storage/databases")
                    .header(AUTHORIZATION, "Bearer token")
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("request must complete");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}

use uuid::Uuid;

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SyncConnectRequest {
    #[schema(value_type = String)]
    pub resource_id: DatabaseId,
    pub target_device: Uuid,
    #[serde(default = "default_bidirectional")]
    pub _direction: SyncDirection,
}

fn default_bidirectional() -> SyncDirection {
    SyncDirection::Bidirectional
}

#[derive(Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) enum SyncDirection {
    Push,
    Pull,
    Bidirectional,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct SyncSessionResponse {
    pub session_id: Uuid,
    #[schema(value_type = String)]
    pub resource_id: DatabaseId,
    pub source_device: Uuid,
    pub target_device: Uuid,
    pub status: SyncStatus,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub(super) enum SyncStatus {
    Connecting,
    Syncing,
    Completed,
    Failed,
    Cancelled,
}

#[utoipa::path(
    post,
    path = "/sync/connect",
    request_body = SyncConnectRequest,
    responses(
        (status = 200, description = "Sync session started", body = SyncSessionResponse),
        (status = 401, description = "Invalid authorization"),
        (status = 403, description = "Token does not permit sync"),
        (status = 404, description = "Resource or target device not found"),
        (status = 503, description = "Service unavailable")
    ),
    security(("authorization" = []))
)]
pub(super) async fn sync_connect(
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
        status: SyncStatus::Connecting,
        bytes_sent: 0,
        bytes_received: 0,
    };

    Ok(Json(response))
}
