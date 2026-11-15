use server_common::openapi::SecurityAddon;
use crate::router::openapi_router::openapi_router::openapi_router::utoipa::OpenApi;
use crate::router::openapi_router::openapi_router::openapi_router::utoipa::openapi::OpenApi as OpenApiDocument;
use crate::router::openapi_router::openapi_router::openapi_router::utoipa::openapi::Paths;
use crate::router::openapi_router::openapi_router::openapi_router::utoipa::openapi::RefOr;
use crate::router::openapi_router::openapi_router::openapi_router::utoipa::openapi::Schema;
use crate::router::openapi_router::openapi_router::openapi_router::utoipa::openapi::Server;
use crate::router::openapi_router::openapi_router::utoipa_axum::crate::router::openapi_router::openapi_router::router::OpenApiRouter;
use crate::router::openapi_router::openapi_router::utoipa_axum::routes;

use crate::router::state::RouterState;

use crate::router::openapi_router::openapi_router::openapi_router::openapi::__path_openapi_json;

use crate::router::openapi_router::openapi_router::openapi_router::openapi::openapi_json;
use crate::router::openapi_router::openapi_router::openapi_router::resources;
use crate::router::openapi_router::openapi_router::openapi_router::routes::health::__path_health;
use crate::router::openapi_router::openapi_router::openapi_router::routes::health::health;
use crate::router::openapi_router::openapi_router::openapi_router::routes::version::__path_version;
use crate::router::openapi_router::openapi_router::openapi_router::routes::version::version;
use crate::router::openapi_router::openapi_router::openapi_router::storage_socket::__path_upgrade_socket;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::router::openapi_router::openapi_router::resources::create_database,
        crate::router::openapi_router::openapi_router::resources::ensure_database,
        crate::router::openapi_router::openapi_router::resources::sql::execute_sql,
        crate::router::openapi_router::openapi_router::resources::list_databases,
        crate::router::openapi_router::openapi_router::resources::get_database,
        crate::router::openapi_router::openapi_router::resources::delete_database,
        crate::router::openapi_router::openapi_router::resources::create_file_system,
        crate::router::openapi_router::openapi_router::resources::list_file_systems,
        crate::router::openapi_router::openapi_router::resources::get_file_system,
        crate::router::openapi_router::openapi_router::resources::delete_file_system,
        crate::router::openapi_router::openapi_router::resources::residency::get_file_system_residency,
        crate::router::openapi_router::openapi_router::resources::residency::put_file_system_residency,
        crate::router::openapi_router::openapi_router::resources::sync_connect,
        upgrade_socket
    ),
    components(schemas(
        crate::router::openapi_router::openapi_router::resources::CreateResourceRequest,
        crate::router::openapi_router::openapi_router::resources::DatabaseDetailResponse,
        crate::router::openapi_router::openapi_router::resources::FileSystemResourceResponse,
        crate::router::openapi_router::openapi_router::resources::FileSystemDetailResponse,
        crate::router::openapi_router::openapi_router::resources::residency::ResidencyMode,
        crate::router::openapi_router::openapi_router::resources::residency::ResourceResidency
    )),
    info(title = "Storage Server", version = env!("CARGO_PKG_VERSION")),
    modifiers(&SecurityAddon)
)]
pub(crate) struct ApiDoc;

fn routes() -> OpenApiRouter<RouterState> {
    crate::router::openapi_router::openapi_router::OpenApiRouter::new()
        .routes(routes!(health))
        .routes(routes!(version))
}

/// Builds the runtime contract without provisioning runtime state.
/// Appends `prefix` to the server base and strips it from documented paths.
pub fn openapi_document(api_base_uri: &str, prefix: &str) -> OpenApiDocument {
    let prefix = if prefix == "/" { "" } else { prefix };
    let spec_router = crate::router::openapi_router::openapi_router::OpenApiRouter::with_openapi(
        crate::router::openapi_router::openapi_router::ApiDoc::openapi(),
    )
    .merge(routes());

    let mut openapi_spec = spec_router.get_openapi().clone();

    openapi_spec.servers = Some(vec![
        crate::router::openapi_router::openapi_router::Server::new(format!(
            "{api_base_uri}{prefix}"
        )),
    ]);

    if !prefix.is_empty() {
        let mut paths = crate::router::openapi_router::openapi_router::Paths::new();
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
    let openapi_spec = server_common::canonical_openapi::canonical_openapi(
        "storage",
        &router_state.api_base_uri,
        prefix,
    );
    let prefix = if prefix == "/" { "" } else { prefix };
    let runtime_router = if prefix.is_empty() {
        crate::router::openapi_router::openapi_router::OpenApiRouter::with_openapi(
            crate::router::openapi_router::openapi_router::ApiDoc::openapi(),
        )
        .merge(routes().with_state(router_state))
    } else {
        crate::router::openapi_router::openapi_router::OpenApiRouter::with_openapi(
            crate::router::openapi_router::openapi_router::ApiDoc::openapi(),
        )
        .nest(prefix, routes().with_state(router_state))
    };

    let openapi_json_routes = crate::router::openapi_router::openapi_router::OpenApiRouter::new()
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
    use crate::router::openapi_router::openapi_router::openapi_router::utoipa::OpenApi;

    use crate::router::openapi_router::openapi_router::openapi_router::ApiDoc;

    #[test]
    fn openapi_document_preserves_runtime_contract_without_state() {
        let registered = crate::router::openapi_router::openapi_router::utoipa_axum::router::OpenApiRouter::with_openapi(crate::router::openapi_router::openapi_router::ApiDoc::openapi())
            .merge(crate::router::openapi_router::openapi_router::routes());
        let original = crate::router::openapi_router::openapi_router::serde_json::to_value(
            registered.get_openapi(),
        )
        .expect("Storage contract");
        for prefix in ["", "/", "/storage", "/custom/storage"] {
            let base = "https://node.example/custom/storage";
            let document = crate::router::openapi_router::openapi_router::serde_json::to_value(
                crate::router::openapi_router::openapi_document(base, prefix),
            )
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

    #[crate::router::openapi_router::openapi_router::tokio::test]
    async fn standalone_runtime_openapi_retains_storage_paths_and_server_base() {
        use crate::router::openapi_router::openapi_router::axum::body::Body;
        use crate::router::openapi_router::openapi_router::axum::body::to_bytes;
        use crate::router::openapi_router::openapi_router::axum::http::Request;
        use crate::router::openapi_router::openapi_router::axum::http::StatusCode;
        use crate::router::openapi_router::openapi_router::openapi_router::tower::ServiceExt;

        let base = "https://standalone.example";
        let (router, _) = crate::router::openapi_router::openapi_router(
            crate::router::state::RouterState::new(
                base,
                crate::router::openapi_router::openapi_router::iroh::SecretKey::generate()
                    .public()
                    .to_string(),
            ),
            "",
        )
        .split_for_parts();
        let response = router
            .oneshot(
                crate::router::openapi_router::openapi_router::Request::builder()
                    .uri("/openapi.json")
                    .body(crate::router::openapi_router::openapi_router::Body::empty())
                    .expect("standalone document request"),
            )
            .await
            .expect("standalone document response");
        assert_eq!(
            response.status(),
            crate::router::openapi_router::openapi_router::StatusCode::OK
        );
        let body = to_bytes(
            response.into_body(),
            crate::router::openapi_router::openapi_router::usize::MAX,
        )
        .await
        .expect("document body");
        let document: crate::router::openapi_router::openapi_router::serde_json::Value =
            crate::router::openapi_router::openapi_router::serde_json::from_slice(&body)
                .expect("document JSON");
        assert_eq!(document["servers"][0]["url"], base);
        let mut expected: crate::router::openapi_router::openapi_router::serde_json::Value =
            crate::router::openapi_router::openapi_router::serde_json::from_str(include_str!(
                "../../../../openapi/storage.json"
            ))
            .expect("exported Storage contract");
        expected["servers"] =
            crate::router::openapi_router::openapi_router::serde_json::json!([{ "url": base }]);
        assert_eq!(
            document, expected,
            "runtime Storage contract matches export"
        );
        assert!(document["paths"]["/databases"].is_object());
        assert!(document["paths"]["/filesystems"].is_object());
        assert!(document["paths"]["/"].is_object());
        assert!(document["paths"]["/storage/databases"].is_null());
    }

    #[test]
    fn documents_storage_routes() {
        let document = crate::router::openapi_router::openapi_router::ApiDoc::openapi();
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
