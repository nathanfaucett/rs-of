#![forbid(unsafe_code)]
mod config;
mod provisioning;
mod router;
mod runtime;
pub use config::AppConfig;
pub use management_service::MANAGEMENT_APPLICATION_URI;
pub use provisioning::provision_initial_administrator;
pub use router::{RouterState, openapi_document, openapi_router, public_metadata_router};
pub use runtime::build_router;
