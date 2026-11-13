use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Copy, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum ResidencyMode {
    Full,
    Passthrough,
}

#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct ResourceResidency {
    pub mode: ResidencyMode,
}
