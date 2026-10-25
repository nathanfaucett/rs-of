use std::sync::{Arc, OnceLock};

use storage_server::LocalReplicationAuthorizer;
use tokio_util::sync::CancellationToken;

pub(crate) fn owner_replication_authorizer(
    authority: device_server::DeviceAuthority,
    local_endpoint: iroh::EndpointId,
    copies: Arc<OnceLock<storage_server::StorageCopyRetirement>>,
) -> LocalReplicationAuthorizer {
    let selected = authority.clone();
    let copies = Arc::downgrade(&copies);
    LocalReplicationAuthorizer::from_callbacks(
        move |endpoint| {
            let authority = selected.clone();
            async move {
                authority
                    .selected_resources(&endpoint.to_string())
                    .await
                    .map_err(|error| format!("{error:?}"))
            }
        },
        move |pending| {
            let authority = authority.clone();
            async move {
                authority
                    .acknowledge_owner_cleanup(&pending)
                    .await
                    .map_err(|error| format!("{error:?}"))
            }
        },
        Some((local_endpoint, CancellationToken::new())),
        move || {
            copies
                .upgrade()
                .ok_or("Storage runtime was released")?
                .get()
                .cloned()
                .ok_or_else(|| "Storage cleanup is not ready".into())
        },
    )
}
