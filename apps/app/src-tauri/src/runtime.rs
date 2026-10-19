use std::io;

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder, Window, WindowEvent, Wry};
#[cfg(any(windows, target_os = "linux"))]
use tauri_plugin_deep_link::DeepLinkExt;

use crate::app;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }));
    }

    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_deep_link::init())
        .invoke_handler(tauri::generate_handler![
            app::get_localhost_server_base_url,
            app::get_setup_stage,
            app::get_desktop_client_id,
            app::get_native_setup_code,
            app::get_native_host_error,
            app::retry_local_runtime,
            app::reset_device,
            app::recover_reset_references,
            app::revoke_reset_on_peer,
            app::pending_reset_revocations,
            app::complete_device_setup
        ])
        .setup(|app| {
            let startup = (|| -> Result<(), Box<dyn std::error::Error>> {
                if cfg!(debug_assertions) {
                    app.handle().plugin(
                        tauri_plugin_log::Builder::default()
                            .level(log::LevelFilter::Warn)
                            .level_for("lidp", log::LevelFilter::Debug)
                            .build(),
                    )?;
                }
                if cfg!(any(windows, target_os = "linux")) {
                    app.deep_link().register_all()?;
                }

                let app_handle = app.handle().clone();
                if let Err(error) =
                    tauri::async_runtime::block_on(app::start_native_host(&app_handle))
                {
                    log::error!(
                        "native HTTPS host unavailable; retained state was not replaced: {error}"
                    );
                }

                let window =
                    WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                        .title("Local")
                        .inner_size(800.0, 600.0)
                        .resizable(true)
                        .fullscreen(false)
                        .build()?;
                if cfg!(debug_assertions) {
                    window.open_devtools();
                }
                Ok(())
            })();
            tauri::async_runtime::block_on(app::finish_managed_startup(
                startup,
                app::close(app.handle()),
            ))
        })
        .on_window_event(on_window_event)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn on_window_event(window: &Window<Wry>, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event {
        api.prevent_close();
        let app_handle = window.app_handle().clone();
        tauri::async_runtime::spawn(async move {
            let result = app::close(&app_handle).await;
            exit_after_close(result, |code| app_handle.exit(code));
        });
    }
}

pub(crate) fn exit_after_close(result: io::Result<()>, exit: impl FnOnce(i32)) {
    let code = match result {
        Ok(()) => 0,
        Err(error) => {
            log::error!("desktop cleanup before exit failed: {error}");
            1
        }
    };
    exit(code);
}
