fn main() {
    // Every app command must be declared here. Tauri then generates an
    // `allow-<command>` permission for each one, and a webview can only call
    // commands that a capability file explicitly grants it.
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "discover_site",
            "begin_authorization",
            "add_site_manual",
            "list_sites",
            "remove_site",
            "get_site_context",
            "list_posts",
            "trash_post",
            "open_editor",
            "close_editor",
            "editor_set_bounds",
            "editor_command",
            "editor_report",
        ]),
    ))
    .expect("failed to run tauri-build");
}
