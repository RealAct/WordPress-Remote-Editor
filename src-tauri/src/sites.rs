//! Non-secret site metadata, persisted as JSON in the app data directory.
//! Secrets are never written here; see `keychain`.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use url::Url;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteRecord {
    pub id: Uuid,
    pub name: String,
    pub url: Url,
    /// `siteurl` + `wp-admin/`, where the block editor is loaded from.
    pub admin_url: Url,
    pub api_root: Url,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<Url>,
    /// UUID of the Application Password, used to revoke it on removal.
    pub app_password_uuid: Option<String>,
    /// Whether the companion plugin is active (enables editor sign-in handoff).
    pub bridge_available: bool,
    pub allow_insecure_local: bool,
    pub added_at: chrono::DateTime<chrono::Utc>,
}

impl SiteRecord {
    pub fn summary(&self) -> crate::wp::models::SiteSummary {
        crate::wp::models::SiteSummary {
            id: self.id,
            name: self.name.clone(),
            url: self.url.clone(),
            username: self.username.clone(),
            display_name: self.display_name.clone(),
            avatar_url: self.avatar_url.clone(),
            bridge_available: self.bridge_available,
        }
    }
}

pub struct SiteStore {
    path: PathBuf,
    sites: Mutex<Vec<SiteRecord>>,
}

impl SiteStore {
    pub fn load(path: PathBuf) -> Self {
        let sites = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            path,
            sites: Mutex::new(sites),
        }
    }

    pub fn list(&self) -> Vec<SiteRecord> {
        self.sites.lock().expect("site store poisoned").clone()
    }

    pub fn get(&self, id: &Uuid) -> AppResult<SiteRecord> {
        self.list()
            .into_iter()
            .find(|s| &s.id == id)
            .ok_or(AppError::UnknownSite)
    }

    /// Inserts or replaces (matched by URL + username so re-adding a site
    /// doesn't create duplicates).
    pub fn upsert(&self, mut record: SiteRecord) -> AppResult<SiteRecord> {
        let mut sites = self.sites.lock().expect("site store poisoned");
        if let Some(existing) = sites
            .iter_mut()
            .find(|s| s.url == record.url && s.username == record.username)
        {
            record.id = existing.id;
            record.added_at = existing.added_at;
            *existing = record.clone();
        } else {
            sites.push(record.clone());
        }
        self.persist(&sites)?;
        Ok(record)
    }

    pub fn remove(&self, id: &Uuid) -> AppResult<()> {
        let mut sites = self.sites.lock().expect("site store poisoned");
        sites.retain(|s| &s.id != id);
        self.persist(&sites)
    }

    fn persist(&self, sites: &[SiteRecord]) -> AppResult<()> {
        let io = |e: std::io::Error| AppError::Internal(format!("couldn't save site list: {e}"));
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(io)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        let bytes =
            serde_json::to_vec_pretty(sites).map_err(|e| AppError::Internal(e.to_string()))?;
        write_private(&tmp, &bytes).map_err(io)?;
        std::fs::rename(&tmp, &self.path).map_err(io)
    }
}

#[cfg(unix)]
fn write_private(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(bytes)?;
    f.sync_all()
}

#[cfg(not(unix))]
fn write_private(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    // The per-user app data directory on Windows is already ACL'd to the user.
    std::fs::write(path, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(url: &str, user: &str) -> SiteRecord {
        let url = Url::parse(url).unwrap();
        SiteRecord {
            id: Uuid::new_v4(),
            name: "Site".into(),
            api_root: url.join("wp-json/").unwrap(),
            admin_url: url.join("wp-admin/").unwrap(),
            url,
            username: user.into(),
            display_name: user.into(),
            avatar_url: None,
            app_password_uuid: None,
            bridge_available: false,
            allow_insecure_local: false,
            added_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn upsert_dedupes_and_persists() {
        let dir = std::env::temp_dir().join(format!("createpress-test-{}", Uuid::new_v4()));
        let path = dir.join("sites.json");
        let store = SiteStore::load(path.clone());
        let a = store.upsert(record("https://a.com/", "eli")).unwrap();
        let again = store.upsert(record("https://a.com/", "eli")).unwrap();
        assert_eq!(a.id, again.id);
        store.upsert(record("https://b.com/", "eli")).unwrap();
        assert_eq!(SiteStore::load(path).list().len(), 2);
        store.remove(&a.id).unwrap();
        assert_eq!(store.list().len(), 1);
        std::fs::remove_dir_all(dir).ok();
    }
}
