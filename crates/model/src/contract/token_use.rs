#[cfg(not(feature = "std"))]
use alloc::{
    boxed::Box,
    format,
    string::{String, ToString},
    vec::Vec,
};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub enum TokenUse {
    #[serde(rename = "Id", alias = "id")]
    Id = 0,
    #[serde(rename = "Access", alias = "access")]
    Access = 1,
    #[serde(rename = "Refresh", alias = "refresh")]
    Refresh = 2,
}
