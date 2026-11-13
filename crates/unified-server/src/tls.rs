use std::{
    fs,
    io::{self, Write},
    net::SocketAddr,
    path::Path,
    sync::Arc,
    time::Duration,
};

use rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{TlsAcceptor, server::TlsStream};

use crate::{TlsConfig, UnifiedConfig};

fn paths(config: &UnifiedConfig) -> Option<(std::path::PathBuf, std::path::PathBuf)> {
    match &config.tls {
        TlsConfig::Disabled => None,
        TlsConfig::AutoGenerate => Some((
            config.data_dir.join("tls/cert.pem"),
            config.data_dir.join("tls/key.pem"),
        )),
        TlsConfig::External {
            cert_path,
            key_path,
        } => Some((cert_path.clone(), key_path.clone())),
    }
}

pub fn load(config: &UnifiedConfig) -> io::Result<Option<Arc<rustls::ServerConfig>>> {
    let Some((cert_path, key_path)) = paths(config) else {
        return Ok(None);
    };
    if matches!(config.tls, TlsConfig::AutoGenerate)
        && !cert_path.try_exists()?
        && !key_path.try_exists()?
    {
        fs::create_dir_all(config.data_dir.join("tls"))?;
        let certificate = rcgen::generate_simple_self_signed(vec![
            "localhost".into(),
            "127.0.0.1".into(),
            "::1".into(),
        ])
        .map_err(io::Error::other)?;
        write_private(
            &key_path,
            certificate.signing_key.serialize_pem().as_bytes(),
        )?;
        write_private(&cert_path, certificate.cert.pem().as_bytes())?;
    }
    let certificates = CertificateDer::pem_slice_iter(&fs::read(cert_path)?)
        .collect::<Result<Vec<_>, _>>()
        .map_err(io::Error::other)?;
    let key = PrivateKeyDer::from_pem_slice(&fs::read(key_path)?).map_err(io::Error::other)?;
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let server = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(io::Error::other)?
        .with_no_client_auth()
        .with_single_cert(certificates, key)
        .map_err(io::Error::other)?;
    Ok(Some(Arc::new(server)))
}

pub fn http_client(config: &UnifiedConfig) -> io::Result<reqwest::Client> {
    let mut client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(10));
    if let Some((cert_path, _)) = paths(config) {
        client = client.add_root_certificate(
            reqwest::Certificate::from_pem(&fs::read(cert_path)?).map_err(io::Error::other)?,
        );
    }
    client.build().map_err(io::Error::other)
}

fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

pub struct TlsListener {
    inner: TcpListener,
    acceptor: TlsAcceptor,
}
impl TlsListener {
    pub fn new(inner: TcpListener, config: Arc<rustls::ServerConfig>) -> Self {
        Self {
            inner,
            acceptor: TlsAcceptor::from(config),
        }
    }
}
impl axum::serve::Listener for TlsListener {
    type Io = TlsStream<TcpStream>;
    type Addr = SocketAddr;
    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            match self.inner.accept().await {
                Ok((stream, address)) => {
                    if let Ok(Ok(stream)) =
                        tokio::time::timeout(Duration::from_secs(2), self.acceptor.accept(stream))
                            .await
                    {
                        return (stream, address);
                    }
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
            }
        }
    }
    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }
}
