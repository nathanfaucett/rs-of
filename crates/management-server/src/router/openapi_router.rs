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

use crate::router::openapi_router::openapi_router::openapi_router::routes::health::__path_health;

use crate::router::openapi_router::openapi_router::openapi_router::routes::health::health;
use crate::router::openapi_router::openapi_router::openapi_router::routes::permission_evaluation::__path_evaluate_permission;
use crate::router::openapi_router::openapi_router::openapi_router::routes::permission_evaluation::evaluate_permission;
use crate::router::openapi_router::openapi_router::openapi_router::routes::permissions::__path_assign_permission_to_role;
use crate::router::openapi_router::openapi_router::openapi_router::routes::permissions::__path_create_permission;
use crate::router::openapi_router::openapi_router::openapi_router::routes::permissions::__path_delete_permission;
use crate::router::openapi_router::openapi_router::openapi_router::routes::permissions::__path_list_permissions;
use crate::router::openapi_router::openapi_router::openapi_router::routes::permissions::__path_list_role_permissions;
use crate::router::openapi_router::openapi_router::openapi_router::routes::permissions::__path_revoke_permission_from_role;
use crate::router::openapi_router::openapi_router::openapi_router::routes::permissions::assign_permission_to_role;
use crate::router::openapi_router::openapi_router::openapi_router::routes::permissions::create_permission;
use crate::router::openapi_router::openapi_router::openapi_router::routes::permissions::delete_permission;
use crate::router::openapi_router::openapi_router::openapi_router::routes::permissions::list_permissions;
use crate::router::openapi_router::openapi_router::openapi_router::routes::permissions::list_role_permissions;
use crate::router::openapi_router::openapi_router::openapi_router::routes::permissions::revoke_permission_from_role;
use crate::router::openapi_router::openapi_router::openapi_router::routes::roles::__path_assign_role_to_user;
use crate::router::openapi_router::openapi_router::openapi_router::routes::roles::__path_create_role;
use crate::router::openapi_router::openapi_router::openapi_router::routes::roles::__path_delete_role;
use crate::router::openapi_router::openapi_router::openapi_router::routes::roles::__path_list_roles;
use crate::router::openapi_router::openapi_router::openapi_router::routes::roles::__path_list_user_roles;
use crate::router::openapi_router::openapi_router::openapi_router::routes::roles::__path_revoke_role_from_user;
use crate::router::openapi_router::openapi_router::openapi_router::routes::roles::assign_role_to_user;
use crate::router::openapi_router::openapi_router::openapi_router::routes::roles::create_role;
use crate::router::openapi_router::openapi_router::openapi_router::routes::roles::delete_role;
use crate::router::openapi_router::openapi_router::openapi_router::routes::roles::list_roles;
use crate::router::openapi_router::openapi_router::openapi_router::routes::roles::list_user_roles;
use crate::router::openapi_router::openapi_router::openapi_router::routes::roles::revoke_role_from_user;
use crate::router::openapi_router::openapi_router::openapi_router::routes::sync_selection::__path_list_sync_selections;
use crate::router::openapi_router::openapi_router::openapi_router::routes::sync_selection::__path_list_sync_targets;
use crate::router::openapi_router::openapi_router::openapi_router::routes::sync_selection::__path_set_sync_selection;
use crate::router::openapi_router::openapi_router::openapi_router::routes::sync_selection::list_sync_selections;
use crate::router::openapi_router::openapi_router::openapi_router::routes::sync_selection::list_sync_targets;
use crate::router::openapi_router::openapi_router::openapi_router::routes::sync_selection::set_sync_selection;
use crate::router::openapi_router::openapi_router::openapi_router::routes::users::__path_list_user_roles_across_applications;
use crate::router::openapi_router::openapi_router::openapi_router::routes::users::list_user_roles_across_applications;
use crate::router::openapi_router::openapi_router::openapi_router::routes::version::__path_version;
use crate::router::openapi_router::openapi_router::openapi_router::routes::version::version;

#[derive(OpenApi)]
#[openapi(
    info(title = "LIDP Management API", version = env!("CARGO_PKG_VERSION")),
    modifiers(&SecurityAddon)
)]
pub(crate) struct ApiDoc;

fn routes() -> OpenApiRouter<RouterState> {
    crate::router::openapi_router::openapi_router::OpenApiRouter::new()
        .routes(routes!(health))
        .routes(routes!(version))
        .routes(routes!(list_user_roles_across_applications))
        .routes(routes!(evaluate_permission))
        .routes(routes!(list_roles))
        .routes(routes!(create_role))
        .routes(routes!(delete_role))
        .routes(routes!(list_user_roles))
        .routes(routes!(assign_role_to_user))
        .routes(routes!(revoke_role_from_user))
        .routes(routes!(list_permissions))
        .routes(routes!(create_permission))
        .routes(routes!(delete_permission))
        .routes(routes!(list_role_permissions))
        .routes(routes!(assign_permission_to_role))
        .routes(routes!(revoke_permission_from_role))
        .routes(routes!(set_sync_selection))
        .routes(routes!(list_sync_selections))
        .routes(routes!(list_sync_targets))
}

/// Builds the complete production API document without provisioning runtime state.
pub fn openapi_document(api_base_uri: &str, prefix: &str) -> OpenApiDocument {
    let prefix = if prefix == "/" { "" } else { prefix };
    let spec_router = crate::router::openapi_router::openapi_router::OpenApiRouter::with_openapi(
        crate::router::openapi_router::openapi_router::ApiDoc::openapi(),
    )
    .merge(routes());

    let mut openapi_spec = spec_router.get_openapi().clone();

    openapi_spec.servers = Some(vec![
        crate::router::openapi_router::openapi_router::Server::new(format!(
            "{}{}",
            api_base_uri, prefix
        )),
    ]);

    if !prefix.is_empty() {
        let mut paths = crate::router::openapi_router::openapi_router::Paths::new();

        for (path, item) in openapi_spec.paths.paths {
            let path = path.strip_prefix(prefix).unwrap_or(&path).to_owned();

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
        "management",
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
    use crate::router::openapi_router::openapi_router::openapi_router::std::collections::BTreeSet;

    use crate::router::openapi_router::openapi_router::openapi_router::utoipa::OpenApi;
    use crate::router::openapi_router::openapi_router::openapi_router::utoipa_axum::router::OpenApiRouter;

    use crate::router::openapi_router::openapi_router::ApiDoc;

    use crate::router::openapi_router::openapi_router::routes;

    #[test]
    fn management_contract_excludes_device_authority() {
        let document = crate::router::openapi_router::openapi_document(
            "https://management.example",
            "/management",
        );
        for path in document.paths.paths.keys() {
            assert!(
                !path.starts_with("/device")
                    && !path.starts_with("/replication")
                    && !path.starts_with("/node-joins"),
                "Device authority must not be exposed by Management: {path}"
            );
        }
        assert!(
            document
                .paths
                .paths
                .contains_key("/applications/{application_id}/roles")
        );
    }

    #[test]
    fn openapi_document_preserves_full_registered_contract_without_state() {
        let registered =
            crate::router::openapi_router::openapi_router::OpenApiRouter::with_openapi(
                crate::router::openapi_router::openapi_router::ApiDoc::openapi(),
            )
            .merge(routes());
        let expected = crate::router::openapi_router::openapi_router::serde_json::to_value(
            registered.get_openapi(),
        )
        .expect("serialize registered Management document");

        for prefix in ["", "/", "/management", "/custom/management"] {
            let document = crate::router::openapi_router::openapi_document(
                "https://management.example",
                prefix,
            );
            let document =
                crate::router::openapi_router::openapi_router::serde_json::to_value(document)
                    .expect("serialize Management document");
            let paths = document["paths"]
                .as_object()
                .expect("complete Management paths");
            let operations = paths
                .values()
                .flat_map(|item| item.as_object().expect("path item").values())
                .filter_map(|operation| operation["operationId"].as_str())
                .collect::<Vec<_>>();
            assert_eq!(
                operations.len(),
                20,
                "complete Management operation inventory"
            );
            assert_eq!(operations.iter().collect::<BTreeSet<_>>().len(), 20);
            assert!(document["paths"]["/openapi.json"]["get"]["operationId"].is_string());
            let mut registered_paths = paths.clone();
            registered_paths.remove("/openapi.json");
            registered_paths.retain(|path, _| !path.starts_with("/node-joins/"));
            assert_eq!(
                crate::router::openapi_router::openapi_router::serde_json::Value::Object(
                    registered_paths
                ),
                expected["paths"]
            );
            for (name, schema) in expected["components"]["schemas"]
                .as_object()
                .expect("registered schemas")
            {
                assert_eq!(&document["components"]["schemas"][name], schema);
            }
            assert_eq!(
                document["components"]["securitySchemes"],
                expected["components"]["securitySchemes"]
            );
            assert_eq!(document["tags"], expected["tags"]);
            for schema in ["HealthResponse", "VersionResponse"] {
                assert!(
                    document["components"]["schemas"][schema].is_object(),
                    "missing {schema}"
                );
            }
            let prefix = if prefix == "/" { "" } else { prefix };
            assert_eq!(
                document["servers"][0]["url"],
                format!("https://management.example{prefix}")
            );
        }
    }
}
