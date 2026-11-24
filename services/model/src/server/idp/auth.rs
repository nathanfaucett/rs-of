#[cfg(not(feature = "std"))]
use alloc::string::String;

use serde::Deserialize;

use crate::idp::contract::application_registration::ApplicationRegistration;

#[derive(Debug, Deserialize)]
pub struct OnboardingRegistration {
    pub application: ApplicationRegistration,
    pub client_id: Option<String>,
}
