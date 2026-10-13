mod auth;
mod commands;
mod editor;
mod error;
mod keychain;
mod menu;
mod security;
mod sites;
mod wp;

use std::sync::Mutex;
use tauri::{LogicalPosition, Manager};

pub struct AppState {
    pub store: sites::SiteStore,
    pub pending_auth: Mutex<Option<auth::PendingAuth>>,
    pub editor: Mutex<Option<editor::EditorSession>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_window("main") {
                w.unminimize().ok();
                w.set_focus().ok();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            app.manage(AppState {
                store: sites::SiteStore::load(data_dir.join("sites.json")),
                pending_auth: Mutex::new(None),
                editor: Mutex::new(None),
            });
            create_main_window(app)?;
            let menu = menu::build(app.handle())?;
            app.set_menu(menu)?;
            app.on_menu_event(|app, event| menu::forward(app, event.id().as_ref()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::discover_site,
            commands::begin_authorization,
            commands::add_site_manual,
            commands::list_sites,
            commands::remove_site,
            commands::get_site_context,
            commands::list_posts,
            commands::trash_post,
            commands::open_editor,
            commands::close_editor,
            commands::editor_set_bounds,
            commands::editor_command,
            commands::editor_report,
        ])
        .run(tauri::generate_context!())
        .expect("error while running WordPress Remote Editor");
}

/// The main window is a plain window hosting two webviews: the shell UI
/// (resized with the window) and, when open, the editor placed over the
/// shell's editor area. Building it this way keeps child-webview layout
/// identical on macOS, Windows and Linux.
fn create_main_window(app: &mut tauri::App) -> tauri::Result<()> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == "main")
        .cloned()
        .expect("main window missing from tauri.conf.json");
    let window = tauri::window::WindowBuilder::from_config(app, &config)?.build()?;
    let size = window
        .inner_size()?
        .to_logical::<f64>(window.scale_factor()?);
    window.add_child(
        tauri::webview::WebviewBuilder::from_config(&config).auto_resize(),
        LogicalPosition::new(0.0, 0.0),
        size,
    )?;
    Ok(())
}
