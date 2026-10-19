use std::io;

use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndependentCreateIntent {
    pub purpose: String,
    pub operation_nonce: String,
    pub administrator: String,
    pub node: String,
    pub idp: String,
    pub management: String,
    pub storage: String,
    pub issuer: String,
    pub peer: Option<String>,
}

impl IndependentCreateIntent {
    pub fn base(&self, owner: &str) -> io::Result<&str> {
        match owner {
            "idp" => Ok(&self.idp),
            "management" => Ok(&self.management),
            "storage" => Ok(&self.storage),
            _ => Err(io::Error::other("invalid independent owner")),
        }
    }

    pub fn canonical(&self) -> io::Result<String> {
        if self.purpose != "create"
            || self.peer.is_some()
            || self.issuer != self.idp
            || self.operation_nonce.len() != 32
            || !self
                .operation_nonce
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || [&self.administrator, &self.node]
                .iter()
                .any(|s| s.trim().is_empty() || s.len() > 256 || s.chars().any(char::is_control))
        {
            return Err(io::Error::other("invalid exact independent Create intent"));
        }
        for base in [&self.idp, &self.management, &self.storage] {
            let url = reqwest::Url::parse(base)
                .map_err(|_| io::Error::other("invalid owner HTTPS base"))?;
            if url.scheme() != "https"
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
                || url.as_str().trim_end_matches('/') != *base
            {
                return Err(io::Error::other("owner base must be exact canonical HTTPS"));
            }
        }
        if self.idp == self.management
            || self.idp == self.storage
            || self.management == self.storage
        {
            return Err(io::Error::other(
                "independent owners require distinct bases",
            ));
        }
        let value = serde_json::to_string(self)?;
        if value.len() > 1024 {
            return Err(io::Error::other("independent operation is too large"));
        }
        Ok(value)
    }
}

#[derive(Deserialize)]
pub struct IndependentCreateRequest {
    #[serde(flatten)]
    pub intent: IndependentCreateIntent,
    pub password: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndependentOwnerPublic {
    pub operation: IndependentCreateIntent,
    pub owner: String,
    pub clients: Vec<serde_json::Value>,
    pub facts: serde_json::Value,
    pub registered: Vec<String>,
}
