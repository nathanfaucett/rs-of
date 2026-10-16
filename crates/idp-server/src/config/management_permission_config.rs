use core::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagementPermissionConfig {
    pub api_base_uri: String,
    pub client_id: String,
    #[serde(skip_serializing)]
    pub client_secret: String,
}

impl fmt::Debug for ManagementPermissionConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ManagementPermissionConfig")
            .field("api_base_uri", &self.api_base_uri)
            .field("client_id", &self.client_id)
            .finish_non_exhaustive()
    }
}
