use std::{fs, io, path::Path};

use serde_json::Value;
use tauri::{AppHandle, Manager, Wry, async_runtime::Mutex};

use crate::{native_host::NativeHost, setup_reset as reset};

static LIFECYCLE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub(crate) fn reject_legacy_state(data: &Path, config: &Path) -> io::Result<()> {
    for path in [
        data.join("lidp.redb"),
        config.join("lidp.redb"),
        data.join("services"),
        data.join("setup-ids.json"),
        data.join("node-setup.json"),
        data.join("setup-ready"),
        data.join("setup-provisioning.json"),
        data.join("reset-pending"),
        data.join("storage-residency.json"),
        data.join("vaults"),
        data.join("https-port"),
        data.join("localhost-ca.key"),
        data.join("localhost-server.key"),
    ] {
        match fs::symlink_metadata(&path) {
            Ok(_) => {
                return Err(io::Error::other(format!(
                    "legacy native setup state at {}; exact explicit recovery is required, not migration or replacement",
                    path.display()
                )));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => (),
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

pub(crate) async fn start_native_host(app: &AppHandle<Wry>) -> io::Result<()> {
    let result = start_host(app).await;
    let error = result.as_ref().err().map(ToString::to_string);
    if let Some(state) = app.try_state::<Mutex<Option<String>>>() {
        *state.lock().await = error;
    } else {
        app.manage(Mutex::new(error));
    }
    result
}

#[tauri::command]
pub async fn get_native_host_error(app: AppHandle<Wry>) -> Option<String> {
    let state = app.try_state::<Mutex<Option<String>>>()?;
    state.lock().await.clone()
}

async fn start_host(app: &AppHandle<Wry>) -> io::Result<()> {
    let data = app.path().app_data_dir().map_err(io::Error::other)?;
    let config = app.path().app_config_dir().map_err(io::Error::other)?;
    reject_legacy_state(&data, &config)?;
    if reset::pending(&data)? {
        return Err(io::Error::other(
            "native reset is pending; explicitly retry reset or recover its references",
        ));
    }
    let host = NativeHost::start(&data.join("node")).await?;
    if let Some(state) = app.try_state::<Mutex<Option<NativeHost>>>() {
        let mut guard = state.lock().await;
        if guard.is_some() {
            return Err(io::Error::other("native HTTPS host is already running"));
        }
        *guard = Some(host);
    } else {
        app.manage(Mutex::new(Some(host)));
    }
    Ok(())
}

#[tauri::command]
pub async fn get_localhost_server_base_url(app: AppHandle<Wry>) -> String {
    let Some(state) = app.try_state::<Mutex<Option<NativeHost>>>() else {
        return String::new();
    };
    state
        .lock()
        .await
        .as_ref()
        .map(|host| host.base_url().to_owned())
        .unwrap_or_default()
}

async fn status(app: &AppHandle<Wry>) -> io::Result<Value> {
    let state = app
        .try_state::<Mutex<Option<NativeHost>>>()
        .ok_or_else(|| io::Error::other("native host unavailable"))?;
    let guard = state.lock().await;
    guard
        .as_ref()
        .ok_or_else(|| io::Error::other("native host stopped"))?
        .status()
        .await
}

/// Protected local host management only. Never expose this capability through HTTPS status.
#[tauri::command]
pub async fn get_native_setup_code(app: AppHandle<Wry>) -> Result<Option<String>, String> {
    let _operation = LIFECYCLE.lock().await;
    let data = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    if reset::pending(&data).map_err(|error| error.to_string())? {
        return Err("native reset is pending".into());
    }
    let path = data.join("node/setup-admission/state");
    let bytes = reset::read_optional(&path)
        .map_err(|error| error.to_string())?
        .ok_or("native setup capability unavailable")?;
    let text = std::str::from_utf8(&bytes).map_err(|_| "invalid native setup capability")?;
    let mut lines = text.splitn(3, '\n');
    if lines.next() != Some("setup-admission-v1") {
        return Err("invalid native setup capability".into());
    }
    let code = lines.next().ok_or("missing native setup capability")?;
    let intent = lines.next().ok_or("missing native setup intent")?;
    if code == "retired" && !intent.is_empty() {
        return Ok(None);
    }
    if code.len() != 64
        || !code
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("invalid native setup capability".into());
    }
    Ok(Some(code.to_owned()))
}

#[tauri::command]
pub async fn get_setup_stage(app: AppHandle<Wry>) -> Result<String, String> {
    let data = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    if reset::pending(&data).map_err(|error| error.to_string())? {
        return Ok("installation".into());
    }
    let status = match status(&app).await {
        Ok(status) => status,
        Err(_) => return Ok("unavailable".into()),
    };
    if status["state"] == "unavailable" {
        return Ok("unavailable".into());
    }
    if status["ready"] != true {
        return Ok("installation".into());
    }
    let client = status["login_client"]["client_id"]
        .as_str()
        .ok_or("ready native host has no public login Client")?;
    crate::setup_completion::read(&data, "device-setup-ready", client)
        .map(|done| if done { "ready" } else { "device" }.into())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_desktop_client_id(app: AppHandle<Wry>) -> Result<String, String> {
    let status = status(&app).await.map_err(|error| error.to_string())?;
    if status["ready"] != true {
        return Err("native Node is not ready".into());
    }
    status["login_client"]["client_id"]
        .as_str()
        .map(str::to_owned)
        .ok_or("ready native host has no public login Client".into())
}

#[tauri::command]
pub async fn complete_device_setup(app: AppHandle<Wry>) -> Result<(), String> {
    let _operation = LIFECYCLE.lock().await;
    let data = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    if reset::pending(&data).map_err(|error| error.to_string())? {
        return Err("native reset is pending".into());
    }
    let client = get_desktop_client_id(app.clone()).await?;
    crate::setup_completion::write(&data, "device-setup-ready", &client)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn retry_local_runtime(app: AppHandle<Wry>) -> Result<(), String> {
    let _operation = LIFECYCLE.lock().await;
    close_host(&app).await.map_err(|error| error.to_string())?;
    start_native_host(&app)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn reset_device(app: AppHandle<Wry>) -> Result<(), String> {
    let _operation = LIFECYCLE.lock().await;
    close_host(&app).await.map_err(|error| error.to_string())?;
    let data = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    reset::run(&data).await.map_err(|error| error.to_string())?;
    app.exit(0);
    Ok(())
}

#[tauri::command]
pub async fn recover_reset_references(app: AppHandle<Wry>) -> Result<(), String> {
    let _operation = LIFECYCLE.lock().await;
    close_host(&app).await.map_err(|error| error.to_string())?;
    let data = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    reset::recover(&data).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn pending_reset_revocations(app: AppHandle<Wry>) -> Result<Value, String> {
    let data = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    reset::revocations(&data)
        .map(Value::Array)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn revoke_reset_on_peer(
    app: AppHandle<Wry>,
    peer_api: String,
    bearer: String,
    member_id: idp_model::model::Id,
) -> Result<(), String> {
    let _operation = LIFECYCLE.lock().await;
    let data = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    let local_api = format!("{}/idp", get_localhost_server_base_url(app.clone()).await);
    let client = crate::setup_peer_revocation::client_builder()
        .build()
        .map_err(|error| error.to_string())?;
    crate::setup_peer_revocation::revoke(&data, &local_api, &peer_api, &bearer, member_id, &client)
        .await
}

pub(crate) async fn close(app: &AppHandle<Wry>) -> io::Result<()> {
    let _operation = LIFECYCLE.lock().await;
    close_host(app).await
}

async fn close_host(app: &AppHandle<Wry>) -> io::Result<()> {
    if let Some(state) = app.try_state::<Mutex<Option<NativeHost>>>() {
        let mut guard = state.lock().await;
        if let Some(mut host) = guard.take() {
            host.close().await?;
        }
    }
    Ok(())
}

pub(crate) async fn finish_managed_startup<E>(
    startup: Result<(), E>,
    cleanup: impl core::future::Future<Output = io::Result<()>>,
) -> Result<(), E> {
    if startup.is_err()
        && let Err(error) = cleanup.await
    {
        log::error!("native startup cleanup failed: {error}");
    }
    startup
}
