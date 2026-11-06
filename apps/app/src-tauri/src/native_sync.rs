use std::io;

use tauri::{AppHandle, Manager, Wry, async_runtime::Mutex};
use unified_server::{StorageSyncRequest, StorageSyncResult};

use crate::native_host::NativeHost;

#[tauri::command]
pub async fn native_database_sync_available(app: AppHandle<Wry>) -> bool {
    let state = app.state::<Mutex<Option<NativeHost>>>();
    state.lock().await.as_ref().and_then(NativeHost::sync_driver).is_some()
}

#[tauri::command]
pub async fn synchronize_native_database(app: AppHandle<Wry>, token: String, request: StorageSyncRequest, target_device_api_url: String) -> Result<StorageSyncResult, String> {
    let (driver, client) = {
        let state = app.state::<Mutex<Option<NativeHost>>>();
        let guard = state.lock().await;
        let host = guard.as_ref().ok_or("Native host is unavailable")?;
        (host.sync_driver().ok_or("Native resource DATA is not ready")?, host.http_client())
    };
    driver.synchronize(&token, request, async move {
        let url = reqwest::Url::parse(&target_device_api_url).map_err(io::Error::other)?;
        if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() || url.query().is_some() || url.fragment().is_some() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Use the target Device HTTPS API prefix without credentials, query, or fragment"));
        }
        let configuration = of_device_client::apis::configuration::Configuration {
            base_path: target_device_api_url.trim_end_matches('/').to_owned(), client,
            ..Default::default()
        };
        let device = of_device_client::apis::default_api::device(&configuration).await.map_err(|_| io::Error::other("Target Device metadata is unavailable over verified HTTPS"))?;
        let address: iroh::EndpointAddr = serde_json::from_str(&device.address).map_err(|_| io::Error::other("Target Device returned an invalid iroh address"))?;
        if address.id.to_string() != device.public_key { return Err(io::Error::new(io::ErrorKind::PermissionDenied, "Target Device metadata identity mismatch")); }
        Ok(address)
    }).await.map_err(|error| match error.kind() {
        io::ErrorKind::Unsupported => error.to_string(),
        io::ErrorKind::PermissionDenied => "Current User, Application, Device and resource authorization was denied".into(),
        _ => "Database sync failed or was cancelled. Earlier committed changes can remain; no completed sync is recorded".into(),
    })
}
