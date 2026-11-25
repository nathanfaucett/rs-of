use core::fmt;

use std::sync::Arc;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use iroh::{EndpointId, Signature};
use of_model::idp::contract::{
    error_code::ErrorCode,
    error_response::{ErrorResponse, ErrorResponseResult},
};

use of_model::device::device::Device;
use of_model::id::Id;

use of_model::device::contract::device_enrollment::DeviceEnrollment;

use of_model::device::contract::device_enrollment_request::DeviceEnrollmentRequest;

use of_model::device::contract::device_info::DeviceInfo;

use of_model::device::contract::device_pairing::DevicePairingApprovalRequest;

use of_model::device::contract::device_pairing::DevicePairingRequest;

use crate::device_repo::DeviceRepo;

use of_model::device::contract::update_device_request::UpdateDeviceRequest;

use of_model::device::contract::device_pairing::device_pairing_approval_payload;

pub struct DeviceEnrollmentService<R> {
    repo: Arc<R>,
}

impl<R> fmt::Debug for DeviceEnrollmentService<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceEnrollmentService")
            .finish_non_exhaustive()
    }
}

#[allow(
    clippy::future_not_send,
    reason = "DeviceRepo does not require Send futures; preserve the public API"
)]
impl<R> DeviceEnrollmentService<R>
where
    R: DeviceRepo,
{
    pub const fn new(repo: Arc<R>) -> Self {
        Self { repo }
    }

    /// # Errors
    ///
    /// Returns an error for invalid enrollment input, denied enrollment, or a repository failure.
    pub async fn enroll(
        &self,
        owner_subject: String,
        request: DeviceEnrollmentRequest,
    ) -> ErrorResponseResult<DeviceEnrollment> {
        if owner_subject.trim().is_empty() {
            return Err(ErrorResponse::new(ErrorCode::AccessDenied));
        }
        validate_enrollment(&request)?;
        if self.repo.has_any().await.map_err(ErrorResponse::from)? {
            return Err(ErrorResponse::new(ErrorCode::AccessDenied));
        }
        let device = self
            .repo
            .create(
                owner_subject,
                request.name,
                request.public_key,
                request.address,
                Vec::new(),
                0,
            )
            .await
            .map_err(ErrorResponse::from)?;
        Ok(DeviceEnrollment {
            id: device.id,
            state: device.state,
            approval_code: None,
        })
    }

    /// # Errors
    ///
    /// Returns an error for invalid pairing input or a repository failure.
    pub async fn request_pairing(
        &self,
        request: DevicePairingRequest,
    ) -> ErrorResponseResult<DeviceEnrollment> {
        validate_pairing(&request)?;
        let device = self
            .repo
            .create_pairing(
                request.name,
                request.public_key,
                request.address,
                request.accepting_public_key,
            )
            .await
            .map_err(ErrorResponse::from)?;
        Ok(DeviceEnrollment {
            id: device.id,
            state: device.state,
            approval_code: None,
        })
    }

    /// Request or replay one exact pending pairing for the authenticated requester.
    ///
    /// # Errors
    /// Returns an error for invalid input, a different requester or request, or an unavailable pairing.
    pub async fn request_pairing_once(
        &self,
        requester: String,
        request: DevicePairingRequest,
    ) -> ErrorResponseResult<DeviceEnrollment> {
        validate_pairing(&request)?;
        let device = self
            .repo
            .request_pairing_once(requester, request)
            .await
            .map_err(|_| ErrorResponse::new(ErrorCode::TemporarilyUnavailable))?
            .ok_or_else(|| ErrorResponse::new(ErrorCode::InvalidRequest))?;
        Ok(DeviceEnrollment {
            id: device.id,
            state: device.state,
            approval_code: None,
        })
    }

    /// # Errors
    ///
    /// Returns an error when pairing is not authorized, the signature is invalid, or the repository fails.
    pub async fn approve_pairing(
        &self,
        device_id: Id,
        request: DevicePairingApprovalRequest,
    ) -> ErrorResponseResult<DeviceInfo> {
        let (device, accepting_public_key) = self
            .repo
            .pending_pairing(device_id)
            .await
            .map_err(ErrorResponse::from)?
            .ok_or_else(|| ErrorResponse::new(ErrorCode::AccessDenied))?;
        if self
            .repo
            .find_approved_by_public_key(&accepting_public_key)
            .await
            .map_err(ErrorResponse::from)?
            .is_none_or(|accepting| accepting.owner_subject != device.owner_subject)
        {
            return Err(ErrorResponse::new(ErrorCode::AccessDenied));
        }
        verify_signature(
            &device.public_key,
            &device_pairing_approval_payload(
                device.id,
                &device.name,
                &device.public_key,
                &device.address,
            ),
            &request.signature,
        )?;
        self.repo
            .approve_pairing(device_id)
            .await
            .map_err(ErrorResponse::from)?
            .map(Into::into)
            .ok_or_else(|| ErrorResponse::new(ErrorCode::AccessDenied))
    }

    /// # Errors
    ///
    /// Returns an error when pairing is unavailable or the repository fails.
    pub async fn pairing_approval_payload(&self, device_id: Id) -> ErrorResponseResult<String> {
        let (device, _) = self
            .repo
            .pending_pairing(device_id)
            .await
            .map_err(ErrorResponse::from)?
            .ok_or_else(|| ErrorResponse::new(ErrorCode::AccessDenied))?;
        Ok(device_pairing_approval_payload(
            device.id,
            &device.name,
            &device.public_key,
            &device.address,
        ))
    }

    /// # Errors
    ///
    /// Returns an error when the repository fails.
    pub async fn list(&self, owner_subject: &str) -> ErrorResponseResult<Vec<DeviceInfo>> {
        self.repo
            .list_owned(owner_subject)
            .await
            .map(|devices| devices.into_iter().map(Device::into).collect())
            .map_err(ErrorResponse::from)
    }

    /// # Errors
    ///
    /// Returns an error when the repository fails.
    pub async fn list_pending(&self) -> ErrorResponseResult<Vec<DeviceEnrollment>> {
        self.repo
            .list_pending()
            .await
            .map(|devices| {
                devices
                    .into_iter()
                    .map(|d| DeviceEnrollment {
                        id: d.id,
                        state: d.state,
                        approval_code: None,
                    })
                    .collect()
            })
            .map_err(ErrorResponse::from)
    }

    /// # Errors
    ///
    /// Returns an error for an invalid name, a missing Device, or a repository failure.
    pub async fn rename(
        &self,
        owner_subject: &str,
        device_id: Id,
        request: UpdateDeviceRequest,
    ) -> ErrorResponseResult<DeviceInfo> {
        validate_name(&request.name)?;
        self.repo
            .rename(owner_subject, device_id, request.name)
            .await
            .map_err(ErrorResponse::from)?
            .map(Into::into)
            .ok_or_else(|| ErrorResponse::new(ErrorCode::NotFound))
    }

    /// # Errors
    ///
    /// Returns an error when the Device cannot be revoked or the repository fails.
    pub async fn revoke(
        &self,
        owner_subject: &str,
        device_id: Id,
        protected_public_key: &str,
    ) -> ErrorResponseResult<()> {
        self.repo
            .revoke(owner_subject, device_id, protected_public_key)
            .await
            .map_err(ErrorResponse::from)?
            .then_some(())
            .ok_or_else(|| ErrorResponse::new(ErrorCode::NotFound))
    }

    /// # Errors
    ///
    /// Returns an error when the Device is not found or the repository fails.
    pub async fn revoke_self(&self, public_key: &str) -> ErrorResponseResult<()> {
        self.repo
            .revoke_self(public_key)
            .await
            .map_err(ErrorResponse::from)?
            .then_some(())
            .ok_or_else(|| ErrorResponse::new(ErrorCode::NotFound))
    }

    /// # Errors
    ///
    /// Returns an error when the enrollment is not found or the repository fails.
    pub async fn approve_enrollment(&self, device_id: Id) -> ErrorResponseResult<DeviceEnrollment> {
        let device = self
            .repo
            .approve_by_admin(device_id)
            .await
            .map_err(ErrorResponse::from)?
            .ok_or_else(|| ErrorResponse::new(ErrorCode::NotFound))?;
        Ok(DeviceEnrollment {
            id: device.id,
            state: device.state,
            approval_code: None,
        })
    }
}

fn validate_pairing(request: &DevicePairingRequest) -> ErrorResponseResult<()> {
    validate_name(&request.name)?;
    validate_public_key(&request.public_key)?;
    validate_public_key(&request.accepting_public_key)?;
    valid_value(&request.address)
        .then_some(())
        .ok_or_else(|| ErrorResponse::new(ErrorCode::InvalidRequest))
}

fn validate_enrollment(request: &DeviceEnrollmentRequest) -> ErrorResponseResult<()> {
    validate_name(&request.name)?;
    validate_public_key(&request.public_key)?;
    valid_value(&request.address)
        .then_some(())
        .ok_or_else(|| ErrorResponse::new(ErrorCode::InvalidRequest))
}

fn validate_name(name: &str) -> ErrorResponseResult<()> {
    valid_value(name)
        .then_some(())
        .ok_or_else(|| ErrorResponse::new(ErrorCode::InvalidRequest))
}

fn validate_public_key(public_key: &str) -> ErrorResponseResult<()> {
    if !valid_value(public_key) {
        return Err(ErrorResponse::new(ErrorCode::InvalidRequest));
    }
    let endpoint_id = public_key
        .parse::<EndpointId>()
        .map_err(|_error| ErrorResponse::new(ErrorCode::InvalidRequest))?;
    (endpoint_id.to_string() == public_key)
        .then_some(())
        .ok_or_else(|| ErrorResponse::new(ErrorCode::InvalidRequest))
}

fn valid_value(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 512
}

fn verify_signature(public_key: &str, payload: &str, signature: &str) -> ErrorResponseResult<()> {
    let public_key = public_key
        .parse::<EndpointId>()
        .map_err(|_error| ErrorResponse::new(ErrorCode::AccessDenied))?;
    let signature = URL_SAFE_NO_PAD
        .decode(signature)
        .map_err(|_error| ErrorResponse::new(ErrorCode::AccessDenied))?;
    let signature = Signature::try_from(signature.as_slice())
        .map_err(|_error| ErrorResponse::new(ErrorCode::AccessDenied))?;
    public_key
        .verify(payload.as_bytes(), &signature)
        .map_err(|_error| ErrorResponse::new(ErrorCode::AccessDenied))
}

#[cfg(test)]
mod tests {
    use iroh::{EndpointId, SecretKey};

    use super::validate_public_key;

    #[test]
    fn public_keys_must_use_the_canonical_endpoint_id_encoding() {
        let canonical = SecretKey::generate().public().to_string();
        assert!(validate_public_key(&canonical).is_ok());
        assert!(validate_public_key("not-an-endpoint-id").is_err());

        let alternate_case = canonical.to_ascii_uppercase();
        if alternate_case != canonical && alternate_case.parse::<EndpointId>().is_ok() {
            assert!(validate_public_key(&alternate_case).is_err());
        }
    }
}
