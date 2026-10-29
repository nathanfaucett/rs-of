use std::{
    fs::File,
    io::{self},
    net::{Ipv4Addr, SocketAddr, TcpListener},
    path::{Path, PathBuf},
};

use device_service::issuer_discovery::validate_issuer;
use idp_model::model::Id;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use idp_server::local_provisioning::{
    absolute, create_private_directory, lock, read_private, require_absent,
    require_unix_bootstrap,
};

use crate::{CredentialBootstrap, LocalJoin, LocalProvisioner, UnifiedConfig};

#[derive(Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapPurpose {
    AwaitingChoice,
    Create,
    Join,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Host {
    version: u8,
    root: PathBuf,
    listen: SocketAddr,
    key_namespace: Id,
    purpose: BootstrapPurpose,
    issuer: String,
    trusted_peer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    setup_attempted: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    explicit: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    public_base: Option<String>,
}

/// Public host identity/settings only. Retain the guard through setup and serving.
pub struct BootstrapConfig {
    host: Host,
    _operation: File,
    explicit_config: Option<UnifiedConfig>,
    fresh: bool,
    store: crate::host_store::HostStore,
}
impl BootstrapConfig {
    pub fn default_directory() -> io::Result<PathBuf> {
        if let Some(path) = std::env::var_os("XDG_DATA_HOME").filter(|value| !value.is_empty()) {
            let path = PathBuf::from(path);
            if !path.is_absolute() {
                return Err(invalid("XDG_DATA_HOME must be absolute"));
            }
            return Ok(path.join("of/unified"));
        }
        let home = std::env::var_os("HOME")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| invalid("HOME is unavailable; supply --data-dir"))?;
        Ok(PathBuf::from(home).join(".local/share/of/unified"))
    }

    /// Authorizes only a positively fresh, absent Node root and retains its port before material production.
    pub fn create(
        root: &Path,
        purpose: BootstrapPurpose,
        peer_issuer: Option<(&str, &str)>,
    ) -> io::Result<Self> {
        Self::create_host(root, purpose, peer_issuer, None)
    }

    /// Creates a proxy-facing HTTP host with retained public URLs and listener settings.
    pub fn create_http(
        root: &Path,
        purpose: BootstrapPurpose,
        peer_issuer: Option<(&str, &str)>,
        listen: SocketAddr,
        public_base: String,
    ) -> io::Result<Self> {
        let url =
            reqwest::Url::parse(&public_base).map_err(|_| invalid("invalid public base URL"))?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(invalid("public base must be an HTTP(S) origin"));
        }
        crate::LocalTls::api_base(listen)?;
        Self::create_host(
            root,
            purpose,
            peer_issuer,
            Some((listen, public_base.trim_end_matches('/').to_owned())),
        )
    }

    fn create_host(
        root: &Path,
        purpose: BootstrapPurpose,
        peer_issuer: Option<(&str, &str)>,
        http: Option<(SocketAddr, String)>,
    ) -> io::Result<Self> {
        require_unix_bootstrap()?;
        let root = absolute(root)?;
        require_absent(&root)?;
        if (purpose == BootstrapPurpose::Join) != peer_issuer.is_some() {
            return Err(invalid(
                "join requires authenticated peer/issuer evidence; create does not",
            ));
        }
        if let Some((peer, issuer)) = peer_issuer {
            device_service::hosted_join::NodeJoinRequest::validate_trusted_peer(peer)?;
            validate_issuer(issuer)?;
        }
        let listener = if http.is_none() {
            Some(TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?)
        } else {
            None
        };
        let listen = match &http {
            Some((listen, _)) => *listen,
            None => listener
                .as_ref()
                .expect("native port reservation")
                .local_addr()?,
        };
        let key_namespace = Id::now_v7();
        let host = Host {
            version: 1,
            root: root.clone(),
            listen,
            key_namespace,
            purpose,
            issuer: peer_issuer.map_or_else(
                || match purpose {
                    BootstrapPurpose::AwaitingChoice => String::new(),
                    _ => format!(
                        "{}/idp",
                        http.as_ref()
                            .map(|(_, base)| base.clone())
                            .unwrap_or_else(|| format!("https://{listen}"))
                    ),
                },
                |(_, issuer)| issuer.to_owned(),
            ),
            trusted_peer: peer_issuer.map(|(peer, _)| peer.to_owned()),
            setup_attempted: (purpose == BootstrapPurpose::AwaitingChoice).then_some(false),
            explicit: None,
            public_base: http.map(|(_, base)| base),
        };
        let retained = Self::retain(host, None)?;
        drop(listener);
        Ok(retained)
    }

    fn retain(host: Host, explicit_config: Option<UnifiedConfig>) -> io::Result<Self> {
        if host.purpose != BootstrapPurpose::AwaitingChoice {
            validate_host_issuer(&host.issuer)?;
        }
        let root = &host.root;
        create_private_directory(root, true)?;
        let operation = lock(root, true)?;
        let store = crate::host_store::HostStore::create(root)?;
        store.put("config", &host)?;
        Ok(Self {
            host,
            _operation: operation,
            explicit_config,
            fresh: true,
            store,
        })
    }

    /// Existing-only opening; missing or partial host identity is not freshness or port regeneration.
    pub fn open(root: &Path) -> io::Result<Self> {
        let host = Self::read(root)?;
        if host.host.explicit.is_some() {
            return Err(invalid(
                "retained explicit host requires its exact configuration",
            ));
        }
        Ok(host)
    }

    fn read(root: &Path) -> io::Result<Self> {
        require_unix_bootstrap()?;
        let root = absolute(root)?;
        let operation = lock(&root, false)?;
        let store = crate::host_store::HostStore::open(&root)?;
        let host: Host = store.get("config")?.ok_or_else(|| invalid("missing host configuration"))?;
        crate::LocalTls::api_base(host.listen)?;
        if host.purpose != BootstrapPurpose::AwaitingChoice {
            validate_host_issuer(&host.issuer)?;
            if host.purpose == BootstrapPurpose::Join {
                validate_issuer(&host.issuer)?;
            }
        }
        if host.version != 1
            || host.root != root
            || host.key_namespace.is_nil()
            || (host.purpose == BootstrapPurpose::AwaitingChoice && host.setup_attempted.is_none())
            || (host.purpose != BootstrapPurpose::AwaitingChoice
                && host.setup_attempted.is_some_and(|attempted| !attempted))
            || (host.purpose == BootstrapPurpose::AwaitingChoice && !host.issuer.is_empty())
            || (host.explicit.is_some() && host.setup_attempted.is_none())
            || (host.purpose == BootstrapPurpose::Join) != host.trusted_peer.is_some()

        {
            return Err(invalid(
                "invalid retained host identity; exact recovery required",
            ));
        }
        if let Some(peer) = &host.trusted_peer {
            device_service::hosted_join::NodeJoinRequest::validate_trusted_peer(peer)?;
        }
        Ok(Self {
            host,
            _operation: operation,
            explicit_config: None,
            fresh: false,
            store,
        })
    }

    pub fn setup_attempted(&self) -> bool {
        self.host.setup_attempted == Some(true)
    }

    pub fn begin_setup(&mut self) -> io::Result<()> {
        if !self.fresh
            || self.host.purpose != BootstrapPurpose::AwaitingChoice
            || self.host.setup_attempted != Some(false)
        {
            return Err(invalid(
                "fresh setup production already consumed; exact recovery required",
            ));
        }
        self.fresh = false;
        let mut selected: Host = serde_json::from_value(serde_json::to_value(&self.host)?)?;
        selected.setup_attempted = Some(true);
        self.retain_selection(selected)?;
        Ok(())
    }

    pub fn select_create(&mut self) -> io::Result<()> {
        if self.host.purpose == BootstrapPurpose::Create && self.setup_attempted() {
            return Ok(());
        }
        if self.host.purpose != BootstrapPurpose::AwaitingChoice || !self.setup_attempted() {
            return Err(invalid(
                "Create requires retained neutral host prerequisites",
            ));
        }
        let mut selected: Host = serde_json::from_slice(&serde_json::to_vec(&self.host)?)?;
        selected.purpose = BootstrapPurpose::Create;
        selected.issuer = match &self.host.explicit {
            Some(identity) => identity["issuer"]
                .as_str()
                .ok_or_else(|| {
                    invalid("retained explicit issuer is missing; exact recovery required")
                })?
                .to_owned(),
            None => self.config().idp.api_public_uri,
        };
        validate_host_issuer(&selected.issuer)?;
        self.retain_selection(selected)
    }

    pub fn select_join(&mut self, peer: &str, issuer: &str) -> io::Result<()> {
        device_service::hosted_join::NodeJoinRequest::validate_trusted_peer(peer)?;
        validate_issuer(issuer)?;
        validate_host_issuer(issuer)?;
        if self.host.purpose == BootstrapPurpose::Join
            && self.setup_attempted()
            && self.host.trusted_peer.as_deref() == Some(peer)
            && self.host.issuer == issuer
        {
            return Ok(());
        }
        if self.host.purpose != BootstrapPurpose::AwaitingChoice || !self.setup_attempted() {
            return Err(invalid(
                "Join requires exact retained neutral host prerequisites",
            ));
        }
        let mut selected: Host = serde_json::from_slice(&serde_json::to_vec(&self.host)?)?;
        selected.purpose = BootstrapPurpose::Join;
        selected.trusted_peer = Some(peer.into());
        selected.issuer = issuer.into();
        self.retain_selection(selected)
    }

    fn retain_selection(&mut self, selected: Host) -> io::Result<()> {
        self.store.update(|records| {
            if records.get("config") != Some(&serde_json::to_value(&self.host)?) {
                return Err(invalid("retained host metadata changed while guarded"));
            }
            records.insert("config".into(), serde_json::to_value(&selected)?);
            Ok(())
        })?;
        self.host = selected;
        Ok(())
    }

    pub(crate) fn store(&self) -> crate::host_store::HostStore { self.store.clone() }

    pub fn directory(&self) -> &Path {
        &self.host.root
    }

    pub fn explicit_directory(config: &UnifiedConfig) -> PathBuf {
        config.endpoint_data_dir.with_extension("host")
    }

    pub fn create_explicit(config: UnifiedConfig) -> io::Result<Self> {
        Self::create_explicit_host(config, true)
    }

    pub fn create_explicit_http(config: UnifiedConfig) -> io::Result<Self> {
        Self::create_explicit_host(config, false)
    }

    fn create_explicit_host(config: UnifiedConfig, https: bool) -> io::Result<Self> {
        require_unix_bootstrap()?;
        crate::config::require_fresh_roots(&config, true, https)?;
        let root = absolute(&Self::explicit_directory(&config))?;
        crate::config::require_host_separation(&config, &root)?;
        require_absent(&root)?;
        let explicit = explicit_identity(&config)?;
        Self::retain(
            Host {
                version: 1,
                root,
                listen: config.listen_addr,
                key_namespace: Id::now_v7(),
                purpose: BootstrapPurpose::AwaitingChoice,
                issuer: String::new(),
                trusted_peer: None,
                setup_attempted: Some(false),
                explicit: Some(explicit),
                public_base: None,
            },
            Some(config),
        )
    }

    pub fn open_explicit(config: UnifiedConfig) -> io::Result<Self> {
        let mut host = Self::read(&Self::explicit_directory(&config))?;
        if host.host.explicit.as_ref() != Some(&explicit_identity(&config)?)
            || host.host.listen != config.listen_addr
            || (host.host.purpose == BootstrapPurpose::Create
                && host.host.issuer != configured_issuer(&config))
        {
            return Err(invalid(
                "explicit configuration conflicts with retained host identity; restore the exact configuration",
            ));
        }
        host.explicit_config = Some(config);
        Ok(host)
    }

    pub fn purpose(&self) -> BootstrapPurpose {
        self.host.purpose
    }
    pub fn trusted_peer(&self) -> Option<&str> {
        self.host.trusted_peer.as_deref()
    }
    pub fn set_runtime_config(&mut self, config: UnifiedConfig) {
        self.explicit_config = Some(config);
    }

    pub fn config(&self) -> UnifiedConfig {
        if let Some(config) = &self.explicit_config {
            let mut settings = config.clone();
            settings.idp.oauth2.issuer = self.host.issuer.clone();
            return settings;
        }
        let root = &self.host.root;
        let base = self
            .host
            .public_base
            .clone()
            .unwrap_or_else(|| format!("https://{}", self.host.listen));
        let idp_uri = format!("{base}/idp");
        let storage_uri = format!("{base}/storage");
        let idp = idp_server::AppConfig {
            data_dir: root.join("idp").to_string_lossy().into_owned(),
            key_namespace: format!("unified-node:{}", self.host.key_namespace),
            api_public_uri: idp_uri.clone(),
            ui_public_uri: base.clone(),
            oauth2: idp_service::oauth2::OAuth2Config {
                issuer: self.host.issuer.clone(),
                ..Default::default()
            },
            service_audience: Some(idp_uri),
            ..Default::default()
        };
        let device = device_server::AppConfig {
            data_dir: root.join("device").to_string_lossy().into_owned(),
            api_public_uri: format!("{base}/device"),
            storage_audience: storage_uri.clone(),
            ..Default::default()
        };
        let storage = storage_server::AppConfig {
            data_dir: root.join("storage").to_string_lossy().into_owned(),
            api_public_uri: storage_uri,
            ..Default::default()
        };
        UnifiedConfig {
            listen_addr: self.host.listen,
            endpoint_data_dir: root.join("endpoint"),
            idp,
            management: management_server::AppConfig {
                api_public_uri: format!("{base}/management"),
                ..Default::default()
            },
            device,
            storage,
        }
    }
}

fn configured_issuer(config: &UnifiedConfig) -> &str {
    if config.idp.oauth2.issuer.is_empty() {
        &config.idp.api_public_uri
    } else {
        &config.idp.oauth2.issuer
    }
}

fn explicit_identity(config: &UnifiedConfig) -> io::Result<serde_json::Value> {
    if config.idp.oauth2.local_signer.is_some() {
        return Err(invalid(
            "neutral startup requires no configured Node signer",
        ));
    }
    crate::LocalTls::api_base(config.listen_addr)?;
    validate_host_issuer(configured_issuer(config))?;
    Ok(serde_json::json!({
        "roots": [
            absolute(Path::new(&config.idp.data_dir))?,
            absolute(Path::new(&config.device.data_dir))?,
            absolute(Path::new(&config.storage.data_dir))?,
            absolute(&config.endpoint_data_dir)?,
        ],
        "listen": config.listen_addr,
        "key_namespace": config.idp.key_namespace,
        "issuer": configured_issuer(config),
        "applications": [config.idp.api_public_uri, config.device.api_public_uri, config.storage.api_public_uri],
        "ui_origin": config.idp.ui_public_uri,
        "audiences": [config.idp.service_audience, Some(config.device.storage_audience.clone())],
    }))
}

fn validate_host_issuer(issuer: &str) -> io::Result<()> {
    if issuer.trim() != issuer {
        return Err(invalid(
            "configured issuer must not contain surrounding whitespace",
        ));
    }
    let url = reqwest::Url::parse(issuer).map_err(|_| invalid("invalid configured issuer URI"))?;
    if url.host_str().is_some_and(|host| {
        host == "installation.invalid" || host.ends_with(".installation.invalid")
    }) {
        return Err(invalid(
            "fabricated installation.invalid issuer is unsupported; stop this installation and recover its original configured issuer and matching identity from backup, or explicitly provision a separate fresh installation; existing data and tokens must not be rewritten",
        ));
    }
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid(
            "configured issuer must be an HTTP(S) URI without credentials, query or fragment",
        ));
    }
    Ok(())
}

pub fn present(path: &Path) -> io::Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

impl BootstrapConfig {
    /// Nonsecret, exact protected-key references for an explicit stopped native reset.
    /// Missing/corrupt owner journals are not evidence that key production never started.
    pub fn local_reset_references(&self) -> io::Result<Value> {
        let mut config = self.config();
        if present(&config.credential_bootstrap_directory())? {
            config = CredentialBootstrap::open(config)?.load()?;
        }
        let provisioning = config.local_provisioning_directory();
        let keys = if present(&provisioning)? {
            match self.purpose() {
                BootstrapPurpose::Create => LocalProvisioner::open(config)?.reset_references()?,
                BootstrapPurpose::Join => LocalJoin::open(config)?.reset_references()?,
                BootstrapPurpose::AwaitingChoice => {
                    return Err(io::Error::other("neutral host has unexpected owner state"));
                }
            }
        } else {
            for path in [
                Path::new(&config.idp.data_dir),
                Path::new(&config.device.data_dir),
                Path::new(&config.storage.data_dir),
                &config.endpoint_data_dir,
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
            "issuer": self.config().idp.oauth2.issuer,
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
        if !keys["endpoint_secret"].is_null() {
            let reference: crate::endpoint_secret::EndpointSecret =
                serde_json::from_value(keys["endpoint_secret"].clone())?;
            reference.delete()?;
        }
        let config = CredentialBootstrap::open(self.config())?.load()?;
        if keys["inventory_attempted"] == false {
            require_absent(Path::new(&config.idp.data_dir))?;
            return Ok(());
        }
        config.idp.require_provisioned_identity()?;
        let owner = idp_server::OwnerProvisioner::open(&config.idp).await?;
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
