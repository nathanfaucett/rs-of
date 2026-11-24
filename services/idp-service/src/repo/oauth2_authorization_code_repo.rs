#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use chrono::{DateTime, Utc};
use of_model::id::Id;
use of_model::idp::contract::code_challenge_method::CodeChallengeMethod;
use of_model::idp::model::oauth2_authorization_code::OAuth2AuthorizationCode;

use super::error::RepoResult;

pub trait OAuth2AuthorizationCodeRepo {
    fn create_authorization_code(
        &self,
        client_id: String,
        key_id: Id,
        redirect_uri: String,
        scopes: Vec<String>,
        resource: Option<String>,
        code_challenge: Option<String>,
        code_challenge_method: Option<CodeChallengeMethod>,
        nonce: Option<String>,
        expires_at: DateTime<Utc>,
    ) -> impl Future<Output = RepoResult<OAuth2AuthorizationCode>>;

    fn find_authorization_code_by_code(
        &self,
        code: &str,
    ) -> impl Future<Output = RepoResult<Option<OAuth2AuthorizationCode>>>;

    fn consume_authorization_code(
        &self,
        id: Id,
        consumed_at: DateTime<Utc>,
    ) -> impl Future<Output = RepoResult<()>>;
}
