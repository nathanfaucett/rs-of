use std::{
    fs, io,
    path::{Component, Path, PathBuf},
};

use device_service::DeviceIdentity;
use device_service::device_identity_from_server;
use iroh::{EndpointId, SecretKey, endpoint::presets};
use iroh_chain::{EndpointIdStore, Server};

use crate::endpoint_secret::{EndpointSecret, retained};

pub struct UnifiedEndpoint {
    server: Server,
    allowed_peers: EndpointIdStore,
    secret_key: SecretKey,
}

impl UnifiedEndpoint {
    /// Existing-only open from the exact host-owned reference and public identity.
    pub async fn open(data_dir: &Path, idp_config: &idp_server::AppConfig) -> io::Result<Self> {
        let data_dir = managed_directory(data_dir)?;
        reject_legacy_layout(&data_dir)?;
        let (reference, expected) = retained(&data_dir)?;
        let secret_key = reference.load(expected)?;
        let allowed_peers = EndpointIdStore::default();
        allowed_peers.replace([secret_key.public()]);
        #[cfg(not(test))]
        let preset = presets::N0;
        #[cfg(test)]
        let preset = TestPreset;
        let server = idp_server::bind_node_join_endpoint(
            idp_config,
            preset,
            secret_key.clone(),
            allowed_peers.clone(),
        )
        .await?;

        #[cfg(test)]
        if let Some(lookup) = TEST_LOOKUP.get() {
            lookup.add_endpoint_info(server.endpoint().addr());
        }
        Ok(Self {
            server,
            allowed_peers,
            secret_key,
        })
    }

    /// The owner must commit the exact reference, public ID and attempt before calling.
    pub(crate) async fn create_prepared(
        data_dir: &Path,
        idp_config: &idp_server::AppConfig,
        reference: &EndpointSecret,
        secret: SecretKey,
    ) -> io::Result<Self> {
        idp_server::local_provisioning::require_unix_bootstrap()?;
        reject_symlink_ancestry(data_dir)?;
        let (retained_reference, expected) = retained(data_dir)?;
        if retained_reference != *reference || expected != secret.public() {
            return Err(io::Error::other(
                "endpoint creation lacks exact durable attempt evidence",
            ));
        }
        idp_server::local_provisioning::create_private_directory(data_dir, false)?;
        sync_directory_ancestry(data_dir)?;
        reference.create(&secret, expected)?;
        Self::open(data_dir, idp_config).await
    }

    pub fn server(&self) -> &Server {
        &self.server
    }

    pub fn identity(&self) -> io::Result<DeviceIdentity> {
        device_identity_from_server(&self.server, self.secret_key.clone())
    }

    pub fn endpoint_id(&self) -> EndpointId {
        self.server.endpoint().id()
    }

    pub fn refresh_admission(&self, approved_peers: impl IntoIterator<Item = EndpointId>) {
        let mut peers = approved_peers.into_iter().collect::<Vec<_>>();
        peers.push(self.endpoint_id());
        self.allowed_peers.replace(peers);
    }

    pub async fn refresh_approved_peers(
        &self,
        management: &device_service::HostedControlPlane,
    ) -> io::Result<()> {
        let peers = management
            .approved_device_endpoints()
            .await
            .map_err(io::Error::other)?
            .into_iter()
            .map(|endpoint| endpoint.parse::<EndpointId>().map_err(io::Error::other))
            .collect::<io::Result<Vec<_>>>()?;
        self.refresh_admission(peers);
        Ok(())
    }

    pub fn persisted_endpoint_id(data_dir: &Path) -> io::Result<EndpointId> {
        let directory = managed_directory(data_dir)?;
        reject_legacy_layout(&directory)?;
        let (reference, expected) = retained(&directory)?;
        Ok(reference.load(expected)?.public())
    }

    pub async fn close(&self) {
        self.server.endpoint().close().await;
    }
}

#[cfg(test)]
static TEST_LOOKUP: std::sync::OnceLock<iroh::address_lookup::MemoryLookup> =
    std::sync::OnceLock::new();

#[cfg(test)]
pub(crate) fn use_isolated_address_lookup() {
    TEST_LOOKUP.get_or_init(iroh::address_lookup::MemoryLookup::new);
}

#[cfg(test)]
pub(crate) struct TestPreset;

#[cfg(test)]
impl presets::Preset for TestPreset {
    fn apply(self, builder: iroh::endpoint::Builder) -> iroh::endpoint::Builder {
        if let Some(lookup) = TEST_LOOKUP.get() {
            presets::Preset::apply(presets::Minimal, builder)
                .address_lookup(lookup.clone())
                .relay_mode(iroh::RelayMode::Disabled)
        } else {
            presets::Preset::apply(presets::N0, builder)
        }
    }
}

fn reject_legacy_layout(directory: &Path) -> io::Result<()> {
    if fs::read_dir(directory)?.next().is_some() {
        return Err(io::Error::other(
            "unsupported endpoint file layout; preserve state and explicitly reset or recover",
        ));
    }
    Ok(())
}

fn reject_symlink_ancestry(directory: &Path) -> io::Result<()> {
    let absolute = std::env::current_dir()?.join(directory);
    let mut prefix = PathBuf::new();
    for component in absolute.components() {
        if component == Component::ParentDir {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unified endpoint path must not contain parent traversal",
            ));
        }
        prefix.push(component);
        match fs::symlink_metadata(&prefix) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unified endpoint ancestry must contain only nonsymlink directories",
                ));
            }
            Ok(_) => (),
            Err(error) if error.kind() == io::ErrorKind::NotFound => (),
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn managed_directory(directory: &Path) -> io::Result<PathBuf> {
    reject_symlink_ancestry(directory)?;
    fs::canonicalize(directory)
}

fn sync_directory_ancestry(directory: &Path) -> io::Result<()> {
    let physical = managed_directory(directory)?;
    for ancestor in physical.ancestors() {
        fs::File::open(ancestor)?.sync_all()?;
    }
    Ok(())
}

pub(crate) async fn discover_with_cancellation(
    peer: &str,
    cancellation: &tokio_util::sync::CancellationToken,
) -> io::Result<String> {
    #[cfg(test)]
    let preset = crate::endpoint::TestPreset;
    #[cfg(not(test))]
    let preset = iroh::endpoint::presets::N0;
    idp_server::issuer_discovery::discover_with_preset(peer, cancellation, preset).await
}

pub async fn discover_installation_issuer(peer: &str) -> io::Result<String> {
    discover_with_cancellation(peer, &tokio_util::sync::CancellationToken::new()).await
}
