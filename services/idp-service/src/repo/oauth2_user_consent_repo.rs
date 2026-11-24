#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use of_model::id::Id;

use of_model::idp::model::oauth2_user_consent::OAuth2UserConsent;

use super::error::RepoResult;

pub trait OAuth2UserConsentRepo {
    fn upsert_user_consent(
        &self,
        user_id: Id,
        client_id: &str,
        redirect_uri: &str,
        scope: &str,
    ) -> impl Future<Output = RepoResult<OAuth2UserConsent>>;

    fn find_user_consent(
        &self,
        user_id: Id,
        client_id: &str,
        redirect_uri: &str,
        scope: &str,
    ) -> impl Future<Output = RepoResult<Option<OAuth2UserConsent>>>;

    fn list_user_consents(
        &self,
        user_id: Id,
        offset: u32,
        limit: u32,
    ) -> impl Future<Output = RepoResult<Vec<OAuth2UserConsent>>>;

    fn find_user_consent_by_id(
        &self,
        consent_id: Id,
    ) -> impl Future<Output = RepoResult<Option<OAuth2UserConsent>>>;

    fn delete_user_consent_by_id(&self, consent_id: Id) -> impl Future<Output = RepoResult<()>>;
}
