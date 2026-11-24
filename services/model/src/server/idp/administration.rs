use core::fmt;

#[cfg(not(feature = "std"))]
use alloc::string::String;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
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

impl fmt::Debug for ResetPassword {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResetPassword").finish_non_exhaustive()
    }
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct SetUserMfaRequest {
    pub method: String,
    pub enable: bool,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct SetUserBlockedRequest {
    pub blocked: bool,
}

const fn default_limit() -> u32 {
    50
}

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "std"))]
    use alloc::format;

    use super::ResetPassword;

    #[test]
    fn reset_password_debug_omits_the_password() {
        let request = ResetPassword {
            password: "reset-password-secret".into(),
        };
        let output = format!("{request:?}");

        assert!(output.contains("ResetPassword"));
        assert!(!output.contains("reset-password-secret"));
        assert!(!output.contains("password"));
    }
}
