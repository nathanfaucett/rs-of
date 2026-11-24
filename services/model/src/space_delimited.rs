#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{DeserializeOwned, Error as DeError},
    ser::Error as SerError,
};

/// Serializes items as space-delimited JSON values.
///
/// # Errors
/// Returns an error if an item cannot be serialized as JSON or to the output.
pub fn serialize<T, S>(value: &[T], serializer: S) -> Result<S::Ok, S::Error>
where
    T: Serialize,
    S: Serializer,
{
    let s = value
        .iter()
        .map(|item| serde_json::to_string(item).map_err(SerError::custom))
        .collect::<Result<Vec<String>, _>>()?
        .join(" ");
    serializer.serialize_str(&s)
}

/// Deserializes space-delimited JSON values.
///
/// # Errors
/// Returns an error if the input string or any JSON item is invalid.
pub fn deserialize<'de, T, D>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    T: DeserializeOwned,
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let items: Result<Vec<T>, _> = s
        .split_whitespace()
        .map(|item| serde_json::from_str(item).map_err(DeError::custom))
        .collect();
    items
}
