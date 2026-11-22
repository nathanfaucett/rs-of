use std::io;

use ofdb_sql::Client;

use crate::AppConfig;

pub async fn device_client(config: &AppConfig) -> io::Result<Client> {
    if let Some(remote) = &config.device_database {
        if std::path::Path::new(&config.data_dir)
            .join("devices.redb")
            .try_exists()?
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "remote Device consumer cannot also own devices.redb",
            ));
        }
        let client = remote.connect().await?;
        device_service::ofdb::validate_device(&client)
            .await
            .map_err(io::Error::other)?;
        Ok(client)
    } else {
        Ok(crate::provisioning::open_device_database(config)
            .await?
            .client())
    }
}

pub async fn identity_client(config: &AppConfig) -> io::Result<Client> {
    if let Some(remote) = &config.identity_database {
        if config.identity_database_path.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "identity_database and identity_database_path are mutually exclusive",
            ));
        }
        let client = remote.connect().await?;
        db::ofdb::validators::validate_identity(&client)
            .await
            .map_err(io::Error::other)?;
        Ok(client)
    } else {
        let owner = crate::provisioning::open_identity_database(config)?;
        db::ofdb::validators::validate_identity(&owner.client())
            .await
            .map_err(io::Error::other)?;
        Ok(owner.client())
    }
}

#[derive(Clone)]
pub(crate) struct DeviceStore {
    pub client: Client,
}

impl DeviceStore {
    pub async fn open(config: &AppConfig) -> io::Result<Self> {
        if config.device_database.is_some() {
            let client = device_client(config).await?;
            std::fs::create_dir_all(&config.data_dir)?;
            Ok(Self { client })
        } else {
            let owner = crate::provisioning::open_device_database(config).await?;
            Ok(Self {
                client: owner.client(),
            })
        }
    }
}
