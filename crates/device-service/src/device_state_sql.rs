#[cfg(not(feature = "std"))]
use alloc::format;

use serde::{Deserialize, Deserializer, Serializer, de::Error};

use crate::DeviceState;

pub fn serialize<S>(value: &DeviceState, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_i64(*value as i64)
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<DeviceState, D::Error>
where
    D: Deserializer<'de>,
{
    match i64::deserialize(deserializer)? {
        0 => Ok(DeviceState::Pending),
        1 => Ok(DeviceState::Approved),
        2 => Ok(DeviceState::Revoked),
        value => Err(D::Error::custom(format!(
            "invalid DeviceState value: {value}"
        ))),
    }
}
