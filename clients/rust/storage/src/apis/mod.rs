mod support;
use support::ContentType;
pub use support::{Error, ResponseContent, parse_deep_object, urlencode};
pub mod default_api;
pub mod resources_api;
pub mod resources_residency_api;

pub mod configuration;
