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
    #[allow(
        clippy::suspicious_operation_groupings,
        reason = "TokenPrincipalBinding::key_id intentionally matches Key::id"
    )]
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

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "std"))]
    use alloc::string::String;

    use chrono::DateTime;

    use super::*;

    fn root() -> Key {
        Key {
            id: Id::nil(),
            parent_id: None,
            entity_type: EntityType::User,
            entity_id: Id::nil(),
            derivation_path: String::new(),
            derivation_index: 0,
            name: String::new(),
            hardened: true,
            public_jwk: None,
            revoked_at: None,
            expires_at: None,
            created_at: DateTime::UNIX_EPOCH,
            updated_at: DateTime::UNIX_EPOCH,
        }
    }

    #[test]
    fn binding_key_id_matches_the_root_key_id() {
        let root = root();
        let binding = TokenPrincipalBinding {
            entity_type: root.entity_type,
            entity_id: root.entity_id,
            key_id: root.id,
        };
        assert!(binding.matches_active_root(&root, 0));

        let wrong_key = TokenPrincipalBinding {
            key_id: Id::from_u128(1),
            ..binding
        };
        assert!(!wrong_key.matches_active_root(&root, 0));
    }
}
