use std::{io, path::Path};

use idp_model::model::Id;
use iroh::{EndpointId, SecretKey};
use key::SecretStore;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EndpointSecret {
    service: String,
    entry: String,
    target: String,
}

impl EndpointSecret {
    pub(crate) fn fresh() -> Self {
        Self {
            service: "of-unified-endpoint".into(),
            entry: Id::now_v7().to_string(),
            target: "Unified endpoint".into(),
        }
    }

    fn provider(&self) -> io::Result<SecretStore> {
        if self.service != "of-unified-endpoint"
            || self.target != "Unified endpoint"
            || !self
                .entry
                .parse::<Id>()
                .is_ok_and(|id| !id.is_nil() && id.to_string() == self.entry)
        {
            return Err(io::Error::other(
                "invalid endpoint provider reference; exact recovery required",
            ));
        }
        SecretStore::new(&self.service, &self.target).map_err(io::Error::other)
    }

    pub(crate) fn create(&self, secret: &SecretKey, expected: EndpointId) -> io::Result<()> {
        if secret.public() != expected {
            return Err(io::Error::other(
                "endpoint creation conflicts with retained public identity",
            ));
        }
        let provider = self.provider()?;
        if provider
            .load(&self.entry)
            .map_err(io::Error::other)?
            .is_some()
        {
            return Err(io::Error::other(
                "endpoint provider entry already exists; refusing overwrite",
            ));
        }
        provider
            .store(&self.entry, &secret.to_bytes())
            .map_err(io::Error::other)?;
        if self.load_from(&provider, expected)?.to_bytes() != secret.to_bytes() {
            return Err(io::Error::other("endpoint provider readback mismatch"));
        }
        Ok(())
    }

    pub(crate) fn load(&self, expected: EndpointId) -> io::Result<SecretKey> {
        self.load_from(&self.provider()?, expected)
    }

    fn load_from(&self, provider: &SecretStore, expected: EndpointId) -> io::Result<SecretKey> {
        let bytes = provider
            .load(&self.entry)
            .map_err(io::Error::other)?
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "attempted endpoint secret is missing; exact recovery required",
                )
            })?;
        let bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|_| io::Error::other("malformed endpoint provider material"))?;
        let secret = SecretKey::from_bytes(&bytes);
        if secret.public() != expected {
            return Err(io::Error::other(
                "endpoint provider public identity mismatch",
            ));
        }
        Ok(secret)
    }

    pub(crate) fn delete(&self) -> io::Result<()> {
        // Absence is idempotent only for explicit reset, never for ordinary startup.
        let provider = self.provider()?;
        if provider
            .load(&self.entry)
            .map_err(io::Error::other)?
            .is_some()
        {
            provider.delete(&self.entry).map_err(io::Error::other)?;
        }
        if provider
            .load(&self.entry)
            .map_err(io::Error::other)?
            .is_some()
        {
            return Err(io::Error::other(
                "endpoint provider deletion readback failed",
            ));
        }
        Ok(())
    }
}

pub(crate) fn retained(data_dir: &Path) -> io::Result<(EndpointSecret, EndpointId)> {
    let directory = data_dir.with_extension("provisioning");
    idp_server::local_provisioning::require_directory(&directory)?;
    idp_server::local_provisioning::require_absent(&directory.join("intent.json"))?;
    let explicit = data_dir.with_extension("host");
    let root = if crate::bootstrap_config::present(&explicit)? {
        explicit
    } else {
        data_dir
            .parent()
            .ok_or_else(|| io::Error::other("missing host root"))?
            .to_path_buf()
    };
    idp_server::local_provisioning::require_absent(&directory.join("join.json"))?;
    let store = crate::host_store::HostStore::open(&root)?;
    match (
        store.get::<serde_json::Value>("create")?.is_some(),
        store.get::<serde_json::Value>("join")?.is_some(),
    ) {
        (true, false) => crate::local_provisioning::endpoint_reference(&store),
        (false, true) => crate::local_join::endpoint_reference(&store),
        _ => Err(io::Error::other(
            "missing or conflicting endpoint owner metadata; exact recovery required",
        )),
    }
}
