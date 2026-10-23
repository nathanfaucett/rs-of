use std::{io, path::PathBuf, time::Duration};

use serde::{Deserialize, Serialize};

use crate::Client;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteDatabaseConfig {
    pub endpoint: String,
    pub ca_certificate: Option<PathBuf>,
    pub client_certificate: Option<PathBuf>,
    pub client_key: Option<PathBuf>,
    #[serde(default = "default_deadline")]
    pub deadline_ms: u64,
}

fn default_deadline() -> u64 {
    10_000
}

impl RemoteDatabaseConfig {
    pub fn validate(&self) -> io::Result<()> {
        let invalid = || {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "remote identity SQL requires a finite deadline, a bare loopback HTTP endpoint or mTLS HTTPS endpoint",
            )
        };
        let endpoint = url::Url::parse(&self.endpoint).map_err(|_| invalid())?;
        if self.deadline_ms == 0
            || self.deadline_ms > 300_000
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
            || endpoint.path() != "/"
            || endpoint.host_str().is_none()
        {
            return Err(invalid());
        }
        let loopback = match endpoint.host() {
            Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
            Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
            _ => false,
        };
        match endpoint.scheme() {
            "http"
                if loopback
                    && self.ca_certificate.is_none()
                    && self.client_certificate.is_none()
                    && self.client_key.is_none() =>
            {
                Ok(())
            }
            "https"
                if self.ca_certificate.is_some()
                    && self.client_certificate.is_some()
                    && self.client_key.is_some() =>
            {
                Ok(())
            }
            _ => Err(invalid()),
        }
    }

    pub async fn connect(&self) -> io::Result<Client> {
        self.validate()?;
        let deadline = Duration::from_millis(self.deadline_ms);
        let operation = async {
            match (
                &self.ca_certificate,
                &self.client_certificate,
                &self.client_key,
            ) {
                (Some(ca), Some(certificate), Some(key)) => Client::connect_with_identity(
                    &self.endpoint,
                    std::fs::read(ca)?,
                    std::fs::read(certificate)?,
                    std::fs::read(key)?,
                )
                .await
                .map_err(io::Error::other),
                _ => Client::connect(&self.endpoint)
                    .await
                    .map_err(io::Error::other),
            }
        };
        tokio::time::timeout(deadline, operation)
            .await
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::TimedOut,
                    "identity SQL connection deadline exceeded",
                )
            })?
            .map(|client| client.with_deadline(deadline))
    }
}

#[cfg(test)]
mod tests {
    use super::RemoteDatabaseConfig;

    #[test]
    fn remote_sql_configuration_fails_closed() {
        let mut config = RemoteDatabaseConfig {
            endpoint: "http://127.0.0.1:50051".into(),
            ca_certificate: None,
            client_certificate: None,
            client_key: None,
            deadline_ms: 1000,
        };
        assert!(config.validate().is_ok());
        for endpoint in [
            "http://sql.example:50051",
            "http://0.0.0.0:50051",
            "https://sql.example:50051",
            "http://user:secret@127.0.0.1:50051",
            "http://127.0.0.1:50051/path",
            "http://127.0.0.1:50051?query=1",
        ] {
            config.endpoint = endpoint.into();
            assert!(config.validate().is_err(), "reject {endpoint}");
        }
        config.endpoint = "http://127.0.0.1:50051".into();
        config.deadline_ms = 0;
        assert!(config.validate().is_err());
        config.deadline_ms = 1000;
        config.client_key = Some("key.pem".into());
        assert!(config.validate().is_err());
        config.endpoint = "https://sql.example:50051".into();
        config.ca_certificate = Some("ca.pem".into());
        config.client_certificate = Some("client.pem".into());
        assert!(config.validate().is_ok());
    }
}
