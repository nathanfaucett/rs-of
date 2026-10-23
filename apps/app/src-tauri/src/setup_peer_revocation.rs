use std::path::Path;

use idp_model::model::Id;
use management_server::MANAGEMENT_APPLICATION_URI;

use crate::setup_reset as reset;

pub(super) fn client_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(15))
}

pub(super) async fn revoke(
    data: &Path,
    local_api: &str,
    peer_api: &str,
    bearer: &str,
    member_id: Id,
    client: &reqwest::Client,
) -> Result<(), String> {
    let target = reset::revocations(data)
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|target| target["member_id"] == member_id.to_string())
        .ok_or("no retained public revocation target for this member")?;
    if peer_api == local_api {
        return Err(
            "revocation must be acknowledged by a surviving peer, not this local Node".into(),
        );
    }
    let url = reqwest::Url::parse(peer_api).map_err(|error| error.to_string())?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/idp"
        || url.as_str() != peer_api
        || bearer.trim().is_empty()
    {
        return Err(
            "select a trusted HTTPS peer IdP base URL and an authorized User session".into(),
        );
    }
    let mut idp = of_idp_client::apis::configuration::Configuration {
        base_path: peer_api.to_owned(),
        client: client.clone(),
        ..Default::default()
    };
    let metadata = of_idp_client::apis::default_api::openid_configuration(&idp)
        .await
        .map_err(|error| error.to_string())?;
    if target["issuer"].as_str() != Some(metadata.issuer.as_str()) {
        return Err("peer belongs to a different Installation; no bearer was sent".into());
    }
    let device_api = format!("{}/device", url.origin().ascii_serialization());
    for (idp_owner, resource) in [(true, peer_api), (false, MANAGEMENT_APPLICATION_URI)] {
        idp.bearer_access_token = None;
        let token = of_idp_client::apis::default_api::token(&idp,
            of_idp_client::apis::default_api::TokenParams {
                grant_type: Some("urn:ietf:params:oauth:grant-type:token-exchange".to_owned()),
                subject_token_type: Some(of_idp_client::models::SubjectTokenType::UrnColonIetfColonParamsColonOauthColonTokenTypeColonAccessToken),
                subject_token: Some(bearer.to_owned()), resource: Some(resource.to_owned()), ..Default::default()
            }).await.map_err(|error| format!("peer User audience exchange denied; target retained: {error}"))?;
        if token.access_token.is_empty() {
            return Err(
                "peer did not return an authorized User access token; target retained".into(),
            );
        }
        if idp_owner {
            idp.bearer_access_token = Some(token.access_token);
            of_idp_client::apis::super_routes_replica_signers_api::revoke_replica_signer(
                &idp,
                of_idp_client::apis::super_routes_replica_signers_api::RevokeReplicaSignerParams {
                    member_id: member_id.to_string(),
                },
            )
            .await
            .map_err(|error| {
                format!("peer signer revocation incomplete; public target retained: {error}")
            })?;
        } else {
            let configuration = of_device_client::apis::configuration::Configuration {
                base_path: device_api.clone(),
                client: client.clone(),
                bearer_access_token: Some(token.access_token),
                ..Default::default()
            };
            of_device_client::apis::default_api::revoke_device(
                &configuration,
                of_device_client::apis::default_api::RevokeDeviceParams {
                    id: member_id.to_string(),
                },
            )
            .await
            .map_err(|error| {
                format!("peer Device revocation incomplete; public target retained: {error}")
            })?;
        }
    }
    let mut acknowledged = target;
    acknowledged["status"] = serde_json::json!("acknowledged");
    reset::publish(
        data,
        &format!("reset-revocation-{member_id}.json"),
        &acknowledged,
    )
    .map_err(|error| error.to_string())
}

#[cfg(all(test, unix))]
#[path = "setup_peer_revocation_tests.rs"]
mod tests;
