use std::{fs, io, path::Path};

use serde_json::Value;
use tauri::{AppHandle, Manager, Wry, async_runtime::Mutex};

use crate::{native_host::NativeHost, setup_reset as reset};

static LIFECYCLE: std::sync::LazyLock<crate::lifecycle::Lifecycle> =
    std::sync::LazyLock::new(crate::lifecycle::Lifecycle::default);

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
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "legacy native setup state at {}; exact explicit recovery is required, not migration or replacement",
                        path.display()
                    ),
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => (),
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

pub(crate) async fn start_native_host(app: &AppHandle<Wry>) -> io::Result<()> {
    start_native_host_with_consent(app, false).await
}

async fn start_native_host_with_consent(app: &AppHandle<Wry>, consent: bool) -> io::Result<()> {
    let Some((_operation, cancellation)) = LIFECYCLE.start().await else {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "Application closing",
        ));
    };
    close_host(app).await?;
    if let Some(state) = app.try_state::<Mutex<Option<crate::onboarding::Failure>>>() {
        *state.lock().await = None;
    }
    let result = start_host(app, consent, cancellation).await;
    let error = result
        .as_ref()
        .err()
        .map(crate::onboarding::Failure::startup);
    if let Some(state) = app.try_state::<Mutex<Option<crate::onboarding::Failure>>>() {
        *state.lock().await = error;
    } else {
        app.manage(Mutex::new(error));
    }
    result
}

async fn start_host(
    app: &AppHandle<Wry>,
    consent: bool,
    cancellation: tokio_util::sync::CancellationToken,
) -> io::Result<()> {
    let data = app.path().app_data_dir().map_err(io::Error::other)?;
    let config = app.path().app_config_dir().map_err(io::Error::other)?;
    reject_legacy_state(&data, &config)?;
    if reset::pending(&data)? {
        return Err(io::Error::other(
            "native reset is pending; explicitly retry reset or recover its references",
        ));
    }
    let host = NativeHost::start(&data.join("node"), consent, cancellation.clone()).await?;
    let state = app.state::<Mutex<Option<NativeHost>>>();
    let mut guard = state.lock().await;
    if let Some(mut host) = LIFECYCLE.publish_host(&cancellation, host, &mut guard) {
        host.close().await?;
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "startup cancelled before publication",
        ));
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

#[tauri::command]
pub async fn get_onboarding_host(app: AppHandle<Wry>) -> crate::onboarding::HostObservation {
    use crate::onboarding::{Failure, HostObservation};
    let mut observation = HostObservation::default();
    let data = match app.path().app_data_dir() {
        Ok(data) => data,
        Err(_) => {
            observation.failure = Some(Failure::Prerequisite);
            return observation;
        }
    };
    match reset::pending(&data) {
        Ok(false) => (),
        _ => {
            observation.failure = Some(Failure::Repair);
            return observation;
        }
    }
    if let Some(state) = app.try_state::<Mutex<Option<Failure>>>() {
        observation.failure = *state.lock().await;
        if observation.failure.is_some() {
            return observation;
        }
    }
    let Some(state) = app.try_state::<Mutex<Option<NativeHost>>>() else {
        return observation;
    };
    let guard = state.lock().await;
    let Some(host) = guard.as_ref() else {
        return observation;
    };
    host.observation(&data).await
}

/// Protected local host management only. Never expose this capability through HTTPS status.
#[tauri::command]
pub async fn get_native_setup_code(app: AppHandle<Wry>) -> Result<Option<String>, String> {
    let _operation = LIFECYCLE.operation.lock().await;
    let data = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    if reset::pending(&data).map_err(|error| error.to_string())? {
        return Err("native reset is pending".into());
    }
    #[cfg(unix)]
    {
        api::SetupAdmission::protected_code(&data.join("node/setup-admission"))
            .map_err(|error| error.to_string())
    }
    #[cfg(not(unix))]
    {
        Err("protected native setup capability unsupported".into())
    }
}

#[tauri::command]
pub async fn complete_device_setup(app: AppHandle<Wry>) -> Result<(), String> {
    let _operation = LIFECYCLE.operation.lock().await;
    let data = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    if reset::pending(&data).map_err(|error| error.to_string())? {
        return Err("native reset is pending".into());
    }
    let observation = current_host_status(&app).await?;
    let client =
        crate::onboarding::login_client(&observation).map_err(|error| error.to_string())?;
    crate::setup_completion::write(&data, "device-setup-ready", client)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn retry_local_runtime(app: AppHandle<Wry>) -> Result<(), String> {
    start_native_host_with_consent(&app, false)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn set_local_trust_consent(app: AppHandle<Wry>, consent: bool) -> Result<(), String> {
    if !consent {
        let _operation = LIFECYCLE.stop(false).await;
        close_host(&app).await.map_err(|error| error.to_string())?;
        if let Some(state) = app.try_state::<Mutex<Option<crate::onboarding::Failure>>>() {
            *state.lock().await = Some(crate::onboarding::Failure::TrustDeclined);
        }
        return Ok(());
    }
    start_native_host_with_consent(&app, true)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn reset_device(app: AppHandle<Wry>) -> Result<(), String> {
    let _operation = LIFECYCLE.stop(false).await;
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
    let _operation = LIFECYCLE.stop(false).await;
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
    let _operation = LIFECYCLE.operation.lock().await;
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
    let _operation = LIFECYCLE.stop(true).await;
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

async fn current_host_status(
    app: &AppHandle<Wry>,
) -> Result<of_idp_client::models::CreateResult, String> {
    let state = app
        .try_state::<Mutex<Option<NativeHost>>>()
        .ok_or("native host unavailable")?;
    let guard = state.lock().await;
    guard
        .as_ref()
        .ok_or("native host stopped")?
        .status()
        .await
        .map_err(|_| "native readiness unavailable".into())
}
