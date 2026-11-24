#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use of_model::idp::contract::client_registration::ClientRegistration;

use of_model::idp::model::client::Client;

use of_model::id::Id;

use super::error::RepoResult;

pub trait ClientRepo {
    fn find_client_by_id(&self, id: Id) -> impl Future<Output = RepoResult<Option<Client>>>;

    fn find_client_by_client_id(
        &self,
        client_id: &str,
    ) -> impl Future<Output = RepoResult<Option<Client>>>;

    fn find_client_registration(
        &self,
        client_id: &str,
    ) -> impl Future<Output = RepoResult<Option<Client>>>;

    fn list_clients(
        &self,
        offset: u32,
        limit: u32,
    ) -> impl Future<Output = RepoResult<Vec<Client>>>;

    fn create_client(&self, client: ClientRegistration)
    -> impl Future<Output = RepoResult<Client>>;

    fn create_client_in_existing_application(
        &self,
        client: ClientRegistration,
    ) -> impl Future<Output = RepoResult<Client>>;

    fn update_client(
        &self,
        client: Client,
        new_secret_hash: Option<String>,
    ) -> impl Future<Output = RepoResult<Client>>;

    fn delete_client_by_client_id(&self, client_id: &str) -> impl Future<Output = RepoResult<()>>;
}
