use std::io;

use iroh::{EndpointId, Signature};
use serde::{Deserialize, Serialize};

use crate::LocalTls;

#[derive(Clone, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct IndependentPeer {
    pub nonce: String,
    pub issuer: String,
    pub idp: String,
    pub device: String,
    pub endpoint: String,
    pub address: serde_json::Value,
    pub signature: String,
}

impl IndependentPeer {
    pub fn payload(&self) -> io::Result<Vec<u8>> {
        Ok(serde_json::to_vec(&(
            "of.independent-issuer.v1",
            &self.nonce,
            &self.issuer,
            &self.idp,
            &self.device,
            &self.endpoint,
            &self.address,
        ))?)
    }

    pub fn choice(choice: &str) -> io::Result<(&str, EndpointId)> {
        let (base, pin) = choice.split_once('#').ok_or_else(|| {
            io::Error::other("peer requires its HTTPS Management base and pinned endpoint")
        })?;
        exact_https(base)?;
        let endpoint: EndpointId = pin.parse().map_err(io::Error::other)?;
        if endpoint.to_string() != pin {
            return Err(io::Error::other("peer endpoint must be canonical"));
        }
        Ok((base, endpoint))
    }

    pub fn verify(&self, choice: &str, nonce: &str) -> io::Result<()> {
        let (base, pin) = Self::choice(choice)?;
        exact_https(&self.idp)?;
        exact_https(&self.issuer)?;
        if self.nonce != nonce || self.device != base || self.endpoint != pin.to_string() {
            return Err(io::Error::other("issuer discovery binding mismatch"));
        }
        if self.signature.len() != 128
            || !self.signature.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(io::Error::other("invalid peer signature"));
        }
        let bytes = (0..64)
            .map(|index| u8::from_str_radix(&self.signature[index * 2..index * 2 + 2], 16))
            .collect::<Result<Vec<_>, _>>()
            .map_err(io::Error::other)?;
        let signature = Signature::try_from(bytes.as_slice()).map_err(io::Error::other)?;
        pin.verify(&self.payload()?, &signature)
            .map_err(io::Error::other)
    }

    pub async fn discover(choice: &str) -> io::Result<Self> {
        let (base, _) = Self::choice(choice)?;
        let nonce = crate::independent_exchange::random_capability()?;
        let configuration = of_device_client::apis::configuration::Configuration {
            base_path: base.to_owned(),
            client: LocalTls::verification_client()?,
            ..Default::default()
        };
        let response = of_device_client::apis::independent_join_api::independent_join_discover(
            &configuration,
            of_device_client::apis::independent_join_api::IndependentJoinDiscoverParams {
                discovery_request: of_device_client::models::DiscoveryRequest {
                    nonce: nonce.clone(),
                },
            },
        )
        .await
        .map_err(|_| io::Error::other("trusted peer discovery unavailable or Join denied"))?;
        let peer: Self = serde_json::from_value(serde_json::to_value(response)?)?;
        peer.verify(choice, &nonce)?;
        Ok(peer)
    }
}

pub fn exact_https(base: &str) -> io::Result<()> {
    let url = reqwest::Url::parse(base).map_err(io::Error::other)?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.as_str().trim_end_matches('/') != base
    {
        return Err(io::Error::other("exact canonical HTTPS base required"));
    }
    Ok(())
}
