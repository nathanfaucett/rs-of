use std::io;

pub(crate) async fn check(base: &str, owner: &str, client: &reqwest::Client) -> io::Result<()> {
    match owner {
        "idp" => {
            let configuration = of_idp_client::apis::configuration::Configuration {
                base_path: base.to_owned(),
                client: client.clone(),
                ..Default::default()
            };
            of_idp_client::apis::default_api::health(&configuration)
                .await
                .map(|_| ())
                .map_err(io::Error::other)
        }
        "management" => {
            let configuration = of_management_client::apis::configuration::Configuration {
                base_path: base.to_owned(),
                client: client.clone(),
                ..Default::default()
            };
            of_management_client::apis::default_api::health(&configuration)
                .await
                .map(|_| ())
                .map_err(io::Error::other)
        }
        "storage" => {
            let configuration = of_storage_client::apis::configuration::Configuration {
                base_path: base.to_owned(),
                client: client.clone(),
                ..Default::default()
            };
            of_storage_client::apis::default_api::health(&configuration)
                .await
                .map(|_| ())
                .map_err(io::Error::other)
        }
        _ => Err(io::Error::other("unknown owner health contract")),
    }
}
