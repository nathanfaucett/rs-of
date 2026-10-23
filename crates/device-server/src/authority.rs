use std::sync::Arc;

use db::Database;
use device_service::{
    DeviceEndpointIdentity, HostedControlPlane,
    replica::{DbDeviceRepo, DbSelectionPolicyRepo},
};
use model::contract::SelectedResource;

#[derive(Clone)]
pub struct DeviceAuthority {
    control: Arc<HostedControlPlane>,
    policies: Arc<DbSelectionPolicyRepo>,
}

impl DeviceAuthority {
    pub fn new(devices: Arc<Database>, control: Arc<HostedControlPlane>) -> Self {
        Self {
            control: Arc::new(
                control
                    .as_ref()
                    .clone()
                    .with_local_devices(Arc::new(DbDeviceRepo::new(Arc::clone(&devices)))),
            ),
            policies: Arc::new(DbSelectionPolicyRepo::new(devices)),
        }
    }

    pub async fn endpoint_identity(
        &self,
        endpoint: &str,
    ) -> Result<DeviceEndpointIdentity, String> {
        self.control.get_storage_endpoint_identity(endpoint).await
    }

    pub async fn replication_policy(
        &self,
        identity: &DeviceEndpointIdentity,
    ) -> Result<(bool, Vec<SelectedResource>), String> {
        let policy = crate::runtime::replication_policy_from_repo(&self.policies, identity).await?;
        if self.endpoint_identity(&identity.endpoint_id).await? != *identity {
            return Err("Device binding changed during policy lookup".into());
        }
        Ok(policy)
    }

    pub async fn reconcile_owner(&self, identity: &DeviceEndpointIdentity) -> Result<bool, String> {
        self.policies
            .reconcile_owner_prevalidated(identity.device_id, &identity.owner_subject)
            .await
            .map_err(|error| error.to_string())
    }

    pub async fn pending_cleanup_owner(
        &self,
        identity: &DeviceEndpointIdentity,
    ) -> Result<String, String> {
        self.policies
            .pending_cleanup_owner(identity.device_id)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "missing local transfer cleanup intent".into())
    }

    pub async fn complete_owner_cleanup(
        &self,
        identity: &DeviceEndpointIdentity,
        old_owner: &str,
    ) -> Result<bool, String> {
        if self.endpoint_identity(&identity.endpoint_id).await? != *identity {
            return Err("Device owner changed during local cleanup".into());
        }
        self.policies
            .complete_owner_cleanup_prevalidated(
                identity.device_id,
                old_owner,
                &identity.owner_subject,
            )
            .await
            .map_err(|error| error.to_string())
    }
}
