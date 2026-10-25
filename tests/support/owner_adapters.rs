use std::{io, sync::Arc};

use device_service::HostedControlPlane;
use idp_server::IdpRuntime;
use storage_server::IdpClient;

pub(crate) fn storage_introspection_error(
    error: idp_model::contract::ErrorResponse,
) -> storage_server::IdpIntrospectionError {
    use idp_model::contract::ErrorCode;
    use storage_server::IdpIntrospectionError;

    match error.error {
        ErrorCode::ServerError | ErrorCode::TemporarilyUnavailable => {
            IdpIntrospectionError::ServiceUnavailable
        }
        _ => IdpIntrospectionError::InvalidToken,
    }
}

pub(crate) fn local_storage_idp_client(idp: Arc<IdpRuntime>) -> IdpClient {
    IdpClient::local(move |token| {
        let idp = Arc::clone(&idp);
        async move {
            idp.authorize_storage_bearer(&token)
                .await
                .map_err(storage_introspection_error)
        }
    })
}

pub(crate) fn build_control_plane(
    idp: Arc<IdpRuntime>,
    devices: Arc<device_service::replica::DbDeviceRepo>,
    idp_api_base: &str,
    storage_api_base: &str,
    issuer: &str,
) -> io::Result<HostedControlPlane> {
    let inspect = Arc::clone(&idp);
    Ok(
        HostedControlPlane::new_with_services(idp_api_base, storage_api_base, issuer)
            .map_err(io::Error::other)?
            .with_local_devices(devices)
            .with_local_idp(
                move |token| {
                    let idp = Arc::clone(&inspect);
                    async move { idp.authorize_management_bearer(&token).await }
                },
                move |user_id| {
                    let idp = Arc::clone(&idp);
                    async move { idp.active_user(user_id).await }
                },
            ),
    )
}

pub(crate) fn attach_security(
    protocol: device_service::security_protocol::SecurityProtocol,
    handler: storage_server::DataProtocolHandler,
) -> storage_server::DataProtocolHandler {
    handler.with_additional_stream(move |kind, connection, send, recv| {
        let protocol = protocol.clone();
        async move {
            protocol
                .accept_data_stream(kind, connection, send, recv)
                .await
        }
    })
}
