use utoipa::openapi::{
    OpenApi as Document,
    security::{ApiKey, ApiKeyValue, SecurityScheme},
};
use utoipa::{Modify, ToSchema};

#[derive(ToSchema)]
#[allow(dead_code)]
pub(crate) struct IndependentSetupStatus {
    stage: String,
    state: String,
    ready: bool,
    owners: Vec<String>,
}

pub(crate) struct SetupSecurity;
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
    let mut document = crate::independent_create::setup_routes()
        .get_openapi()
        .clone();
    document.info.title = "Independent owner setup".into();
    SetupSecurity.modify(&mut document);
    document
}

#[cfg(test)]
mod tests {
    #[test]
    fn complete_routes_include_operation_bound_handoffs() {
        let document = super::independent_setup_openapi();
        let paths = document.paths.paths;
        assert_eq!(paths.len(), 10);
        for path in [
            "/independent/setup/status",
            "/independent/setup/admission",
            "/independent/setup/create",
            "/independent/setup/join",
            "/independent/setup/intent",
            "/independent/setup/cancel",
            "/independent/setup/independent/public",
        ] {
            assert!(paths.contains_key(path), "actual public route {path}");
        }
        for path in [
            "/independent/setup/independent/challenge",
            "/independent/setup/independent/register",
            "/independent/setup/independent/history",
        ] {
            assert!(
                paths.contains_key(path),
                "worker handoff is part of the contract"
            );
        }
    }
}
