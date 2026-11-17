use axum::http::header::AUTHORIZATION;
use utoipa::{
    Modify,
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
};

pub struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                AUTHORIZATION.to_string(),
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build(),
                ),
            );
        } else {
            log::warn!("OpenAPI components is None, cannot add security scheme");
        }
    }
}
