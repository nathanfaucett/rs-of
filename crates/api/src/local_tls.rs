use std::{
    fs,
    io::{self, Write},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    path::Path,
    path::PathBuf,
    process::{Command, Stdio},
    sync::Arc,
    time::Duration,
};

use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, KeyPair, SanType,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, pem::PemObject};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{TlsAcceptor, server::TlsStream};

use crate::setup_files::{
    absolute, create_private_directory, private_options, read_private, require_absent,
    sync_ancestry,
};

/// Domain config mapping without a dependency on any composing host.
pub trait LocalTlsConfig {
    fn tls_directory(&self) -> PathBuf;
    fn tls_address(&self) -> SocketAddr;
}

/// Protected retained localhost TLS material. Missing/partial material is not regenerated.
#[derive(Clone)]
pub struct LocalTls {
    directory: PathBuf,
    server: Arc<rustls::ServerConfig>,
}

impl LocalTls {
    pub fn directory(config: &impl LocalTlsConfig) -> PathBuf {
        config.tls_directory()
    }

    /// Explicit first production into an absent directory; interrupted production requires recovery.
    pub fn create(config: &impl LocalTlsConfig) -> io::Result<Self> {
        crate::setup_files::require_unix_bootstrap()?;
        Self::api_base(config.tls_address())?;
        let directory = absolute(&Self::directory(config))?;
        require_absent(&directory)?;
        create_private_directory(&directory, true)?;
        let ca_key = KeyPair::generate().map_err(io::Error::other)?;
        let mut ca = CertificateParams::new(Vec::<String>::new()).map_err(io::Error::other)?;
        ca.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        ca.distinguished_name = DistinguishedName::new();
        ca.distinguished_name
            .push(DnType::CommonName, "Node localhost CA");
        let certificate = ca.self_signed(&ca_key).map_err(io::Error::other)?;
        let issuer = rcgen::Issuer::new(ca, &ca_key);
        let key = KeyPair::generate().map_err(io::Error::other)?;
        let mut params =
            CertificateParams::new(vec!["localhost".into()]).map_err(io::Error::other)?;
        params.subject_alt_names.extend([
            SanType::IpAddress(Ipv4Addr::LOCALHOST.into()),
            SanType::IpAddress(Ipv6Addr::LOCALHOST.into()),
        ]);
        let server = params.signed_by(&key, &issuer).map_err(io::Error::other)?;
        for (name, bytes) in [
            ("ca.key", ca_key.serialize_pem().into_bytes()),
            ("ca.pem", certificate.pem().into_bytes()),
            ("ca.der", certificate.der().to_vec()),
            ("server.key", key.serialize_der()),
            ("server.der", server.der().to_vec()),
        ] {
            let mut file = private_options(true).open(directory.join(name))?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            sync_ancestry(&directory)?;
        }
        Self::open(config)
    }

    pub fn open(config: &impl LocalTlsConfig) -> io::Result<Self> {
        let directory = absolute(&Self::directory(config))?;
        crate::setup_files::require_private(&directory, true)?;
        let ca_key = read_private(&directory.join("ca.key"))?;
        let ca_key = String::from_utf8(ca_key).map_err(io::Error::other)?;
        KeyPair::from_pem(&ca_key).map_err(io::Error::other)?;
        let ca_pem = read_private(&directory.join("ca.pem"))?;
        let ca = read_private(&directory.join("ca.der"))?;
        if CertificateDer::from_pem_slice(&ca_pem)
            .map_err(io::Error::other)?
            .as_ref()
            != ca.as_slice()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained CA PEM and DER disagree; exact recovery required",
            ));
        }
        let certificate = read_private(&directory.join("server.der"))?;
        let key = read_private(&directory.join("server.key"))?;
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let server = rustls::ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(io::Error::other)?
            .with_no_client_auth()
            .with_single_cert(
                vec![CertificateDer::from(certificate), CertificateDer::from(ca)],
                PrivateKeyDer::from(PrivatePkcs8KeyDer::from(key)),
            )
            .map_err(io::Error::other)?;
        Ok(Self {
            directory,
            server: Arc::new(server),
        })
    }

    pub fn ca_path(&self) -> PathBuf {
        self.directory.join("ca.pem")
    }

    /// Mandatory for both create and join, including continuation; no browser-only fallback.
    pub fn install_os_ca(&self) -> io::Result<()> {
        self.install_os_ca_with(command_succeeds)
    }

    /// Test command injection only; production startup always uses `install_os_ca`.
    #[cfg(any(test, feature = "test-support"))]
    pub fn test_install_os_ca(&self, run: impl FnMut(&mut Command) -> bool) -> io::Result<()> {
        self.install_os_ca_with(run)
    }

    pub(crate) fn install_os_ca_with(
        &self,
        mut run: impl FnMut(&mut Command) -> bool,
    ) -> io::Result<()> {
        crate::setup_files::require_unix_bootstrap()?;
        let path = self.ca_path();
        #[cfg(target_os = "linux")]
        {
            if run(Command::new("trust").arg("anchor").arg(&path)) {
                return Ok(());
            }
            let digest = rustls::crypto::ring::cipher_suite::TLS13_AES_128_GCM_SHA256
                .tls13()
                .expect("TLS 1.3 cipher suite")
                .common
                .hash_provider
                .hash(&fs::read(self.directory.join("ca.der"))?);
            let fingerprint: String = digest
                .as_ref()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            if let Some((directory, refresh)) = linux_trust_store(Path::is_dir)
                && install_linux_ca(&path, &fingerprint, directory, refresh, &mut run)
            {
                return Ok(());
            }
        }
        #[cfg(target_os = "macos")]
        {
            let home =
                std::env::var_os("HOME").ok_or_else(|| io::Error::other("HOME is missing"))?;
            let keychain = PathBuf::from(home).join("Library/Keychains/login.keychain-db");
            if run(Command::new("security")
                .args(["add-trusted-cert", "-r", "trustRoot", "-p", "ssl", "-k"])
                .arg(keychain)
                .arg(path))
            {
                return Ok(());
            }
        }

        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "mandatory OS CA installation failed; approve OS elevation and retry setup; no plaintext/browser fallback",
        ))
    }

    pub fn verification_client() -> io::Result<reqwest::Client> {
        reqwest::Client::builder()
            .use_rustls_tls()
            .tls_built_in_root_certs(false)
            .tls_built_in_native_certs(true)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(Duration::from_secs(1))
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(io::Error::other)
    }

    pub fn listener(&self, listener: TcpListener) -> TlsListener {
        TlsListener {
            inner: listener,
            acceptor: TlsAcceptor::from(self.server.clone()),
        }
    }

    pub fn api_base(address: SocketAddr) -> io::Result<String> {
        Self::validate_ip(address.ip())?;
        if address.port() == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "HTTPS requires a fixed nonzero loopback port",
            ));
        }
        Ok(format!("https://{address}"))
    }

    pub fn validate_ip(ip: IpAddr) -> io::Result<()> {
        if ip != IpAddr::V4(Ipv4Addr::LOCALHOST) && ip != IpAddr::V6(Ipv6Addr::LOCALHOST) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "localhost TLS supports only 127.0.0.1 or ::1",
            ));
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
const LINUX_TRUST_STORES: [(&str, &[&str]); 2] = [
    (
        "/usr/local/share/ca-certificates",
        &["update-ca-certificates"],
    ),
    (
        "/etc/pki/ca-trust/source/anchors",
        &["update-ca-trust", "extract"],
    ),
];

#[cfg(target_os = "linux")]
fn linux_trust_store(
    mut exists: impl FnMut(&Path) -> bool,
) -> Option<(&'static str, &'static [&'static str])> {
    LINUX_TRUST_STORES
        .iter()
        .rev()
        .copied()
        .find(|(directory, _)| exists(Path::new(directory)))
}

#[cfg(target_os = "linux")]
fn install_linux_ca(
    path: &Path,
    fingerprint: &str,
    directory: &str,
    refresh: &[&str],
    run: &mut impl FnMut(&mut Command) -> bool,
) -> bool {
    let destination = Path::new(directory).join(format!("unified-node-{fingerprint}.crt"));
    run(Command::new("pkexec")
        .args(["install", "-m", "0644"])
        .arg(path)
        .arg(destination))
        && run(Command::new("pkexec").args(refresh))
}

fn command_succeeds(command: &mut Command) -> bool {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

pub struct TlsListener {
    inner: TcpListener,
    acceptor: TlsAcceptor,
}
impl axum::serve::Listener for TlsListener {
    type Io = TlsStream<TcpStream>;
    type Addr = SocketAddr;
    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            match self.inner.accept().await {
                Ok((stream, address)) => {
                    if let Ok(Ok(stream)) =
                        tokio::time::timeout(Duration::from_secs(10), self.acceptor.accept(stream))
                            .await
                    {
                        return (stream, address);
                    }
                }
                Err(error) => {
                    log::warn!("local HTTPS accept failed: {error}");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }
    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::{LocalTls, LocalTlsConfig};
    use std::{fs, net::SocketAddr, path::PathBuf};

    struct Config(PathBuf);
    impl LocalTlsConfig for Config {
        fn tls_directory(&self) -> PathBuf {
            self.0.clone()
        }
        fn tls_address(&self) -> SocketAddr {
            "127.0.0.1:3000".parse().expect("address")
        }
    }

    #[test]
    fn https_requires_fixed_loopback_identity() {
        for address in ["127.0.0.1:0", "0.0.0.0:3000", "127.0.0.2:3000"] {
            assert!(LocalTls::api_base(address.parse().expect("address")).is_err());
        }
        assert_eq!(
            LocalTls::api_base("[::1]:3000".parse().expect("address")).expect("IPv6 SAN"),
            "https://[::1]:3000"
        );
        assert_eq!(
            LocalTls::api_base("127.0.0.1:3000".parse().expect("address")).expect("HTTPS base"),
            "https://127.0.0.1:3000"
        );
    }

    #[test]
    fn retained_tls_material_is_not_replaced_after_loss() {
        let root = tempfile::tempdir().expect("isolated root");
        let config = Config(root.path().join("tls"));
        let tls = LocalTls::create(&config).expect("explicit fresh TLS production");
        let ca = fs::read(tls.ca_path()).expect("retained CA");
        LocalTls::open(&config).expect("existing-only reopen");
        assert_eq!(fs::read(tls.ca_path()).expect("unchanged CA"), ca);
        fs::remove_file(LocalTls::directory(&config).join("server.key")).expect("simulate loss");
        assert!(LocalTls::open(&config).is_err());
        assert!(LocalTls::create(&config).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_install_pairs_each_platform_destination_with_its_refresh_command() {
        use std::path::Path;
        assert_eq!(
            super::linux_trust_store(|path| path == Path::new("/usr/local/share/ca-certificates")),
            Some((
                "/usr/local/share/ca-certificates",
                &["update-ca-certificates"][..]
            ))
        );
        assert_eq!(
            super::linux_trust_store(|_| true),
            Some((
                "/etc/pki/ca-trust/source/anchors",
                &["update-ca-trust", "extract"][..]
            ))
        );
        assert!(super::linux_trust_store(|_| false).is_none());
        let mut commands = Vec::new();
        let mut run = |command: &mut std::process::Command| {
            commands.push(
                command
                    .get_args()
                    .map(|arg| arg.to_string_lossy().into_owned())
                    .collect::<Vec<_>>(),
            );
            true
        };
        assert!(super::install_linux_ca(
            Path::new("ca.pem"),
            "fingerprint",
            "/usr/local/share/ca-certificates",
            &["update-ca-certificates"],
            &mut run
        ));
        assert!(super::install_linux_ca(
            Path::new("ca.pem"),
            "fingerprint",
            "/etc/pki/ca-trust/source/anchors",
            &["update-ca-trust", "extract"],
            &mut run
        ));
        assert_eq!(
            commands[0],
            [
                "install",
                "-m",
                "0644",
                "ca.pem",
                "/usr/local/share/ca-certificates/unified-node-fingerprint.crt"
            ]
        );
        assert_eq!(commands[1], ["update-ca-certificates"]);
        assert_eq!(
            commands[2],
            [
                "install",
                "-m",
                "0644",
                "ca.pem",
                "/etc/pki/ca-trust/source/anchors/unified-node-fingerprint.crt"
            ]
        );
        assert_eq!(commands[3], ["update-ca-trust", "extract"]);
    }
}
