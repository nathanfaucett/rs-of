//! Management Service client for Storage Server.

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime},
};

use idp_model::model::Id;
use management_service::{SyncSelection, SyncSelectionRepo};
use reqwest::{Client, Url, redirect::Policy};

use crate::{IdpClient, IdpIntrospectionError};

#[derive(Clone)]
pub struct ManagementClient {
    management_base_url: Url,
    idp_base_url: Url,
    client_id: String,
    client_secret: String,
    client: Client,
    cached_token: Arc<tokio::sync::Mutex<Option<CachedToken>>>,
}

struct CachedToken {
    value: String,
    valid_until: SystemTime,
}

impl ManagementClient {
    pub fn new(
        management_base_url: &str,
        idp_base_url: &str,
        client_id: &str,
        client_secret: &str,
    ) -> Result<Self, String> {
        let management_base_url = Url::parse(management_base_url)
            .map_err(|e| format!("invalid management base URL: {}", e))?;
        let idp_base_url =
            Url::parse(idp_base_url).map_err(|e| format!("invalid IdP base URL: {}", e))?;

        let client = Client::builder()
            .redirect(Policy::none())
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| format!("failed to build HTTP client: {}", e))?;

        Ok(Self {
            management_base_url,
            idp_base_url,
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            client,
            cached_token: Arc::new(tokio::sync::Mutex::new(None)),
        })
    }

    async fn service_access_token(&self) -> Result<String, String> {
        let mut guard = self.cached_token.lock().await;
        if let Some(token) = &*guard {
            if SystemTime::now() < token.valid_until {
                return Ok(token.value.clone());
            }
        }

        // Request new service token from IdP
        let token_url = self
            .idp_base_url
            .join("/token")
            .map_err(|e| format!("invalid token URL: {}", e))?;

        let params = [
            ("grant_type", "client_credentials"),
            ("client_id", &self.client_id),
            ("client_secret", &self.client_secret),
            ("scope", "management.sync-selection.read"),
        ];

        let response = self
            .client
            .post(token_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("token request failed: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            return Err(format!("token request failed with status: {}", status));
        }

        let token_response: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("invalid token response: {}", e))?;

        let access_token = token_response["access_token"]
            .as_str()
            .ok_or("no access token in response")?
            .to_string();

        let expires_in = token_response["expires_in"].as_u64().unwrap_or(3600);

        *guard = Some(CachedToken {
            value: access_token.clone(),
            valid_until: SystemTime::now() + Duration::from_secs(expires_in.saturating_sub(60)),
        });

        Ok(access_token)
    }

    /// Get the sync target device for a resource from a source device.
    pub async fn get_sync_target(
        &self,
        resource_id: Id,
        source_device_id: Id,
    ) -> Result<Option<SyncSelection>, String> {
        let token = self.service_access_token().await?;

        let url = self
            .management_base_url
            .join(&format!(
                "/management/resources/{}/sync-targets",
                resource_id
            ))
            .map_err(|e| format!("invalid URL: {}", e))?;

        let response = self
            .client
            .get(url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| format!("sync targets request failed: {}", e))?;

        let status = response.status();
        if status == 404 {
            return Ok(None);
        }
        if !status.is_success() {
            return Err(format!(
                "sync targets request failed with status: {}",
                status
            ));
        }

        let response_json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("invalid response: {}", e))?;

        let selections = response_json["selections"]
            .as_array()
            .ok_or("no selections array")?;

        for selection in selections {
            let device_id = Id::parse_str(selection["device_id"].as_str().unwrap_or(""))
                .map_err(|_| "invalid device_id")?;
            if device_id != source_device_id {
                return Ok(Some(SyncSelection {
                    device_id: selection["device_id"]
                        .as_str()
                        .unwrap_or("")
                        .parse()
                        .unwrap(),
                    resource_id: selection["resource_id"]
                        .as_str()
                        .unwrap_or("")
                        .parse()
                        .unwrap(),
                    application_id: selection["application_id"]
                        .as_str()
                        .unwrap_or("")
                        .parse()
                        .unwrap(),
                    owner_subject: selection["owner_subject"]
                        .as_str()
                        .unwrap_or("")
                        .to_string(),
                    selected_at: selection["selected_at"].as_i64().unwrap_or(0),
                }));
            }
        }

        Ok(None)
    }

    /// Check if a device has a resource selected for WebSocket sync.
    pub async fn has_selection(&self, device_id: Id, resource_id: Id) -> Result<bool, String> {
        let token = self.service_access_token().await?;

        let url = self
            .management_base_url
            .join(&format!(
                "/management/devices/{}/sync-selections",
                device_id
            ))
            .map_err(|e| format!("invalid URL: {}", e))?;

        let response = self
            .client
            .get(url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| format!("sync selections request failed: {}", e))?;

        let status = response.status();
        if status == 404 {
            return Ok(false);
        }
        if !status.is_success() {
            return Err(format!(
                "sync selections request failed with status: {}",
                status
            ));
        }

        let response_json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("invalid response: {}", e))?;

        let selections = response_json["selections"]
            .as_array()
            .ok_or("no selections array")?;

        for selection in selections {
            let sel_resource_id = Id::parse_str(selection["resource_id"].as_str().unwrap_or(""))
                .map_err(|_| "invalid resource_id")?;
            if sel_resource_id == resource_id {
                return Ok(true);
            }
        }

        Ok(false)
    }
}
