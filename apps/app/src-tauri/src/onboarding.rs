use std::io;

use of_idp_client::models::CreateResult;
use serde::Serialize;

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Failure {
    Retry,
    Prerequisite,
    Repair,
}

impl Failure {
    pub(crate) fn startup(error: &io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::InvalidData | io::ErrorKind::NotFound => Self::Repair,
            io::ErrorKind::PermissionDenied | io::ErrorKind::Unsupported => Self::Prerequisite,
            _ => Self::Retry,
        }
    }
}

#[derive(Default, Serialize)]
pub(crate) struct HostObservation {
    pub base: Option<String>,
    pub status: Option<CreateResult>,
    pub device_setup_required: bool,
    pub failure: Option<Failure>,
}

pub(crate) fn login_client(status: &CreateResult) -> io::Result<&str> {
    let client = status.login_client.as_ref().and_then(Option::as_ref);
    if status.stage != "installation"
        || status.state != "ready"
        || !status.ready
        || status.owners.len() != 3
        || !["idp", "device", "storage"]
            .iter()
            .all(|owner| status.owners.iter().any(|value| value == owner))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "current Node readiness not verified",
        ));
    }
    let client = client.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "public login Client unavailable",
        )
    })?;
    if client.client_id == "00000000-0000-0000-0000-000000000000"
        || uuid_format(&client.client_id).is_err()
        || !client.scopes.iter().any(|scope| scope == "openid")
        || client
            .scopes
            .iter()
            .any(|scope| scope.is_empty() || scope.chars().any(char::is_whitespace))
        || client
            .scopes
            .iter()
            .enumerate()
            .any(|(index, scope)| client.scopes[..index].contains(scope))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid public login Client",
        ));
    }
    Ok(&client.client_id)
}

fn uuid_format(value: &str) -> Result<(), ()> {
    if value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if [8, 13, 18, 23].contains(&index) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            }
        })
    {
        Ok(())
    } else {
        Err(())
    }
}
