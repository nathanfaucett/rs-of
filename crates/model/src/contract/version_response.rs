#[cfg(not(feature = "std"))]
use alloc::{
    boxed::Box,
    format,
    string::{String, ToString},
    vec::Vec,
};

use build_time::build_time_utc;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(
    feature = "wasm",
    derive(tsify::Tsify),
    tsify(into_wasm_abi, from_wasm_abi)
)]
pub struct VersionResponse {
    pub name: String,
    pub version: String,
    pub build: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
}

impl Default for VersionResponse {
    fn default() -> Self {
        Self {
            name: env!("CARGO_PKG_NAME").to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            build: DateTime::parse_from_rfc3339(build_time_utc!())
                .expect("invalid build time")
                .with_timezone(&Utc),
            features: Vec::new(),
        }
    }
}
