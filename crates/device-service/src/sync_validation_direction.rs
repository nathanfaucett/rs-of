/// Deprecated placeholder for sync validation direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SyncValidationDirection {
    Incoming,
    Outgoing,
}
