//! Application Password authorization.
//!
//! WordPress's own `authorize-application.php` screen is opened in an
//! isolated, incognito window with no IPC access. The user signs in with
//! their normal login (2FA/SSO keep working) and approves the app. WordPress
//! then redirects to `wpremoteeditor://auth/callback?...`, which we intercept
//! inside our own webview, so no other app can claim the scheme.

use crate::error::{AppError, AppResult};
use crate::keychain::{self, Credentials};
use crate::sites::SiteRecord;
use crate::wp::client::WpClient;
use crate::wp::models::{RestUser, SiteDiscovery, SiteSummary};
use crate::AppState;
use rand::RngCore;
use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use url::Url;
use uuid::Uuid;
use zeroize::Zeroizing;

pub const CALLBACK_SCHEME: &str = "wpremoteeditor";
/// Stable app ID so WordPress groups all passwords created by this app.
pub const APP_ID: &str = "8f4d7f3e-6b0e-4c1a-9a58-2f0b8f7f9b21";
pub const APP_NAME: &str = "WordPress Remote Editor";
const AUTH_WINDOW: &str = "auth";

pub struct PendingAuth {
    pub state: String,
    pub discovery: SiteDiscovery,
    pub allow_insecure_local: bool,
}

fn random_state() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// WordPress rejects a second Application Password with the same name, so
/// name each one after this device and the time it was created. That also
/// tells the user which computer a password belongs to on their profile page.
pub fn password_name(device: &str, now: chrono::DateTime<chrono::Local>) -> String {
    let device: String = device
        .chars()
        .filter(|c| !c.is_control())
        .take(40)
        .collect();
    let stamp = now.format("%Y-%m-%d %H:%M");
    if device.trim().is_empty() {
        format!("{APP_NAME} ({stamp})")
    } else {
        format!("{APP_NAME} ({}, {stamp})", device.trim())
    }
}

pub fn authorize_url(endpoint: &Url, state: &str, name: &str) -> Url {
    let mut url = endpoint.clone();
    url.query_pairs_mut()
        .append_pair("app_name", name)
        .append_pair("app_id", APP_ID)
        .append_pair(
            "success_url",
            &format!("{CALLBACK_SCHEME}://auth/callback?state={state}"),
        )
        .append_pair(
            "reject_url",
            &format!("{CALLBACK_SCHEME}://auth/rejected?state={state}"),
        );
    url
}

/// Opens the authorization window. Completion is reported to the shell via
/// the `auth://completed` or `auth://failed` events.
pub fn begin(
    app: &AppHandle,
    discovery: SiteDiscovery,
    allow_insecure_local: bool,
) -> AppResult<()> {
    let endpoint = discovery
        .authorization_endpoint
        .clone()
        .ok_or(AppError::ApplicationPasswordsUnavailable)?;
    let state = random_state();
    let name = password_name(&tauri_plugin_os::hostname(), chrono::Local::now());
    let url = authorize_url(&endpoint, &state, &name);
    *app.state::<AppState>()
        .pending_auth
        .lock()
        .expect("poisoned") = Some(PendingAuth {
        state,
        discovery: discovery.clone(),
        allow_insecure_local,
    });

    // Reuse an open sign-in window (closing is asynchronous, so its label
    // can't be reused immediately). Its navigation guard is site-independent.
    if let Some(existing) = app.get_webview_window(AUTH_WINDOW) {
        existing.set_title(&format!("Sign in to {}", discovery.name))?;
        existing.navigate(url)?;
        existing.unminimize().ok();
        existing.set_focus().ok();
        return Ok(());
    }

    let handle = app.clone();
    let window = WebviewWindowBuilder::new(app, AUTH_WINDOW, WebviewUrl::External(url))
        .title(format!("Sign in to {}", discovery.name))
        .inner_size(520.0, 720.0)
        .resizable(true)
        .center()
        // Fresh, throwaway cookie jar: nothing from this sign-in persists.
        .incognito(true)
        .on_navigation(move |nav| {
            if nav.scheme() == CALLBACK_SCHEME {
                let handle = handle.clone();
                let nav = nav.clone();
                tauri::async_runtime::spawn(async move {
                    complete(&handle, nav).await;
                });
                return false;
            }
            // SSO providers may live on other hosts, but never allow
            // downgrades or non-web schemes in the sign-in window.
            nav.scheme() == "https"
                || (allow_insecure_local && nav.scheme() == "http")
                || nav.scheme() == "about"
        })
        .build()?;
    // The app menu is attached to every window on Windows and Linux; the
    // sign-in window shouldn't offer editor commands.
    #[cfg(not(target_os = "macos"))]
    window.remove_menu().ok();
    #[cfg(target_os = "macos")]
    let _ = window;
    Ok(())
}

#[derive(Deserialize)]
struct Introspection {
    uuid: String,
}

async fn complete(app: &AppHandle, callback: Url) {
    let result = handle_callback(app, &callback).await;
    if let Some(w) = app.get_webview_window(AUTH_WINDOW) {
        w.close().ok();
    }
    match result {
        Ok(summary) => {
            app.emit_to("main", "auth://completed", summary).ok();
        }
        Err(e) => {
            app.emit_to("main", "auth://failed", e.to_string()).ok();
        }
    }
}

async fn handle_callback(app: &AppHandle, callback: &Url) -> AppResult<SiteSummary> {
    let pending = app
        .state::<AppState>()
        .pending_auth
        .lock()
        .expect("poisoned")
        .take()
        .ok_or(AppError::AuthFlow)?;

    let mut state = None;
    let mut user_login = None;
    let mut password = Zeroizing::new(String::new());
    for (k, v) in callback.query_pairs() {
        match k.as_ref() {
            "state" => state = Some(v.into_owned()),
            "user_login" => user_login = Some(v.into_owned()),
            "password" => password = Zeroizing::new(v.into_owned()),
            _ => {}
        }
    }
    if !state.is_some_and(|s| constant_time_eq(&s, &pending.state)) {
        return Err(AppError::AuthFlow);
    }
    if callback.host_str() != Some("auth") || callback.path() != "/callback" {
        return Err(AppError::AuthFlow);
    }
    let user_login = user_login.ok_or(AppError::AuthFlow)?;
    if password.is_empty() {
        return Err(AppError::AuthFlow);
    }

    let creds = Credentials::new(&user_login, &password);
    let record = finalize_site(app, pending.discovery, creds, pending.allow_insecure_local).await?;
    Ok(record.summary())
}

/// Verifies credentials against the site, then stores the password in the
/// keychain and the site metadata on disk.
pub async fn finalize_site(
    app: &AppHandle,
    discovery: SiteDiscovery,
    creds: Credentials,
    allow_insecure_local: bool,
) -> AppResult<SiteRecord> {
    let client = WpClient::new(discovery.api_root.clone(), &creds, allow_insecure_local)?;
    let (me, _) = client
        .get::<RestUser>("wp/v2/users/me", &[("context", "edit".into())])
        .await?;
    let app_password_uuid = client
        .get::<Introspection>("wp/v2/users/me/application-passwords/introspect", &[])
        .await
        .ok()
        .map(|(i, _)| i.uuid);

    let avatar_url = ["96", "48", "24"]
        .iter()
        .find_map(|k| me.avatar_urls.get(*k))
        .and_then(|u| Url::parse(u).ok())
        .filter(|u| u.scheme() == "https");

    let store = &app.state::<AppState>().store;
    let record = store.upsert(SiteRecord {
        id: Uuid::new_v4(),
        name: discovery.name,
        url: discovery.site_url,
        admin_url: discovery.admin_url,
        api_root: discovery.api_root,
        username: creds.username.clone(),
        display_name: if me.name.is_empty() {
            creds.username.clone()
        } else {
            me.name
        },
        avatar_url,
        app_password_uuid,
        bridge_available: discovery.bridge_available,
        allow_insecure_local,
        added_at: chrono::Utc::now(),
    })?;
    if let Err(e) = keychain::save(&record.id, &creds) {
        store.remove(&record.id).ok();
        return Err(e);
    }
    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_authorize_url_with_state() {
        let ep = Url::parse("https://example.com/wp-admin/authorize-application.php").unwrap();
        let u = authorize_url(
            &ep,
            "abc",
            "WordPress Remote Editor (laptop, 2026-10-13 09:00)",
        );
        let pairs: std::collections::HashMap<_, _> = u.query_pairs().into_owned().collect();
        assert_eq!(
            pairs["app_name"],
            "WordPress Remote Editor (laptop, 2026-10-13 09:00)"
        );
        assert_eq!(pairs["app_id"], APP_ID);
        assert_eq!(
            pairs["success_url"],
            "wpremoteeditor://auth/callback?state=abc"
        );
        assert_eq!(
            pairs["reject_url"],
            "wpremoteeditor://auth/rejected?state=abc"
        );
    }

    #[test]
    fn password_names_are_unique_per_device_and_time() {
        use chrono::TimeZone;
        let t = chrono::Local
            .with_ymd_and_hms(2026, 10, 13, 9, 5, 0)
            .unwrap();
        assert_eq!(
            password_name("eli-mbp", t),
            "WordPress Remote Editor (eli-mbp, 2026-10-13 09:05)"
        );
        assert_eq!(
            password_name("  ", t),
            "WordPress Remote Editor (2026-10-13 09:05)"
        );
        assert!(!password_name("bad\nname", t).contains('\n'));
    }

    #[test]
    fn state_is_random_and_long() {
        let (a, b) = (random_state(), random_state());
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
    }

    #[test]
    fn constant_time_eq_works() {
        assert!(constant_time_eq("abc", "abc"));
        assert!(!constant_time_eq("abc", "abd"));
        assert!(!constant_time_eq("abc", "abcd"));
    }
}
