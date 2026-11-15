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
use crate::router::openapi_router::openapi_router::openapi_router::routes::active_user::__path_active_user;
use crate::router::openapi_router::openapi_router::openapi_router::routes::active_user::active_user;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::__path_create_application;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::__path_create_user;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::__path_delete_application;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::__path_delete_user;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::__path_get_application;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::__path_get_user;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::__path_list_applications;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::__path_list_client_keys;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::__path_list_consents;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::__path_list_users;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::__path_reset_password;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::__path_revoke_client_keys;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::__path_revoke_consent;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::__path_rotate_client_key;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::__path_set_user_blocked;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::__path_set_user_mfa;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::__path_update_application;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::__path_update_user;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::create_application;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::create_user;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::delete_application;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::delete_user;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::get_application;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::get_user;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::list_applications;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::list_client_keys;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::list_consents;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::list_users;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::reset_password;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::revoke_client_keys;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::revoke_consent;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::rotate_client_key;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::set_user_blocked;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::set_user_mfa;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::update_application;
use crate::router::openapi_router::openapi_router::openapi_router::routes::administration::update_user;
use crate::router::openapi_router::openapi_router::openapi_router::routes::health::__path_health;
use crate::router::openapi_router::openapi_router::openapi_router::routes::health::health;
use crate::router::openapi_router::openapi_router::openapi_router::routes::replica_signers::__path_enroll_replica_signer;
use crate::router::openapi_router::openapi_router::openapi_router::routes::replica_signers::__path_revoke_replica_signer;
use crate::router::openapi_router::openapi_router::openapi_router::routes::replica_signers::__path_rotate_replica_signer;
use crate::router::openapi_router::openapi_router::openapi_router::routes::replica_signers::enroll_replica_signer;
use crate::router::openapi_router::openapi_router::openapi_router::routes::replica_signers::revoke_replica_signer;
use crate::router::openapi_router::openapi_router::openapi_router::routes::replica_signers::rotate_replica_signer;

use crate::router::openapi_router::openapi_router::openapi_router::routes::oauth2::approvals::__path_approve_for_user;

use crate::router::openapi_router::openapi_router::openapi_router::routes::oauth2::approvals::__path_is_allowed_for_user;

use crate::router::openapi_router::openapi_router::openapi_router::routes::oauth2::approvals::approve_for_user;

use crate::router::openapi_router::openapi_router::openapi_router::routes::oauth2::approvals::is_allowed_for_user;
use crate::router::openapi_router::openapi_router::openapi_router::routes::oauth2::auth::__path_authorize_json;
use crate::router::openapi_router::openapi_router::openapi_router::routes::oauth2::auth::__path_authorize_query;
use crate::router::openapi_router::openapi_router::openapi_router::routes::oauth2::auth::authorize_json;
use crate::router::openapi_router::openapi_router::openapi_router::routes::oauth2::auth::authorize_query;
use crate::router::openapi_router::openapi_router::openapi_router::routes::oauth2::client::__path_get_client;
use crate::router::openapi_router::openapi_router::openapi_router::routes::oauth2::client::get_client;
use crate::router::openapi_router::openapi_router::openapi_router::routes::oauth2::device::__path_device_auth;
use crate::router::openapi_router::openapi_router::openapi_router::routes::oauth2::device::__path_device_verify;
use crate::router::openapi_router::openapi_router::openapi_router::routes::oauth2::device::device_auth;
use crate::router::openapi_router::openapi_router::openapi_router::routes::oauth2::device::device_verify;
use crate::router::openapi_router::openapi_router::openapi_router::routes::oauth2::introspect::__path_introspect;
use crate::router::openapi_router::openapi_router::openapi_router::routes::oauth2::introspect::introspect;
use crate::router::openapi_router::openapi_router::openapi_router::routes::oauth2::register::__path_delete_register;
use crate::router::openapi_router::openapi_router::openapi_router::routes::oauth2::register::__path_get_register;
use crate::router::openapi_router::openapi_router::openapi_router::routes::oauth2::register::__path_put_register;
use crate::router::openapi_router::openapi_router::openapi_router::routes::oauth2::register::__path_register;
use crate::router::openapi_router::openapi_router::openapi_router::routes::oauth2::register::__path_rotate_client_secret;
use crate::router::openapi_router::openapi_router::openapi_router::routes::oauth2::register::delete_register;
use crate::router::openapi_router::openapi_router::openapi_router::routes::oauth2::register::get_register;
use crate::router::openapi_router::openapi_router::openapi_router::routes::oauth2::register::put_register;
use crate::router::openapi_router::openapi_router::openapi_router::routes::oauth2::register::register;
use crate::router::openapi_router::openapi_router::openapi_router::routes::oauth2::register::rotate_client_secret;
use crate::router::openapi_router::openapi_router::openapi_router::routes::oauth2::revoke::__path_revoke;
use crate::router::openapi_router::openapi_router::openapi_router::routes::oauth2::revoke::revoke;
use crate::router::openapi_router::openapi_router::openapi_router::routes::oauth2::sessions::__path_sessions_logout;
use crate::router::openapi_router::openapi_router::openapi_router::routes::oauth2::sessions::sessions_logout;
use crate::router::openapi_router::openapi_router::openapi_router::routes::oauth2::token::__path_token;
use crate::router::openapi_router::openapi_router::openapi_router::routes::oauth2::token::token;

use crate::router::openapi_router::openapi_router::openapi_router::routes::userinfo::__path_userinfo;

use crate::router::openapi_router::openapi_router::openapi_router::routes::userinfo::userinfo;
use crate::router::openapi_router::openapi_router::openapi_router::routes::version::__path_version;
use crate::router::openapi_router::openapi_router::openapi_router::routes::version::version;
use crate::router::openapi_router::openapi_router::openapi_router::routes::well_known::__path_jwks;
use crate::router::openapi_router::openapi_router::openapi_router::routes::well_known::__path_openid_configuration;
use crate::router::openapi_router::openapi_router::openapi_router::routes::well_known::jwks;
use crate::router::openapi_router::openapi_router::openapi_router::routes::well_known::openid_configuration;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::router::openapi_router::openapi_router::routes::oauth2::introspect::introspect,
        crate::router::openapi_router::openapi_router::routes::replica_signers::enroll_replica_signer,
        crate::router::openapi_router::openapi_router::routes::replica_signers::rotate_replica_signer,
        crate::router::openapi_router::openapi_router::routes::replica_signers::revoke_replica_signer,
        crate::router::openapi_router::openapi_router::routes::active_user::active_user,
    ),
    info(title = "OAuth Server", version = env!("CARGO_PKG_VERSION")),
    modifiers(&SecurityAddon)
)]
pub(crate) struct ApiDoc;

fn routes() -> OpenApiRouter<RouterState> {
    crate::router::openapi_router::openapi_router::OpenApiRouter::new()
        .routes(routes!(health))
        .routes(routes!(active_user))
        .routes(routes!(enroll_replica_signer))
        .routes(routes!(rotate_replica_signer))
        .routes(routes!(revoke_replica_signer))
        .routes(routes!(list_applications))
        .routes(routes!(create_application))
        .routes(routes!(get_application))
        .routes(routes!(update_application))
        .routes(routes!(delete_application))
        .routes(routes!(list_users))
        .routes(routes!(create_user))
        .routes(routes!(get_user))
        .routes(routes!(update_user))
        .routes(routes!(delete_user))
        .routes(routes!(reset_password))
        .routes(routes!(set_user_mfa))
        .routes(routes!(set_user_blocked))
        .routes(routes!(list_consents))
        .routes(routes!(revoke_consent))
        .routes(routes!(list_client_keys))
        .routes(routes!(rotate_client_key))
        .routes(routes!(revoke_client_keys))
        .routes(routes!(authorize_json))
        .routes(routes!(authorize_query))
        .routes(routes!(is_allowed_for_user))
        .routes(routes!(approve_for_user))
        .routes(routes!(device_auth))
        .routes(routes!(device_verify))
        .routes(routes!(introspect))
        .routes(routes!(register))
        .routes(routes!(get_register))
        .routes(routes!(get_client))
        .routes(routes!(delete_register))
        .routes(routes!(put_register))
        .routes(routes!(rotate_client_secret))
        .routes(routes!(token))
        .routes(routes!(revoke))
        .routes(routes!(sessions_logout))
        .routes(routes!(version))
        .routes(routes!(jwks))
        .routes(routes!(openid_configuration))
        .routes(routes!(userinfo))
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
        "idp",
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
        .expect("serialize registered IdP document");

        for prefix in ["", "/", "/idp", "/custom/idp"] {
            let document =
                crate::router::openapi_router::openapi_document("https://idp.example", prefix);
            let document =
                crate::router::openapi_router::openapi_router::serde_json::to_value(document)
                    .expect("serialize IdP document");
            let paths = document["paths"].as_object().expect("complete IdP paths");
            let operations = paths
                .values()
                .flat_map(|item| item.as_object().expect("path item").values())
                .filter_map(|operation| operation["operationId"].as_str())
                .collect::<Vec<_>>();
            assert_eq!(operations.len(), 44, "complete IdP operation inventory");
            assert_eq!(operations.iter().collect::<BTreeSet<_>>().len(), 44);
            assert!(document["paths"]["/openapi.json"]["get"]["operationId"].is_string());
            let mut registered_paths = paths.clone();
            registered_paths.remove("/openapi.json");
            assert_eq!(
                crate::router::openapi_router::openapi_router::serde_json::Value::Object(
                    registered_paths
                ),
                expected["paths"]
            );
            assert_eq!(document["components"], expected["components"]);
            assert_eq!(document["tags"], expected["tags"]);
            for schema in [
                "TokenResponse",
                "UserInfo",
                "ClientRegistration",
                "EnrollReplicaSigner",
            ] {
                assert!(
                    document["components"]["schemas"][schema].is_object(),
                    "missing {schema}"
                );
            }
            let prefix = if prefix == "/" { "" } else { prefix };
            assert_eq!(
                document["servers"][0]["url"],
                format!("https://idp.example{prefix}")
            );
        }
    }

    #[test]
    fn excludes_obsolete_bootstrap_and_keeps_idp_signer_routes() {
        let document =
            crate::router::openapi_router::openapi_document("https://idp.example", "/idp");
        assert!(!document.paths.paths.contains_key("/setup/bootstrap"));
        let enrollment = document
            .paths
            .paths
            .get("/replica-signers")
            .expect("IdP must publish signer enrollment");
        assert!(enrollment.post.is_some());
        let membership = document
            .paths
            .paths
            .get("/replica-signers/{member_id}")
            .expect("IdP must publish signer membership operations");
        assert!(membership.put.is_some());
        assert!(membership.delete.is_some());
        assert!(
            document
                .paths
                .paths
                .keys()
                .all(|path| !path.starts_with("/internal/") && !path.starts_with("/storage/")),
            "IdP must not publish internal or Storage routes"
        );
    }
}
