#[cfg(not(feature = "std"))]
use alloc::{
    boxed::Box,
    format,
    string::{String, ToString},
    vec::Vec,
};

use serde::{Deserialize, Serialize};

use crate::contract::health_status::HealthStatus;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct HealthResponse {
    pub database: HealthStatus,
}

impl HealthResponse {
    pub fn is_healthy(&self) -> bool {
        matches!(self.database, HealthStatus::Healthy)
    }
}
