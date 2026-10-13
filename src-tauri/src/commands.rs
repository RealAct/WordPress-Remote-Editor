//! IPC commands for the shell webview. Each must also be listed in build.rs
//! and granted in a capability file.

use crate::auth;
use crate::editor::{self, Bounds, EditorCommand, EditorReport};
use crate::error::{AppError, AppResult};
use crate::keychain::{self, Credentials};
use crate::wp::client::{header_u64, WpClient};
use crate::wp::discovery;
use crate::wp::models::*;
use crate::AppState;
use std::collections::HashMap;
use tauri::{AppHandle, Manager, State, Webview};
use uuid::Uuid;

fn client_for(state: &AppState, site_id: &Uuid) -> AppResult<(crate::sites::SiteRecord, WpClient)> {
    let site = state.store.get(site_id)?;
    let creds = keychain::load(site_id)?;
    let client = WpClient::new(site.api_root.clone(), &creds, site.allow_insecure_local)?;
    Ok((site, client))
}

fn valid_rest_base(s: &str) -> AppResult<&str> {
    if !s.is_empty()
        && s.len() <= 64
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        Ok(s)
    } else {
        Err(AppError::Internal("invalid content type".into()))
    }
}

#[tauri::command]
pub async fn discover_site(url: String, allow_insecure_local: bool) -> AppResult<SiteDiscovery> {
    discovery::discover(&url, allow_insecure_local).await
}

#[tauri::command]
pub async fn begin_authorization(
    app: AppHandle,
    url: String,
    allow_insecure_local: bool,
) -> AppResult<()> {
    let d = discovery::discover(&url, allow_insecure_local).await?;
    auth::begin(&app, d, allow_insecure_local)
}

#[tauri::command]
pub async fn add_site_manual(
    app: AppHandle,
    url: String,
    username: String,
    app_password: String,
    allow_insecure_local: bool,
) -> AppResult<SiteSummary> {
    let app_password = zeroize::Zeroizing::new(app_password);
    if username.trim().is_empty() || app_password.trim().is_empty() {
        return Err(AppError::Unauthorized);
    }
    let d = discovery::discover(&url, allow_insecure_local).await?;
    let creds = Credentials::new(&username, &app_password);
    Ok(auth::finalize_site(&app, d, creds, allow_insecure_local)
        .await?
        .summary())
}

#[tauri::command]
pub fn list_sites(state: State<'_, AppState>) -> Vec<SiteSummary> {
    state.store.list().iter().map(|s| s.summary()).collect()
}

#[tauri::command]
pub async fn remove_site(app: AppHandle, site_id: Uuid) -> AppResult<()> {
    let state = app.state::<AppState>();
    let site = state.store.get(&site_id)?;
    // Best effort: revoke the Application Password on the server.
    if let (Some(uuid), Ok((_, client))) = (&site.app_password_uuid, client_for(&state, &site_id)) {
        if uuid.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
            if let Err(e) = client
                .delete(&format!("wp/v2/users/me/application-passwords/{uuid}"), &[])
                .await
            {
                log::warn!("couldn't revoke application password: {e}");
            }
        }
    }
    if state
        .editor
        .lock()
        .expect("poisoned")
        .as_ref()
        .map(|s| s.site_id)
        == Some(site_id)
    {
        editor::close(&app)?;
    }
    keychain::delete(&site_id)?;
    state.store.remove(&site_id)?;
    editor::clear_site_data(&app, &site_id);
    Ok(())
}

#[tauri::command]
pub async fn get_site_context(state: State<'_, AppState>, site_id: Uuid) -> AppResult<SiteContext> {
    let (site, client) = client_for(&state, &site_id)?;
    let (me, _) = client
        .get::<RestUser>("wp/v2/users/me", &[("context", "edit".into())])
        .await?;
    let (types, _) = client
        .get::<HashMap<String, RestPostType>>("wp/v2/types", &[("context", "edit".into())])
        .await?;
    let mut post_types: Vec<PostTypeAccess> = types
        .values()
        .filter(|t| t.viewable && t.slug != "attachment")
        .filter(|t| t.rest_namespace.as_deref().unwrap_or("wp/v2") == "wp/v2")
        .map(|t| post_type_access(t, &me.capabilities))
        .filter(|a| a.can_create || me.capabilities.get("edit_posts").copied().unwrap_or(false))
        .collect();
    // Posts, then Pages, then everything else alphabetically.
    post_types.sort_by_key(|t| match t.slug.as_str() {
        "post" => (0, String::new()),
        "page" => (1, String::new()),
        _ => (2, t.name.to_lowercase()),
    });
    Ok(SiteContext {
        site: site.summary(),
        user_id: me.id,
        roles: me.roles,
        post_types,
    })
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPostsQuery {
    site_id: Uuid,
    rest_base: String,
    status: Option<String>,
    search: Option<String>,
    page: Option<u32>,
    mine_only: bool,
    can_edit_others: bool,
    user_id: u64,
}

#[tauri::command]
pub async fn list_posts(state: State<'_, AppState>, query: ListPostsQuery) -> AppResult<PostPage> {
    let ListPostsQuery {
        site_id,
        rest_base,
        status,
        search,
        page,
        mine_only,
        can_edit_others,
        user_id,
    } = query;
    let (_, client) = client_for(&state, &site_id)?;
    let rest_base = valid_rest_base(&rest_base)?;
    let page = page.unwrap_or(1).clamp(1, 10_000);
    let status = match status.as_deref() {
        Some(s @ ("publish" | "draft" | "pending" | "future" | "private" | "trash")) => {
            s.to_string()
        }
        _ => "any".to_string(),
    };
    let mut q: Vec<(&str, String)> = vec![
        ("context", "edit".into()),
        ("per_page", "30".into()),
        ("page", page.to_string()),
        ("status", status),
        ("orderby", "modified".into()),
        ("order", "desc".into()),
        ("_fields", "id,title,status,modified,author,link".into()),
    ];
    if let Some(s) = search.filter(|s| !s.trim().is_empty()) {
        q.push(("search", s.chars().take(200).collect()));
    }
    if mine_only {
        q.push(("author", user_id.to_string()));
    }
    let (posts, headers) = client
        .get::<Vec<RestPost>>(&format!("wp/v2/{rest_base}"), &q)
        .await?;
    let items = posts
        .into_iter()
        .map(|p| PostSummary {
            can_edit: can_edit_others || p.author == Some(user_id),
            title: p
                .title
                .and_then(|t| t.raw.or(t.rendered))
                .unwrap_or_default(),
            id: p.id,
            status: p.status,
            modified: p.modified,
            author: p.author,
            link: p
                .link
                .filter(|l| l.starts_with("https://") || l.starts_with("http://")),
        })
        .collect();
    Ok(PostPage {
        items,
        total: header_u64(&headers, "x-wp-total"),
        total_pages: header_u64(&headers, "x-wp-totalpages"),
        page,
    })
}

#[tauri::command]
pub async fn trash_post(
    state: State<'_, AppState>,
    site_id: Uuid,
    rest_base: String,
    post_id: u64,
) -> AppResult<()> {
    let (_, client) = client_for(&state, &site_id)?;
    let rest_base = valid_rest_base(&rest_base)?;
    // Without `force`, WordPress moves the post to the trash (recoverable).
    client
        .delete(&format!("wp/v2/{rest_base}/{post_id}"), &[])
        .await
}

#[tauri::command]
pub async fn open_editor(
    app: AppHandle,
    site_id: Uuid,
    post_type: String,
    post_id: Option<u64>,
    bounds: Bounds,
) -> AppResult<()> {
    editor::open(&app, site_id, &post_type, post_id, bounds).await
}

#[tauri::command]
pub fn close_editor(app: AppHandle) -> AppResult<()> {
    editor::close(&app)
}

#[tauri::command]
pub fn editor_set_bounds(app: AppHandle, bounds: Bounds) -> AppResult<()> {
    editor::set_bounds(&app, bounds)
}

/// Hides the editor pane while a shell sheet (such as About) is shown, since
/// the pane is a separate webview drawn above the shell.
#[tauri::command]
pub fn editor_set_hidden(app: AppHandle, hidden: bool) -> AppResult<()> {
    editor::set_hidden(&app, hidden)
}

#[tauri::command]
pub fn editor_command(app: AppHandle, command: EditorCommand) -> AppResult<()> {
    editor::command(&app, command)
}

#[tauri::command]
pub fn editor_report(app: AppHandle, webview: Webview, report: EditorReport) -> AppResult<()> {
    editor::report(&app, &webview, report)
}
