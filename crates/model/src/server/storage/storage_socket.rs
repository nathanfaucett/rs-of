use alloc::string::String;

#[derive(serde::Deserialize)]
pub struct StorageQuery {
    pub access_token: String,
    pub filesystem_id: String,
}
