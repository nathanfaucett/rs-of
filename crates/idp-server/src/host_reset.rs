use std::{io, path::Path};

use serde_json::{Value, json};

use crate::{
    BootstrapConfig, BootstrapPurpose, CredentialBootstrap, LocalJoin, LocalProvisioner,
    bootstrap_config::present,
    local_provisioning::{read_private, require_absent, require_file},
};

impl<H: crate::bootstrap_host::BootstrapHost> BootstrapConfig<H> {
    /// Nonsecret, exact protected-key references for an explicit stopped native reset.
    /// Missing/corrupt owner journals are not evidence that key production never started.
    pub fn local_reset_references(&self) -> io::Result<Value> {
        let mut config = self.config();
        if present(&H::settings(&config).credential_bootstrap_directory())? {
            config = CredentialBootstrap::<H>::open(config)?.load()?;
        }
        let settings = H::settings(&config);
        let provisioning = settings.local_provisioning_directory();
        let keys = if present(&provisioning)? {
            match self.purpose() {
                BootstrapPurpose::Create => {
                    LocalProvisioner::<H>::open(config)?.reset_references()?
                }
                BootstrapPurpose::Join => LocalJoin::<H>::open(config)?.reset_references()?,
                BootstrapPurpose::AwaitingChoice => {
                    return Err(io::Error::other("neutral host has unexpected owner state"));
                }
            }
        } else {
            for path in [
                Path::new(&settings.idp.data_dir),
                Path::new(&settings.management.data_dir),
                Path::new(&settings.storage.data_dir),
                &settings.endpoint_data_dir,
            ] {
                require_absent(path)?;
            }
            if self.purpose() == BootstrapPurpose::Join
                && read_private(&self.directory().join("join-owner.attempted"))? != b"unattempted\n"
            {
                return Err(io::Error::other(
                    "lost attempted Join owner journal; explicit recovery required",
                ));
            }
            Value::Null
        };
        Ok(json!({
            "version": 1,
            "root": self.directory(),
            "issuer": H::settings(&self.config()).idp.oauth2.issuer,
            "keys": keys,
        }))
    }

    /// Delete only the exact captured inventory, through its owner and protected provider.
    /// The caller must durably retain references before this call and remove data only after success.
    pub async fn delete_local_setup_secrets(&self, references: &Value) -> io::Result<()> {
        if self.local_reset_references()? != *references {
            return Err(io::Error::other(
                "native reset references conflict with retained owner intent",
            ));
        }
        let keys = &references["keys"];
        if keys.is_null() {
            return Ok(());
        }
        let config = CredentialBootstrap::<H>::open(self.config())?.load()?;
        let settings = H::settings(&config);
        if keys["inventory_attempted"] == false {
            require_absent(Path::new(&settings.idp.data_dir))?;
            return Ok(());
        }
        require_file(&Path::new(&settings.idp.data_dir).join("idp.redb"), false)?;
        let owner = crate::OwnerProvisioner::open(&settings.idp).await?;
        owner
            .delete_local_secrets(
                serde_json::from_value(keys["member_id"].clone())?,
                serde_json::from_value(keys["signer_key_id"].clone())?,
                serde_json::from_value(keys["owner_id"].clone())?,
                &serde_json::from_value::<Vec<idp_model::model::Id>>(keys["clients"].clone())?,
            )
            .await
    }
}
