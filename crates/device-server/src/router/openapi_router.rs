use api::SecurityAddon;
use utoipa::OpenApi;
use utoipa::openapi::{OpenApi as OpenApiDocument, RefOr, Schema, Server};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::RouterState;

use super::openapi::{__path_openapi_json, openapi_json};

use super::routes::device::{__path_device, device};
use super::routes::device_restriction::{
    __path_get_device_restriction, __path_put_device_restriction, get_device_restriction,
    put_device_restriction,
};
use super::routes::device_selection::{
    __path_delete_device_resource_selection, __path_delete_device_selection,
    __path_put_device_selection, delete_device_resource_selection, delete_device_selection,
    put_device_selection,
};
use super::routes::device_self_revocation::{__path_revoke_self, revoke_self};
use super::routes::devices::{
    __path_enroll_device, __path_list_approved_device_endpoints, __path_list_devices,
    __path_lookup_device_endpoint, __path_lookup_device_identity, __path_pairing_acceptance,
    __path_revoke_device, __path_set_pairing_acceptance, __path_transfer_device,
    __path_trusted_devices, __path_update_device, enroll_device, list_approved_device_endpoints,
    list_devices, lookup_device_endpoint, lookup_device_identity, pairing_acceptance,
    revoke_device, set_pairing_acceptance, transfer_device, trusted_devices, update_device,
};
use super::routes::health::{__path_health, health};
use super::routes::replication::{
    __path_owner_cleanup, __path_replication_admission, __path_selected_resources, owner_cleanup,
    replication_admission, selected_resources,
};
use super::routes::version::{__path_version, version};

#[derive(OpenApi)]
#[openapi(
    info(title = "LIDP Device API", version = env!("CARGO_PKG_VERSION")),
    paths(
        super::routes::replication::selected_resources,
        super::routes::replication::replication_admission,
        super::routes::replication::owner_cleanup
    ),
    modifiers(&SecurityAddon)
)]
pub(crate) struct ApiDoc;

fn routes() -> OpenApiRouter<RouterState> {
    OpenApiRouter::new()
        .routes(routes!(health))
        .routes(routes!(device))
        .routes(routes!(revoke_self))
        .routes(routes!(transfer_device))
        .routes(routes!(lookup_device_endpoint))
        .routes(routes!(lookup_device_identity))
        .routes(routes!(list_approved_device_endpoints))
        .routes(routes!(trusted_devices))
        .routes(routes!(enroll_device))
        .routes(routes!(pairing_acceptance))
        .routes(routes!(set_pairing_acceptance))
        .routes(routes!(list_devices))
        .routes(routes!(update_device))
        .routes(routes!(revoke_device))
        .routes(routes!(version))
        .routes(routes!(get_device_restriction))
        .routes(routes!(put_device_restriction))
        .routes(routes!(put_device_selection))
        .routes(routes!(delete_device_selection))
        .routes(routes!(delete_device_resource_selection))
        .routes(routes!(selected_resources))
        .routes(routes!(replication_admission))
        .routes(routes!(owner_cleanup))
}

/// Builds the complete production API document without provisioning runtime state.
pub fn openapi_document(api_base_uri: &str, prefix: &str) -> OpenApiDocument {
    let prefix = if prefix == "/" { "" } else { prefix };
    let spec_router = OpenApiRouter::with_openapi(ApiDoc::openapi()).merge(routes());

    let mut openapi_spec = spec_router.get_openapi().clone();

    let mut joins = crate::node_join_openapi();
    joins.paths.paths = joins
        .paths
        .paths
        .into_iter()
        .map(|(path, mut item)| {
            for operation in [
                &mut item.get,
                &mut item.put,
                &mut item.post,
                &mut item.delete,
            ]
            .into_iter()
            .flatten()
            {
                operation.operation_id = Some(format!(
                    "node_join_{}",
                    operation
                        .operation_id
                        .as_deref()
                        .expect("Node join operation ID")
                ));
                operation.tags = Some(vec!["node_join".into()]);
            }
            (
                path.strip_prefix("/device")
                    .expect("Device Node-join path")
                    .to_owned(),
                item,
            )
        })
        .collect();
    openapi_spec.merge(joins);
    openapi_spec.servers = Some(vec![Server::new(format!("{}{}", api_base_uri, prefix))]);

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
    let openapi_spec = api::canonical_openapi("device", &router_state.api_base_uri, prefix);
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
    fn openapi_document_preserves_full_registered_contract_without_state() {
        let registered = OpenApiRouter::with_openapi(ApiDoc::openapi()).merge(routes());
        let expected = serde_json::to_value(registered.get_openapi())
            .expect("serialize registered Management document");

        for prefix in ["", "/", "/device", "/custom/device"] {
            let document = crate::openapi_document("https://device.example", prefix);
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
                29,
                "complete Management operation inventory"
            );
            assert_eq!(operations.iter().collect::<BTreeSet<_>>().len(), 29);
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
            for schema in [
                "SelectedResourcesResponse",
                "HealthResponse",
                "VersionResponse",
            ] {
                assert!(
                    document["components"]["schemas"][schema].is_object(),
                    "missing {schema}"
                );
            }
            let prefix = if prefix == "/" { "" } else { prefix };
            assert_eq!(
                document["servers"][0]["url"],
                format!("https://device.example{prefix}")
            );
        }
    }
}
