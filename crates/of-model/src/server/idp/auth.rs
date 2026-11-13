use alloc::string::String;

use serde::Deserialize;

use crate::idp::contract::ApplicationRegistration;

#[derive(Deserialize)]
pub struct OnboardingRegistration {
    pub application: ApplicationRegistration,
    pub client_id: Option<String>,
}
