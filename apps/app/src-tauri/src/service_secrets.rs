use std::io;

use idp_model::model::Id;
use idp_service::repo::RawKeyringRepo;

const KEYRING_SERVICE: &str = "local.unified-service-credentials";

pub fn delete_secret(installation_id: Id, key_name: &str, client_id: Id) -> io::Result<()> {
    let keyring = RawKeyringRepo::new(KEYRING_SERVICE).map_err(io::Error::other)?;
    let user = installation_id.to_string();
    let client = client_id.to_string();
    if keyring
        .load(&user, &client, key_name)
        .map_err(io::Error::other)?
        .is_some()
    {
        keyring
            .delete(&user, &client, key_name)
            .map_err(io::Error::other)?;
        if keyring
            .load(&user, &client, key_name)
            .map_err(io::Error::other)?
            .is_some()
        {
            return Err(io::Error::other(
                "secure-store deletion was not durable; retry reset",
            ));
        }
    }
    Ok(())
}
