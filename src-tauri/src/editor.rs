//! The editor pane: a child webview that loads the site's own block editor,
//! so every block, sidebar plugin and meta box works exactly as in wp-admin.
//!
//! The webview is treated as untrusted remote content:
//! - navigation is locked to the site's origins; other links open in the
//!   system browser;
//! - each site gets its own cookie store;
//! - it may call exactly one app command, `editor_report`, granted at runtime
//!   for that site's origin only;
//! - the core controls it by evaluating fixed scripts, never page-supplied code.

use crate::error::{AppError, AppResult};
use crate::keychain;
use crate::security::same_origin;
use crate::sites::SiteRecord;
use crate::wp::client::WpClient;
use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::ipc::CapabilityBuilder;
use tauri::webview::WebviewBuilder;
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Webview, WebviewUrl};
use tauri_plugin_opener::OpenerExt;
use url::Url;
use uuid::Uuid;

pub const EDITOR_LABEL: &str = "editor";
const BRIDGE_JS: &str = include_str!("editor_bridge.js");

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Bounds {
    fn sane(self) -> Self {
        let clamp = |v: f64| {
            if v.is_finite() {
                v.clamp(0.0, 20_000.0)
            } else {
                0.0
            }
        };
        Self {
            x: clamp(self.x),
            y: clamp(self.y),
            width: clamp(self.width).max(1.0),
            height: clamp(self.height).max(1.0),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EditorCommand {
    Save,
    Publish,
    Undo,
    Redo,
    ToggleSettings,
    DeviceDesktop,
    DeviceTablet,
    DeviceMobile,
}

impl EditorCommand {
    fn script(self) -> &'static str {
        match self {
            Self::Save => "window.__wpre && window.__wpre.save()",
            Self::Publish => "window.__wpre && window.__wpre.publish()",
            Self::Undo => "window.__wpre && window.__wpre.undo()",
            Self::Redo => "window.__wpre && window.__wpre.redo()",
            Self::ToggleSettings => "window.__wpre && window.__wpre.toggleSettings()",
            Self::DeviceDesktop => "window.__wpre && window.__wpre.device('Desktop')",
            Self::DeviceTablet => "window.__wpre && window.__wpre.device('Tablet')",
            Self::DeviceMobile => "window.__wpre && window.__wpre.device('Mobile')",
        }
    }
}

/// State reported by the bridge script. Every field is bounded; anything
/// unexpected is rejected by serde.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EditorReport {
    pub kind: String,
    #[serde(default)]
    pub post_id: Option<u64>,
    #[serde(default)]
    pub post_type: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub link: Option<String>,
    #[serde(default)]
    pub dirty: Option<bool>,
    #[serde(default)]
    pub saving: Option<bool>,
    #[serde(default)]
    pub can_publish: Option<bool>,
    #[serde(default)]
    pub path: Option<String>,
}

impl EditorReport {
    pub fn validate(mut self) -> AppResult<Self> {
        if !matches!(self.kind.as_str(), "editor" | "login" | "page") {
            return Err(AppError::Internal("invalid editor report".into()));
        }
        let cap = |s: &mut Option<String>, n: usize| {
            if let Some(v) = s {
                if v.chars().count() > n {
                    *v = v.chars().take(n).collect();
                }
            }
        };
        cap(&mut self.post_type, 64);
        cap(&mut self.title, 500);
        cap(&mut self.status, 32);
        cap(&mut self.path, 512);
        // Only pass through web links; the shell may offer "View on site".
        if let Some(l) = &self.link {
            if l.len() > 2048 || !(l.starts_with("https://") || l.starts_with("http://")) {
                self.link = None;
            }
        }
        Ok(self)
    }
}

pub struct EditorSession {
    pub site_id: Uuid,
}

fn origin_pattern(u: &Url) -> String {
    format!("{}/*", u.origin().ascii_serialization())
}

fn editor_target(site: &SiteRecord, post_type: &str, post_id: Option<u64>) -> AppResult<Url> {
    if !post_type
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        || post_type.len() > 20
    {
        return Err(AppError::Internal("invalid post type".into()));
    }
    let mut url = match post_id {
        Some(id) => {
            let mut u = site
                .admin_url
                .join("post.php")
                .map_err(|_| AppError::InvalidUrl)?;
            u.query_pairs_mut()
                .append_pair("post", &id.to_string())
                .append_pair("action", "edit");
            u
        }
        None => {
            let mut u = site
                .admin_url
                .join("post-new.php")
                .map_err(|_| AppError::InvalidUrl)?;
            if post_type != "post" {
                u.query_pairs_mut().append_pair("post_type", post_type);
            }
            u
        }
    };
    url.set_fragment(None);
    Ok(url)
}

#[derive(Deserialize)]
struct Handoff {
    url: String,
}

/// With the companion plugin active, trade the Application Password for a
/// single-use, short-lived login link so the user isn't asked to sign in again.
async fn handoff_url(site: &SiteRecord, target: &Url) -> AppResult<Url> {
    let creds = keychain::load(&site.id)?;
    let client = WpClient::new(site.api_root.clone(), &creds, site.allow_insecure_local)?;
    let h: Handoff = client
        .post(
            "wpre/v1/handoff",
            &serde_json::json!({ "redirect_to": target.as_str() }),
        )
        .await?;
    let url = Url::parse(&h.url).map_err(|_| AppError::BadResponse)?;
    if !same_origin(&site.admin_url, &url) {
        return Err(AppError::BadResponse);
    }
    Ok(url)
}

pub async fn open(
    app: &AppHandle,
    site_id: Uuid,
    post_type: &str,
    post_id: Option<u64>,
    bounds: Bounds,
) -> AppResult<()> {
    let state = app.state::<AppState>();
    let site = state.store.get(&site_id)?;
    let target = editor_target(&site, post_type, post_id)?;
    let initial = if site.bridge_available {
        handoff_url(&site, &target).await.unwrap_or_else(|e| {
            log::warn!("editor handoff unavailable, falling back to site login: {e}");
            target.clone()
        })
    } else {
        target
    };

    // Reuse the existing pane for the same site; recreate it for another site
    // so cookie stores never mix.
    let same_site = state
        .editor
        .lock()
        .expect("poisoned")
        .as_ref()
        .map(|s| s.site_id)
        == Some(site_id);
    if let Some(existing) = app.get_webview(EDITOR_LABEL) {
        if same_site {
            existing.navigate(initial)?;
            set_bounds(app, bounds)?;
            return Ok(());
        }
        existing.close()?;
    }

    let allowed: Vec<Url> = vec![site.admin_url.clone(), site.url.clone()];
    let origins: Vec<String> = allowed
        .iter()
        .map(|u| u.origin().ascii_serialization())
        .collect();
    let config = serde_json::json!({ "origins": origins }).to_string();
    let script = BRIDGE_JS.replace("__WPRE_CONFIG__", &config);

    // Grant the remote page exactly one command, only on this site's origins.
    let mut cap = CapabilityBuilder::new(format!("editor-remote-{site_id}"))
        .webview(EDITOR_LABEL)
        .permission("allow-editor-report");
    for u in &allowed {
        cap = cap.remote(origin_pattern(u));
    }
    app.add_capability(cap)?;

    let handle = app.clone();
    let nav_allowed = allowed.clone();
    let mut builder = WebviewBuilder::new(EDITOR_LABEL, WebviewUrl::External(initial))
        .initialization_script(&script)
        .on_navigation(move |url| {
            if matches!(url.scheme(), "about" | "blob" | "data") {
                return true;
            }
            if nav_allowed.iter().any(|a| same_origin(a, url)) {
                return true;
            }
            if matches!(url.scheme(), "https" | "http" | "mailto") {
                handle.opener().open_url(url.as_str(), None::<&str>).ok();
            }
            false
        });

    #[cfg(target_os = "macos")]
    {
        builder = builder.data_store_identifier(*site_id.as_bytes());
    }
    #[cfg(not(target_os = "macos"))]
    {
        let dir = app
            .path()
            .app_local_data_dir()?
            .join("editor-sessions")
            .join(site_id.to_string());
        builder = builder.data_directory(dir);
    }

    let window = app
        .get_window("main")
        .ok_or_else(|| AppError::Internal("main window missing".into()))?;
    let b = bounds.sane();
    window.add_child(
        builder,
        LogicalPosition::new(b.x, b.y),
        LogicalSize::new(b.width, b.height),
    )?;
    platform_layout(app, Some(b))?;
    *state.editor.lock().expect("poisoned") = Some(EditorSession { site_id });
    Ok(())
}

pub fn set_bounds(app: &AppHandle, bounds: Bounds) -> AppResult<()> {
    if let Some(w) = app.get_webview(EDITOR_LABEL) {
        let b = bounds.sane();
        w.set_position(LogicalPosition::new(b.x, b.y))?;
        w.set_size(LogicalSize::new(b.width, b.height))?;
        platform_layout(app, Some(b))?;
    }
    Ok(())
}

/// On Linux, Tauri stacks a window's webviews in a vertical GTK box and
/// ignores explicit positions. The shell keeps the editor full-width below its
/// toolbar (focus mode, the same layout as macOS and Windows), so on Linux the
/// editor is packed at the bottom of the box with a height of "everything below
/// the toolbar", and the shell, which expands, keeps the space above it.
#[cfg(target_os = "linux")]
fn platform_layout(app: &AppHandle, editor: Option<Bounds>) -> AppResult<()> {
    use gtk::prelude::*;
    let (Some(b), Some(window), Some(webview)) = (
        editor,
        app.get_window("main"),
        app.get_webview(EDITOR_LABEL),
    ) else {
        return Ok(());
    };
    webview.with_webview(move |platform| {
        let Ok(vbox) = window.default_vbox() else {
            return;
        };
        let widget = platform.inner();
        // Space used by non-webview children such as the menu bar.
        let chrome: i32 = vbox
            .children()
            .iter()
            .filter(|c| !c.type_().name().starts_with("WebKit"))
            .map(|c| c.allocated_height())
            .sum();
        let height = (vbox.allocated_height() - chrome - b.y.round() as i32).max(1);
        vbox.set_child_packing(&widget, false, false, 0, gtk::PackType::End);
        widget.set_size_request(-1, height);
    })?;
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn platform_layout(_app: &AppHandle, _editor: Option<Bounds>) -> AppResult<()> {
    Ok(())
}

pub fn command(app: &AppHandle, cmd: EditorCommand) -> AppResult<()> {
    let w = app
        .get_webview(EDITOR_LABEL)
        .ok_or_else(|| AppError::Internal("no editor open".into()))?;
    w.eval(cmd.script())?;
    Ok(())
}

pub fn close(app: &AppHandle) -> AppResult<()> {
    if let Some(w) = app.get_webview(EDITOR_LABEL) {
        w.close()?;
    }
    platform_layout(app, None)?;
    *app.state::<AppState>().editor.lock().expect("poisoned") = None;
    Ok(())
}

/// Called from the remote page. Rejects calls from any webview but the editor.
pub fn report(app: &AppHandle, webview: &Webview, report: EditorReport) -> AppResult<()> {
    if webview.label() != EDITOR_LABEL {
        return Err(AppError::Forbidden);
    }
    let report = report.validate()?;
    app.emit_to("main", "editor://state", report)?;
    Ok(())
}

/// Removes a site's isolated cookie store (on site removal).
pub fn clear_site_data(app: &AppHandle, site_id: &Uuid) {
    #[cfg(not(target_os = "macos"))]
    if let Ok(dir) = app.path().app_local_data_dir() {
        std::fs::remove_dir_all(dir.join("editor-sessions").join(site_id.to_string())).ok();
    }
    #[cfg(target_os = "macos")]
    {
        // WKWebsiteDataStore removal needs the store to be unused; done at next launch.
        let _ = (app, site_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site() -> SiteRecord {
        let url = Url::parse("https://example.com/").unwrap();
        SiteRecord {
            id: Uuid::new_v4(),
            name: "x".into(),
            admin_url: url.join("wp-admin/").unwrap(),
            api_root: url.join("wp-json/").unwrap(),
            url,
            username: "eli".into(),
            display_name: "Eli".into(),
            avatar_url: None,
            app_password_uuid: None,
            bridge_available: false,
            allow_insecure_local: false,
            added_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn editor_urls() {
        let s = site();
        assert_eq!(
            editor_target(&s, "post", Some(42)).unwrap().as_str(),
            "https://example.com/wp-admin/post.php?post=42&action=edit"
        );
        assert_eq!(
            editor_target(&s, "page", None).unwrap().as_str(),
            "https://example.com/wp-admin/post-new.php?post_type=page"
        );
        assert!(editor_target(&s, "page&x=1", None).is_err());
    }

    #[test]
    fn report_validation() {
        let r: EditorReport = serde_json::from_value(serde_json::json!({
            "kind": "editor", "title": "x".repeat(900), "link": "javascript:alert(1)"
        }))
        .unwrap();
        let r = r.validate().unwrap();
        assert_eq!(r.title.unwrap().len(), 500);
        assert!(r.link.is_none());
        let bad: Result<EditorReport, _> =
            serde_json::from_value(serde_json::json!({"kind":"editor","evil":1}));
        assert!(bad.is_err());
        let bad_kind: EditorReport =
            serde_json::from_value(serde_json::json!({"kind":"x"})).unwrap();
        assert!(bad_kind.validate().is_err());
    }

    #[test]
    fn bounds_are_clamped() {
        let b = Bounds {
            x: f64::NAN,
            y: -5.0,
            width: 0.0,
            height: 1e9,
        }
        .sane();
        assert_eq!((b.x, b.y, b.width, b.height), (0.0, 0.0, 1.0, 20_000.0));
    }
}
