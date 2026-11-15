use server_common::openapi::SecurityAddon;
use crate::router::openapi_router::openapi_router::openapi_router::utoipa::OpenApi;
use crate::router::openapi_router::openapi_router::openapi_router::utoipa::openapi::OpenApi as OpenApiDocument;
use crate::router::openapi_router::openapi_router::openapi_router::utoipa::openapi::RefOr;
use crate::router::openapi_router::openapi_router::openapi_router::utoipa::openapi::Schema;
use crate::router::openapi_router::openapi_router::openapi_router::utoipa::openapi::Server;
use crate::router::openapi_router::openapi_router::utoipa_axum::crate::router::openapi_router::openapi_router::router::OpenApiRouter;
use crate::router::openapi_router::openapi_router::utoipa_axum::routes;

use crate::router::state::RouterState;

use crate::router::openapi_router::openapi_router::openapi_router::openapi::__path_openapi_json;

use crate::router::openapi_router::openapi_router::openapi_router::openapi::openapi_json;

use crate::router::openapi_router::openapi_router::openapi_router::routes::device::__path_device;

use crate::router::openapi_router::openapi_router::openapi_router::routes::device::device;
use crate::router::openapi_router::openapi_router::openapi_router::routes::device_restriction::__path_get_device_restriction;
use crate::router::openapi_router::openapi_router::openapi_router::routes::device_restriction::__path_put_device_restriction;
use crate::router::openapi_router::openapi_router::openapi_router::routes::device_restriction::get_device_restriction;
use crate::router::openapi_router::openapi_router::openapi_router::routes::device_restriction::put_device_restriction;
use crate::router::openapi_router::openapi_router::openapi_router::routes::device_selection::__path_delete_device_resource_selection;
use crate::router::openapi_router::openapi_router::openapi_router::routes::device_selection::__path_delete_device_selection;
use crate::router::openapi_router::openapi_router::openapi_router::routes::device_selection::__path_put_device_selection;
use crate::router::openapi_router::openapi_router::openapi_router::routes::device_selection::delete_device_resource_selection;
use crate::router::openapi_router::openapi_router::openapi_router::routes::device_selection::delete_device_selection;
use crate::router::openapi_router::openapi_router::openapi_router::routes::device_selection::put_device_selection;
use crate::router::openapi_router::openapi_router::openapi_router::routes::device_self_revocation::__path_revoke_self;
use crate::router::openapi_router::openapi_router::openapi_router::routes::device_self_revocation::revoke_self;
use crate::router::openapi_router::openapi_router::openapi_router::routes::devices::__path_approve_enrollment;
use crate::router::openapi_router::openapi_router::openapi_router::routes::devices::__path_enroll_device;
use crate::router::openapi_router::openapi_router::openapi_router::routes::devices::__path_list_approved_device_endpoints;
use crate::router::openapi_router::openapi_router::openapi_router::routes::devices::__path_list_devices;
use crate::router::openapi_router::openapi_router::openapi_router::routes::devices::__path_list_pending_enrollments;
use crate::router::openapi_router::openapi_router::openapi_router::routes::devices::__path_lookup_device_endpoint;
use crate::router::openapi_router::openapi_router::openapi_router::routes::devices::__path_lookup_device_identity;
use crate::router::openapi_router::openapi_router::openapi_router::routes::devices::__path_pairing_acceptance;
use crate::router::openapi_router::openapi_router::openapi_router::routes::devices::__path_revoke_device;
use crate::router::openapi_router::openapi_router::openapi_router::routes::devices::__path_set_pairing_acceptance;
use crate::router::openapi_router::openapi_router::openapi_router::routes::devices::__path_transfer_device;
use crate::router::openapi_router::openapi_router::openapi_router::routes::devices::__path_trusted_devices;
use crate::router::openapi_router::openapi_router::openapi_router::routes::devices::__path_update_device;
use crate::router::openapi_router::openapi_router::openapi_router::routes::devices::approve_enrollment;
use crate::router::openapi_router::openapi_router::openapi_router::routes::devices::enroll_device;
use crate::router::openapi_router::openapi_router::openapi_router::routes::devices::list_approved_device_endpoints;
use crate::router::openapi_router::openapi_router::openapi_router::routes::devices::list_devices;
use crate::router::openapi_router::openapi_router::openapi_router::routes::devices::list_pending_enrollments;
use crate::router::openapi_router::openapi_router::openapi_router::routes::devices::lookup_device_endpoint;
use crate::router::openapi_router::openapi_router::openapi_router::routes::devices::lookup_device_identity;
use crate::router::openapi_router::openapi_router::openapi_router::routes::devices::pairing_acceptance;
use crate::router::openapi_router::openapi_router::openapi_router::routes::devices::revoke_device;
use crate::router::openapi_router::openapi_router::openapi_router::routes::devices::set_pairing_acceptance;
use crate::router::openapi_router::openapi_router::openapi_router::routes::devices::transfer_device;
use crate::router::openapi_router::openapi_router::openapi_router::routes::devices::trusted_devices;
use crate::router::openapi_router::openapi_router::openapi_router::routes::devices::update_device;
use crate::router::openapi_router::openapi_router::openapi_router::routes::health::__path_health;
use crate::router::openapi_router::openapi_router::openapi_router::routes::health::health;
use crate::router::openapi_router::openapi_router::openapi_router::routes::replication::__path_owner_cleanup;
use crate::router::openapi_router::openapi_router::openapi_router::routes::replication::__path_replication_admission;
use crate::router::openapi_router::openapi_router::openapi_router::routes::replication::__path_selected_resources;
use crate::router::openapi_router::openapi_router::openapi_router::routes::replication::owner_cleanup;
use crate::router::openapi_router::openapi_router::openapi_router::routes::replication::replication_admission;
use crate::router::openapi_router::openapi_router::openapi_router::routes::replication::selected_resources;
use crate::router::openapi_router::openapi_router::openapi_router::routes::version::__path_version;
use crate::router::openapi_router::openapi_router::openapi_router::routes::version::version;

#[derive(OpenApi)]
#[openapi(
    info(title = "LIDP Device API", version = env!("CARGO_PKG_VERSION")),
    paths(
        crate::router::openapi_router::openapi_router::routes::replication::selected_resources,
        crate::router::openapi_router::openapi_router::routes::replication::replication_admission,
        crate::router::openapi_router::openapi_router::routes::replication::owner_cleanup
    ),
    modifiers(&SecurityAddon)
)]
pub(crate) struct ApiDoc;

fn routes() -> OpenApiRouter<RouterState> {
    crate::router::openapi_router::openapi_router::OpenApiRouter::new()
        .routes(routes!(health))
        .routes(routes!(device))
        .routes(routes!(revoke_self))
        .routes(routes!(transfer_device))
        .routes(routes!(lookup_device_endpoint))
        .routes(routes!(lookup_device_identity))
        .routes(routes!(list_approved_device_endpoints))
        .routes(routes!(trusted_devices))
        .routes(routes!(enroll_device))
        .routes(routes!(list_pending_enrollments))
        .routes(routes!(approve_enrollment))
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
    let spec_router = crate::router::openapi_router::openapi_router::OpenApiRouter::with_openapi(
        crate::router::openapi_router::openapi_router::ApiDoc::openapi(),
    )
    .merge(routes());

    let mut openapi_spec = spec_router.get_openapi().clone();

    let mut joins = crate::node_join_source::node_join_openapi();
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
    openapi_spec.servers = Some(vec![
        crate::router::openapi_router::openapi_router::Server::new(format!(
            "{}{}",
            api_base_uri, prefix
        )),
    ]);

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
        "device",
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

        for prefix in ["", "/", "/device", "/custom/device"] {
            let document =
                crate::router::openapi_router::openapi_document("https://device.example", prefix);
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
                29,
                "complete Management operation inventory"
            );
            assert_eq!(operations.iter().collect::<BTreeSet<_>>().len(), 29);
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
