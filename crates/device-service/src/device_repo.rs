use alloc::{string::String, vec::Vec};

use crate::{Device, TrustedDevice};

use crate::DeviceResult;

pub trait DeviceRepo {
    fn create(
        &self,
        owner_subject: String,
        name: String,
        public_key: String,
        address: String,
        enrollment_code_hash: Vec<u8>,
        enrollment_expires_at: i64,
    ) -> impl Future<Output = DeviceResult<Device>>;

    fn create_pairing(
        &self,
        name: String,
        public_key: String,
        address: String,
        accepting_public_key: String,
    ) -> impl Future<Output = DeviceResult<Device>>;

    fn pending_pairing(
        &self,
        device_id: idp_model::model::Id,
    ) -> impl Future<Output = DeviceResult<Option<(Device, String)>>>;

    fn approve_pairing(
        &self,
        device_id: idp_model::model::Id,
    ) -> impl Future<Output = DeviceResult<Option<Device>>>;

    fn has_any(&self) -> impl Future<Output = DeviceResult<bool>>;

    fn list(&self) -> impl Future<Output = DeviceResult<Vec<Device>>>;

    fn find_approved_by_public_key(
        &self,
        public_key: &str,
    ) -> impl Future<Output = DeviceResult<Option<Device>>>;

    fn list_owned(&self, owner_subject: &str) -> impl Future<Output = DeviceResult<Vec<Device>>>;

    fn list_approved(
        &self,
        owner_subject: &str,
    ) -> impl Future<Output = DeviceResult<Vec<TrustedDevice>>>;

    fn are_approved(
        &self,
        local_public_key: &str,
        remote_public_key: &str,
    ) -> impl Future<Output = DeviceResult<bool>>;

    fn approve(
        &self,
        device_id: idp_model::model::Id,
        enrollment_code_hash: &[u8],
    ) -> impl Future<Output = DeviceResult<Option<Device>>>;

    fn rename(
        &self,
        owner_subject: &str,
        device_id: idp_model::model::Id,
        name: String,
    ) -> impl Future<Output = DeviceResult<Option<Device>>>;

    fn revoke(
        &self,
        owner_subject: &str,
        device_id: idp_model::model::Id,
        protected_public_key: &str,
    ) -> impl Future<Output = DeviceResult<bool>>;

    fn revoke_self(&self, public_key: &str) -> impl Future<Output = DeviceResult<bool>>;
}
