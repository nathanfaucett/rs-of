use std::io;

use of_idp_client::models::OwnerSetupStatus;
use serde::Serialize;

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Failure {
    Retry,
    TrustConsent,
    TrustDeclined,
    Prerequisite,
    Repair,
}

impl Failure {
    pub(crate) fn startup(error: &io::Error) -> Self {
        if crate::native_host::needs_consent(error) {
            return Self::TrustConsent;
        }
        match error.kind() {
            io::ErrorKind::InvalidData | io::ErrorKind::NotFound => Self::Repair,
            io::ErrorKind::PermissionDenied | io::ErrorKind::Unsupported => Self::Prerequisite,
            _ => Self::Retry,
        }
    }
}

#[derive(Default, Serialize)]
pub(crate) struct HostObservation {
    pub base: Option<String>,
    pub status: Option<OwnerSetupStatus>,
    pub failure: Option<Failure>,
}
