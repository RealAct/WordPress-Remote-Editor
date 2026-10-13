//! Native application menu. Standard Edit items matter on macOS: without them
//! clipboard shortcuts don't reach the webviews.

use tauri::menu::{Menu, MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::{AppHandle, Emitter, Runtime};

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let item = |id: &str, label: &str, accel: Option<&str>| {
        let mut b = MenuItemBuilder::with_id(id, label);
        if let Some(a) = accel {
            b = b.accelerator(a);
        }
        b.build(app)
    };

    let file = SubmenuBuilder::new(app, "File")
        .item(&item("new-post", "New Post", Some("CmdOrCtrl+N"))?)
        .item(&item("new-page", "New Page", Some("CmdOrCtrl+Shift+N"))?)
        .separator()
        .item(&item("save", "Save Draft", Some("CmdOrCtrl+S"))?)
        .item(&item("publish", "Publish…", Some("CmdOrCtrl+Shift+P"))?)
        .separator()
        .item(&item("add-site", "Add Site…", None)?)
        .item(&item("close-editor", "Close Editor", Some("CmdOrCtrl+W"))?);
    #[cfg(not(target_os = "macos"))]
    let file = file
        .separator()
        .item(&tauri::menu::PredefinedMenuItem::quit(app, None)?);
    let file = file.build()?;

    let edit = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;

    let view = SubmenuBuilder::new(app, "View")
        .item(&item(
            "toggle-sidebar",
            "Toggle Sidebar",
            Some("CmdOrCtrl+Alt+S"),
        )?)
        .item(&item(
            "toggle-settings",
            "Toggle Block Settings",
            Some("CmdOrCtrl+Shift+,"),
        )?)
        .separator()
        .item(&item("device-desktop", "Preview Desktop", None)?)
        .item(&item("device-tablet", "Preview Tablet", None)?)
        .item(&item("device-mobile", "Preview Mobile", None)?)
        .separator()
        .fullscreen()
        .build()?;

    let window = SubmenuBuilder::new(app, "Window")
        .minimize()
        .maximize()
        .separator()
        .close_window()
        .build()?;

    let help = SubmenuBuilder::new(app, "Help").item(&item("help-docs", "CreatePress Help", None)?);
    // On macOS, About lives in the app menu instead.
    #[cfg(not(target_os = "macos"))]
    let help = help
        .separator()
        .item(&item("about", "About CreatePress", None)?);
    let help = help.build()?;

    #[allow(unused_mut)]
    let mut menu = MenuBuilder::new(app);
    #[cfg(target_os = "macos")]
    {
        let app_menu = SubmenuBuilder::new(app, "CreatePress")
            .item(&item("about", "About CreatePress", None)?)
            .separator()
            .services()
            .separator()
            .hide()
            .hide_others()
            .show_all()
            .separator()
            .quit()
            .build()?;
        menu = menu.item(&app_menu);
    }
    menu.items(&[&file, &edit, &view, &window, &help]).build()
}

/// Forwards custom menu items to the shell as `menu://action` events.
pub fn forward<R: Runtime>(app: &AppHandle<R>, id: &str) {
    app.emit_to("main", "menu://action", id).ok();
}
