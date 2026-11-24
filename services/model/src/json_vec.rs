#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{DeserializeOwned, Error as DeError},
    ser::Error as SerError,
};

/// Serializes a slice as a JSON string.
///
/// # Errors
/// Returns an error if JSON serialization or output serialization fails.
pub fn serialize<T, S>(value: &[T], serializer: S) -> Result<S::Ok, S::Error>
where
    T: Serialize,
    S: Serializer,
{
    serde_json::to_string(value)
        .map_err(SerError::custom)?
        .serialize(serializer)
}

/// Deserializes a slice from a JSON string.
///
/// # Errors
/// Returns an error if the input string or its JSON contents are invalid.
pub fn deserialize<'de, T, D>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    T: DeserializeOwned,
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    if value.is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(&value).map_err(DeError::custom)
}
