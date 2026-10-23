use api::SecurityAddon;
use utoipa::OpenApi;
use utoipa::openapi::{OpenApi as OpenApiDocument, Paths, RefOr, Schema, Server};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::RouterState;

use super::openapi::{__path_openapi_json, openapi_json};

use super::routes::health::{__path_health, health};
use super::routes::permission_evaluation::{__path_evaluate_permission, evaluate_permission};
use super::routes::permissions::{
    __path_assign_permission_to_role, __path_create_permission, __path_delete_permission,
    __path_list_permissions, __path_list_role_permissions, __path_revoke_permission_from_role,
    assign_permission_to_role, create_permission, delete_permission, list_permissions,
    list_role_permissions, revoke_permission_from_role,
};
use super::routes::roles::{
    __path_assign_role_to_user, __path_create_role, __path_delete_role, __path_list_roles,
    __path_list_user_roles, __path_revoke_role_from_user, assign_role_to_user, create_role,
    delete_role, list_roles, list_user_roles, revoke_role_from_user,
};
use super::routes::users::{
    __path_list_user_roles_across_applications, list_user_roles_across_applications,
};
use super::routes::version::{__path_version, version};

#[derive(OpenApi)]
#[openapi(
    info(title = "LIDP Management API", version = env!("CARGO_PKG_VERSION")),
    modifiers(&SecurityAddon)
)]
pub(crate) struct ApiDoc;

fn routes() -> OpenApiRouter<RouterState> {
    OpenApiRouter::new()
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
}

/// Builds the complete production API document without provisioning runtime state.
pub fn openapi_document(api_base_uri: &str, prefix: &str) -> OpenApiDocument {
    let prefix = if prefix == "/" { "" } else { prefix };
    let spec_router = OpenApiRouter::with_openapi(ApiDoc::openapi()).merge(routes());

    let mut openapi_spec = spec_router.get_openapi().clone();

    openapi_spec.servers = Some(vec![Server::new(format!("{}{}", api_base_uri, prefix))]);

    if !prefix.is_empty() {
        let mut paths = Paths::new();

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
    let openapi_spec = api::canonical_openapi("management", &router_state.api_base_uri, prefix);
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
    use std::collections::BTreeSet;

    use utoipa::OpenApi;
    use utoipa_axum::router::OpenApiRouter;

    use super::{ApiDoc, routes};

    #[test]
    fn management_contract_excludes_device_authority() {
        let document = crate::openapi_document("https://management.example", "/management");
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
        let registered = OpenApiRouter::with_openapi(ApiDoc::openapi()).merge(routes());
        let expected = serde_json::to_value(registered.get_openapi())
            .expect("serialize registered Management document");

        for prefix in ["", "/", "/management", "/custom/management"] {
            let document = crate::openapi_document("https://management.example", prefix);
            let document = serde_json::to_value(document).expect("serialize Management document");
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
                17,
                "complete Management operation inventory"
            );
            assert_eq!(operations.iter().collect::<BTreeSet<_>>().len(), 17);
            assert!(document["paths"]["/openapi.json"]["get"]["operationId"].is_string());
            let mut registered_paths = paths.clone();
            registered_paths.remove("/openapi.json");
            registered_paths.retain(|path, _| !path.starts_with("/node-joins/"));
            assert_eq!(
                serde_json::Value::Object(registered_paths),
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
