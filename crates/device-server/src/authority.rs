use std::sync::Arc;

use db::Database;
use device_service::replica::OwnerPolicyObservation;
use device_service::{
    DeviceEndpointIdentity, HostedControlPlane,
    replica::{DbDeviceRepo, DbSelectionPolicyRepo},
};
use idp_model::{
    contract::{ErrorCode, ErrorResponse},
    model::Id,
};
use model::contract::{OwnerCopyCleanup, SelectedResource, SelectedResourcesResponse};

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
                    .with_local_devices(Arc::new(DbDeviceRepo::new(Arc::clone(&devices).client()))),
            ),
            policies: Arc::new(DbSelectionPolicyRepo::new(devices.client())),
        }
    }

    pub(crate) fn from_parts(
        control: Arc<HostedControlPlane>,
        policies: Arc<DbSelectionPolicyRepo>,
    ) -> Self {
        Self { control, policies }
    }

    pub async fn selected_resources(
        &self,
        endpoint_id: &str,
    ) -> Result<SelectedResourcesResponse, ErrorResponse> {
        let identity = self
            .control
            .get_storage_endpoint_identity(endpoint_id)
            .await
            .map_err(|_| unavailable())?;
        let observation = self
            .policies
            .observe_owner_prevalidated(identity.device_id, &identity.owner_subject)
            .await
            .map_err(|_| unavailable())?;
        let (pending_cleanup, admin_allowed, resources) = match observation {
            OwnerPolicyObservation::PendingCleanup {
                cleanup_id,
                old_owner_subject,
            } => (
                Some(OwnerCopyCleanup {
                    cleanup_id: cleanup_id.to_string(),
                    device_id: identity.device_id.to_string(),
                    endpoint_id: identity.endpoint_id.clone(),
                    old_owner_subject,
                    new_owner_subject: identity.owner_subject.clone(),
                }),
                false,
                vec![],
            ),
            ready => {
                let (allowed, resources) =
                    crate::runtime::replication_policy_from_observation(ready)
                        .map_err(|_| unavailable())?;
                (None, allowed, resources)
            }
        };
        if self
            .control
            .get_storage_endpoint_identity(endpoint_id)
            .await
            .map_err(|_| unavailable())?
            != identity
        {
            return Err(denied());
        }
        Ok(SelectedResourcesResponse {
            device_id: identity.device_id.to_string(),
            endpoint_id: identity.endpoint_id,
            owner_subject: identity.owner_subject,
            pending_cleanup,
            admin_allowed,
            resources,
        })
    }

    pub async fn acknowledge_owner_cleanup(
        &self,
        request: &OwnerCopyCleanup,
    ) -> Result<(), ErrorResponse> {
        let identity = self
            .control
            .get_storage_endpoint_identity(&request.endpoint_id)
            .await
            .map_err(|_| unavailable())?;
        if identity.device_id.to_string() != request.device_id
            || identity.owner_subject != request.new_owner_subject
            || !request
                .old_owner_subject
                .parse::<Id>()
                .is_ok_and(|owner| !owner.is_nil())
            || !self
                .policies
                .complete_owner_cleanup_bound(
                    identity.device_id,
                    &request.old_owner_subject,
                    &request.new_owner_subject,
                    &request.endpoint_id,
                    request.cleanup_id.parse::<Id>().map_err(|_| denied())?,
                )
                .await
                .map_err(|_| unavailable())?
        {
            return Err(denied());
        }
        if self
            .control
            .get_storage_endpoint_identity(&request.endpoint_id)
            .await
            .map_err(|_| unavailable())?
            != identity
        {
            return Err(denied());
        }
        Ok(())
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
}

fn denied() -> ErrorResponse {
    ErrorResponse::new(ErrorCode::AccessDenied)
}

fn unavailable() -> ErrorResponse {
    ErrorResponse::new(ErrorCode::TemporarilyUnavailable)
}
