use serde::{Deserialize, Deserializer};

/// Deserializes an optional value, using its default when the input is null.
///
/// # Errors
/// Returns an error when the input cannot be deserialized as `T`.
pub fn none_to_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}
