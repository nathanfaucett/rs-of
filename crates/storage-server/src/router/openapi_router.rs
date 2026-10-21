use api::SecurityAddon;
use utoipa::OpenApi;
use utoipa::openapi::{OpenApi as OpenApiDocument, Paths, RefOr, Schema, Server};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::RouterState;

use super::openapi::{__path_openapi_json, openapi_json};
use super::resources;
use super::routes::health::{__path_health, health};
use super::routes::version::{__path_version, version};
use super::storage_socket::__path_upgrade_socket;

#[derive(OpenApi)]
#[openapi(
    paths(
        resources::create_database,
        resources::list_databases,
        resources::get_database,
        resources::delete_database,
        resources::create_file_system,
        resources::list_file_systems,
        resources::get_file_system,
        resources::delete_file_system,
        resources::residency::get_file_system_residency,
        resources::residency::put_file_system_residency,
        upgrade_socket
    ),
    components(schemas(
        resources::CreateResourceRequest,
        resources::DatabaseDetailResponse,
        resources::FileSystemResourceResponse,
        resources::FileSystemDetailResponse,
        resources::residency::ResidencyMode,
        resources::residency::ResourceResidency
    )),
    info(title = "Storage Server", version = env!("CARGO_PKG_VERSION")),
    modifiers(&SecurityAddon)
)]
pub(crate) struct ApiDoc;

fn routes() -> OpenApiRouter<RouterState> {
    OpenApiRouter::new()
        .routes(routes!(health))
        .routes(routes!(version))
}

/// Builds the runtime contract without provisioning runtime state.
/// Appends `prefix` to the server base and strips it from documented paths.
pub fn openapi_document(api_base_uri: &str, prefix: &str) -> OpenApiDocument {
    let prefix = if prefix == "/" { "" } else { prefix };
    let spec_router = OpenApiRouter::with_openapi(ApiDoc::openapi()).merge(routes());

    let mut openapi_spec = spec_router.get_openapi().clone();

    openapi_spec.servers = Some(vec![Server::new(format!("{api_base_uri}{prefix}"))]);

    if !prefix.is_empty() {
        let mut paths = Paths::new();
        for (path, item) in openapi_spec.paths.paths {
            let path = path.strip_prefix(prefix).unwrap_or(&path);
            let path = if path.is_empty() { "/" } else { path }.to_owned();
            paths.paths.insert(path, item);
        }
        openapi_spec.paths = paths;
    }

    let mut schemas = Vec::<(String, RefOr<Schema>)>::new();
    let (openapi_json_path, openapi_json_item, openapi_json_types) =
        routes!(@resolve_types openapi_json : schemas);

    openapi_spec.paths.add_path_operation(
        &openapi_json_path,
        openapi_json_types,
        openapi_json_item,
    );

    openapi_spec
}

pub fn openapi_router(router_state: RouterState, prefix: &str) -> OpenApiRouter {
    let openapi_spec = openapi_document(&router_state.api_base_uri, prefix);
    let prefix = if prefix == "/" { "" } else { prefix };
    let runtime_router = if prefix.is_empty() {
        OpenApiRouter::with_openapi(ApiDoc::openapi()).merge(routes().with_state(router_state))
    } else {
        OpenApiRouter::with_openapi(ApiDoc::openapi())
            .nest(prefix, routes().with_state(router_state))
    };

    let openapi_json_routes = OpenApiRouter::new()
        .routes(routes!(openapi_json))
        .with_state(openapi_spec);

    if prefix.is_empty() {
        runtime_router.merge(openapi_json_routes)
    } else {
        runtime_router.nest(prefix, openapi_json_routes)
    }
}

#[cfg(test)]
mod tests {
    use utoipa::OpenApi;

    use super::ApiDoc;

    #[test]
    fn openapi_document_preserves_runtime_contract_without_state() {
        let registered = utoipa_axum::router::OpenApiRouter::with_openapi(ApiDoc::openapi())
            .merge(super::routes());
        let original = serde_json::to_value(registered.get_openapi()).expect("Storage contract");
        for prefix in ["", "/", "/storage", "/custom/storage"] {
            let base = "https://node.example/custom/storage";
            let document = serde_json::to_value(crate::openapi_document(base, prefix))
                .expect("state-free Storage contract");
            let prefix = if prefix == "/" { "" } else { prefix };
            assert_eq!(document["servers"][0]["url"], format!("{base}{prefix}"));
            assert_eq!(document["components"], original["components"]);
            for (path, operation) in original["paths"].as_object().expect("Storage paths") {
                let relative = if prefix.is_empty() {
                    path.as_str()
                } else {
                    path.strip_prefix(prefix).unwrap_or(path)
                };
                let relative = if relative.is_empty() { "/" } else { relative };
                assert_eq!(document["paths"][relative], *operation, "{path}");
            }
            let paths = document["paths"].as_object().expect("owner-relative paths");
            assert_eq!(
                paths.len(),
                original["paths"].as_object().expect("Storage paths").len() + 1
            );
            for path in ["/health", "/version", "/openapi.json"] {
                assert!(paths.contains_key(path), "missing {path}");
            }
        }
    }

    #[tokio::test]
    async fn standalone_runtime_openapi_retains_storage_paths_and_server_base() {
        use axum::{
            body::{Body, to_bytes},
            http::{Request, StatusCode},
        };
        use tower::ServiceExt;

        let base = "https://standalone.example";
        let (router, _) =
            crate::openapi_router(crate::RouterState::new(base), "").split_for_parts();
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/openapi.json")
                    .body(Body::empty())
                    .expect("standalone document request"),
            )
            .await
            .expect("standalone document response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("document body");
        let document: serde_json::Value = serde_json::from_slice(&body).expect("document JSON");
        assert_eq!(document["servers"][0]["url"], base);
        assert!(document["paths"]["/storage/databases"].is_object());
        assert!(document["paths"]["/storage/filesystems"].is_object());
        assert!(document["paths"]["/storage"].is_object());
        assert!(document["paths"]["/databases"].is_null());
    }

    #[test]
    fn documents_storage_routes() {
        let document = ApiDoc::openapi();
        for path in [
            "/storage/databases",
            "/storage/databases/{database_id}",
            "/storage/filesystems",
            "/storage/filesystems/{filesystem_id}",
            "/storage/filesystems/{filesystem_id}/residency",
            "/storage",
        ] {
            assert!(document.paths.paths.contains_key(path), "missing {path}");
        }
    }
}
