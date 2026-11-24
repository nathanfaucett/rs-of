#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use chrono::{DateTime, Utc};
use of_model::id::Id;
use of_model::idp::contract::entity_type::EntityType;
use of_model::idp::contract::jwk_public::JwkPublic;
use of_model::idp::model::key::Key;

use super::error::RepoResult;

pub trait KeyRepo {
    fn issuer_keys(
        &self,
        issuer: &str,
        now: i64,
    ) -> impl Future<Output = RepoResult<Vec<(JwkPublic, bool)>>>;

    fn rotate_issuer_key(
        &self,
        issuer: &str,
        key: &JwkPublic,
        previous: &str,
        expires: i64,
    ) -> impl Future<Output = RepoResult<()>>;

    fn set_public_jwk(&self, id: Id, jwk: JwkPublic) -> impl Future<Output = RepoResult<Key>>;

    fn list_active(&self) -> impl Future<Output = RepoResult<Vec<Key>>>;

    fn list_by_entity_type_and_id(
        &self,
        entity_type: EntityType,
        entity_id: Id,
    ) -> impl Future<Output = RepoResult<Vec<Key>>>;

    fn find_by_id(&self, id: Id) -> impl Future<Output = RepoResult<Option<Key>>>;

    fn find_by_entity_type_and_id(
        &self,
        entity_type: EntityType,
        entity_id: Id,
    ) -> impl Future<Output = RepoResult<Option<Key>>>;

    fn find_active_entity_root_key(
        &self,
        entity_type: EntityType,
        entity_id: Id,
    ) -> impl Future<Output = RepoResult<Option<Key>>>;

    fn delete_by_entity_type_and_id(
        &self,
        entity_type: EntityType,
        entity_id: Id,
    ) -> impl Future<Output = RepoResult<()>>;

    fn create_key(
        &self,
        parent_id: Option<Id>,
        entity_type: EntityType,
        entity_id: Id,
        hardened: bool,
        name: String,
        expires_at: Option<DateTime<Utc>>,
    ) -> impl Future<Output = RepoResult<Key>>;
}
