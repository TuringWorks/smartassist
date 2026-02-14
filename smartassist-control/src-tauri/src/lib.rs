mod commands;
mod gateway;
mod ws_client;

use gateway::GatewayManager;
use tauri::Manager;
use ws_client::WsClient;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(GatewayManager::new())
        .manage(WsClient::new())
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config,
            commands::validate_config,
            commands::get_config_path,
            commands::reset_config,
            commands::get_version,
            commands::gateway_status,
            commands::gateway_start,
            commands::gateway_stop,
            commands::gateway_restart,
            commands::rpc_call,
        ])
        .setup(|app| {
            // Build system tray menu
            let title = tauri::menu::MenuItem::with_id(
                app,
                "title",
                "SmartAssist",
                false,
                None::<&str>,
            )?;
            let status = tauri::menu::MenuItem::with_id(
                app,
                "status",
                "Gateway: Stopped",
                false,
                None::<&str>,
            )?;
            let sep1 = tauri::menu::PredefinedMenuItem::separator(app)?;
            let start = tauri::menu::MenuItem::with_id(
                app,
                "gateway_start",
                "Start Gateway",
                true,
                None::<&str>,
            )?;
            let stop = tauri::menu::MenuItem::with_id(
                app,
                "gateway_stop",
                "Stop Gateway",
                true,
                None::<&str>,
            )?;
            let restart = tauri::menu::MenuItem::with_id(
                app,
                "gateway_restart",
                "Restart Gateway",
                true,
                None::<&str>,
            )?;
            let sep2 = tauri::menu::PredefinedMenuItem::separator(app)?;
            let show = tauri::menu::MenuItem::with_id(
                app,
                "show",
                "Open Control Panel",
                true,
                None::<&str>,
            )?;
            let sep3 = tauri::menu::PredefinedMenuItem::separator(app)?;
            let quit =
                tauri::menu::MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = tauri::menu::Menu::with_items(
                app,
                &[
                    &title, &status, &sep1, &start, &stop, &restart, &sep2, &show, &sep3,
                    &quit,
                ],
            )?;

            if let Some(tray) = app.tray_by_id("main") {
                tray.set_menu(Some(menu))?;
                tray.on_menu_event(|app, event| match event.id().as_ref() {
                    "quit" => {
                        app.exit(0);
                    }
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "gateway_start" => {
                        let gw = app.state::<GatewayManager>();
                        let config = smartassist_core::config::Config::load_or_default();
                        let port = config.gateway.port;
                        let provider = config
                            .agents
                            .defaults
                            .model
                            .as_deref()
                            .and_then(|m| m.split('/').next())
                            .unwrap_or("anthropic")
                            .to_string();
                        let gw = gw.inner().clone();
                        tauri::async_runtime::spawn(async move {
                            let _ = gw.start(port, &provider).await;
                        });
                    }
                    "gateway_stop" => {
                        let gw = app.state::<GatewayManager>();
                        let gw = gw.inner().clone();
                        tauri::async_runtime::spawn(async move {
                            let _ = gw.stop().await;
                        });
                    }
                    "gateway_restart" => {
                        let gw = app.state::<GatewayManager>();
                        let config = smartassist_core::config::Config::load_or_default();
                        let port = config.gateway.port;
                        let provider = config
                            .agents
                            .defaults
                            .model
                            .as_deref()
                            .and_then(|m| m.split('/').next())
                            .unwrap_or("anthropic")
                            .to_string();
                        let gw = gw.inner().clone();
                        tauri::async_runtime::spawn(async move {
                            let _ = gw.stop().await;
                            let _ = gw.start(port, &provider).await;
                        });
                    }
                    _ => {}
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running SmartAssist Control Panel");
}
