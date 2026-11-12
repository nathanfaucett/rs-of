use alloc::vec::Vec;

use idp_model::model::{Id, User, UserEmail, UserPassword, UserPhoneNumber};

use crate::repo::RepoResult;

pub trait UserRepo {
    fn find_user_by_id(&self, id: Id) -> impl Future<Output = RepoResult<Option<User>>>;

    fn list_users(&self, offset: u32, limit: u32) -> impl Future<Output = RepoResult<Vec<User>>>;

    fn find_user_by_username_or_email(
        &self,
        identifier: &str,
    ) -> impl Future<Output = RepoResult<Option<User>>>;

    fn find_user_emails_by_user_id(
        &self,
        user_id: Id,
    ) -> impl Future<Output = RepoResult<Vec<UserEmail>>>;
    fn find_user_phone_numbers_by_user_id(
        &self,
        user_id: Id,
    ) -> impl Future<Output = RepoResult<Vec<UserPhoneNumber>>>;

    fn find_user_password_by_user_id(
        &self,
        user_id: Id,
    ) -> impl Future<Output = RepoResult<Option<UserPassword>>>;

    fn create_user_with_password(
        &self,
        name: &str,
        password: &str,
    ) -> impl Future<Output = RepoResult<User>>;

    fn create_user_with_password_and_ids(
        &self,
        user_id: Id,
        credential_id: Id,
        name: &str,
        password: &str,
    ) -> impl Future<Output = RepoResult<User>>;

    fn update_user(&self, user: User) -> impl Future<Output = RepoResult<User>>;

    fn upsert_primary_user_email(
        &self,
        user_id: Id,
        email: &str,
        verified: bool,
    ) -> impl Future<Output = RepoResult<()>>;

    fn upsert_primary_user_phone_number(
        &self,
        user_id: Id,
        phone_number: &str,
        verified: bool,
    ) -> impl Future<Output = RepoResult<()>>;

    fn replace_user_password(
        &self,
        user_id: Id,
        password: &str,
    ) -> impl Future<Output = RepoResult<()>>;

    fn set_user_mfa_totp_secret(
        &self,
        user_id: Id,
        secret: Option<String>,
    ) -> impl Future<Output = RepoResult<()>>;

    fn set_user_mfa_webauthn_credentials(
        &self,
        user_id: Id,
        credentials: Option<String>,
    ) -> impl Future<Output = RepoResult<()>>;

    fn set_user_blocked(
        &self,
        user_id: Id,
        blocked: bool,
        blocked_by: Option<Id>,
    ) -> impl Future<Output = RepoResult<()>>;

    fn delete_user_by_id(&self, user_id: Id) -> impl Future<Output = RepoResult<()>>;
}
