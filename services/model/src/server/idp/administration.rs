use alloc::string::String;

use serde::Deserialize;

#[derive(Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::IntoParams))]
#[cfg_attr(feature = "utoipa", into_params(parameter_in = Query))]
pub struct Page {
    #[serde(default)]
    pub offset: u32,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

#[derive(Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct ResetPassword {
    pub password: String,
}

#[derive(Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct SetUserMfaRequest {
    pub method: String,
    pub enable: bool,
}

#[derive(Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct SetUserBlockedRequest {
    pub blocked: bool,
}

fn default_limit() -> u32 {
    50
}
