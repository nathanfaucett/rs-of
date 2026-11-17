#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::DeserializeOwned};

pub fn serialize<T, S>(value: &[T], serializer: S) -> Result<S::Ok, S::Error>
where
    T: Serialize,
    S: Serializer,
{
    let s = value
        .iter()
        .map(|item| serde_json::to_string(item).map_err(serde::ser::Error::custom))
        .collect::<Result<Vec<String>, _>>()?
        .join(" ");
    serializer.serialize_str(&s)
}

pub fn deserialize<'de, T, D>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    T: DeserializeOwned,
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let items: Result<Vec<T>, _> = s
        .split_whitespace()
        .map(|item| serde_json::from_str(item).map_err(serde::de::Error::custom))
        .collect();
    items
}
