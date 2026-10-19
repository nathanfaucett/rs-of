use utoipa::openapi::{
    OpenApi as Document,
    security::{ApiKey, ApiKeyValue, SecurityScheme},
};
use utoipa::{Modify, OpenApi, ToSchema};

#[derive(ToSchema)]
#[allow(dead_code)]
pub(crate) struct IndependentSetupStatus {
    stage: String,
    state: String,
    ready: bool,
    owners: Vec<String>,
}

#[derive(OpenApi)]
#[openapi(
    info(title = "Independent owner setup", version = "0.1.0"),
    servers((url = "https://owner.example")),
    paths(
        super::independent_create::status,
        super::independent_create::admission,
        super::independent_create::create,
        super::independent_create::join,
        super::independent_create::recover,
        super::independent_create::cancel,
        super::independent_exchange::public,
    ),
    modifiers(&SetupSecurity)
)]
struct IndependentSetupDoc;
struct SetupSecurity;
impl Modify for SetupSecurity {
    fn modify(&self, document: &mut Document) {
        document
            .components
            .as_mut()
            .expect("setup schemas")
            .add_security_scheme(
                "setup_code",
                SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::with_description(
                    "Authorization",
                    "Setup <this owner's host-generated code>; never OAuth or another owner's code",
                ))),
            );
    }
}
pub fn independent_setup_openapi() -> Document {
    IndependentSetupDoc::openapi()
}

#[cfg(test)]
mod tests {
    #[test]
    fn public_routes_exclude_internal_handoff_and_use_owner_relative_prefixes() {
        let document = super::independent_setup_openapi();
        let paths = document.paths.paths;
        assert_eq!(paths.len(), 7);
        for path in [
            "/setup/status",
            "/setup/admission",
            "/setup/create",
            "/setup/join",
            "/setup/intent",
            "/setup/cancel",
            "/setup/independent/public",
        ] {
            assert!(paths.contains_key(path), "actual public route {path}");
        }
        for path in [
            "/setup/independent/challenge",
            "/setup/independent/register",
            "/setup/independent/history",
        ] {
            assert!(
                !paths.contains_key(path),
                "worker handoff is not browser API"
            );
        }
    }
    #[test]
    #[ignore = "explicit owner client generation supplies OF_INDEPENDENT_SETUP_OPENAPI_OUT"]
    fn export_independent_setup_openapi_for_client_generation() {
        let path =
            std::env::var_os("OF_INDEPENDENT_SETUP_OPENAPI_OUT").expect("explicit export path");
        std::fs::write(
            path,
            serde_json::to_vec_pretty(&super::independent_setup_openapi())
                .expect("actual public setup schema"),
        )
        .expect("requested schema destination");
    }
}
