use crate::{
    id::Id,
    idp::{contract::entity_type::EntityType, model::key::Key},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TokenPrincipalBinding {
    pub entity_type: EntityType,
    pub entity_id: Id,
    pub key_id: Id,
}

impl TokenPrincipalBinding {
    /// The caller must also resolve a live User/Client and its current active root.
    #[must_use]
    pub fn matches_active_root(&self, root: &Key, now: i64) -> bool {
        self.key_id == root.id
            && self.entity_type == root.entity_type
            && self.entity_id == root.entity_id
            && root.parent_id.is_none()
            && root.revoked_at.is_none()
            && root
                .expires_at
                .is_none_or(|expires| expires.timestamp() > now)
    }
}
