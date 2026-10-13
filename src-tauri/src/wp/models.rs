//! WordPress REST API shapes (only the fields the app uses) and the
//! camelCase DTOs sent to the shell UI.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;

// ---- REST responses ------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct RestIndex {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// `siteurl`: where wp-admin lives.
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub namespaces: Vec<String>,
    #[serde(default)]
    pub authentication: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct RestUser {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub avatar_urls: HashMap<String, String>,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub capabilities: HashMap<String, bool>,
}

#[derive(Debug, Deserialize)]
pub struct RestPostType {
    pub slug: String,
    #[serde(default)]
    pub name: String,
    pub rest_base: String,
    #[serde(default)]
    pub rest_namespace: Option<String>,
    #[serde(default)]
    pub viewable: bool,
    #[serde(default)]
    pub hierarchical: bool,
    #[serde(default)]
    pub capabilities: HashMap<String, String>,
    #[serde(default)]
    pub labels: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct RestRendered {
    #[serde(default)]
    pub raw: Option<String>,
    #[serde(default)]
    pub rendered: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RestPost {
    pub id: u64,
    #[serde(default)]
    pub title: Option<RestRendered>,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub modified: Option<String>,
    #[serde(default)]
    pub author: Option<u64>,
    #[serde(default)]
    pub link: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RestErrorBody {
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub message: String,
}

// ---- DTOs for the shell UI -----------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteDiscovery {
    pub site_url: Url,
    pub admin_url: Url,
    pub api_root: Url,
    pub name: String,
    pub description: String,
    pub supports_application_passwords: bool,
    #[serde(skip)]
    pub authorization_endpoint: Option<Url>,
    pub bridge_available: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteSummary {
    pub id: uuid::Uuid,
    pub name: String,
    pub url: Url,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<Url>,
    pub bridge_available: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostTypeAccess {
    pub slug: String,
    pub rest_base: String,
    pub name: String,
    pub singular_name: String,
    pub hierarchical: bool,
    pub can_create: bool,
    pub can_publish: bool,
    pub can_edit_others: bool,
    pub can_delete: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteContext {
    pub site: SiteSummary,
    pub user_id: u64,
    pub roles: Vec<String>,
    pub post_types: Vec<PostTypeAccess>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostSummary {
    pub id: u64,
    pub title: String,
    pub status: String,
    pub modified: Option<String>,
    pub author: Option<u64>,
    pub link: Option<String>,
    pub can_edit: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostPage {
    pub items: Vec<PostSummary>,
    pub total: u64,
    pub total_pages: u64,
    pub page: u32,
}

/// Works out what the user may do with a post type from their capability map
/// and the post type's capability names. WordPress enforces all of this on
/// the server too; this only decides what the UI offers.
pub fn post_type_access(t: &RestPostType, caps: &HashMap<String, bool>) -> PostTypeAccess {
    let has = |key: &str, fallback: &str| {
        let cap = t
            .capabilities
            .get(key)
            .map(String::as_str)
            .unwrap_or(fallback);
        caps.get(cap).copied().unwrap_or(false)
    };
    let singular = t
        .labels
        .get("singular_name")
        .and_then(|v| v.as_str())
        .unwrap_or(&t.name)
        .to_string();
    PostTypeAccess {
        slug: t.slug.clone(),
        rest_base: t.rest_base.clone(),
        name: t.name.clone(),
        singular_name: singular,
        hierarchical: t.hierarchical,
        can_create: has("create_posts", "edit_posts") && has("edit_posts", "edit_posts"),
        can_publish: has("publish_posts", "publish_posts"),
        can_edit_others: has("edit_others_posts", "edit_others_posts"),
        can_delete: has("delete_posts", "delete_posts"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page_type() -> RestPostType {
        serde_json::from_value(serde_json::json!({
            "slug": "page", "name": "Pages", "rest_base": "pages", "viewable": true,
            "hierarchical": true,
            "capabilities": {
                "edit_posts": "edit_pages", "create_posts": "edit_pages",
                "publish_posts": "publish_pages", "edit_others_posts": "edit_others_pages",
                "delete_posts": "delete_pages"
            },
            "labels": { "singular_name": "Page" }
        }))
        .unwrap()
    }

    #[test]
    fn contributor_cannot_create_pages() {
        let caps = HashMap::from([("edit_posts".to_string(), true)]);
        let a = post_type_access(&page_type(), &caps);
        assert!(!a.can_create);
        assert!(!a.can_publish);
    }

    #[test]
    fn editor_can_do_everything_with_pages() {
        let caps: HashMap<String, bool> = [
            "edit_pages",
            "publish_pages",
            "edit_others_pages",
            "delete_pages",
        ]
        .into_iter()
        .map(|c| (c.to_string(), true))
        .collect();
        let a = post_type_access(&page_type(), &caps);
        assert!(a.can_create && a.can_publish && a.can_edit_others && a.can_delete);
        assert_eq!(a.singular_name, "Page");
    }

    #[test]
    fn false_capabilities_are_respected() {
        let caps = HashMap::from([
            ("edit_pages".to_string(), true),
            ("publish_pages".to_string(), false),
        ]);
        let a = post_type_access(&page_type(), &caps);
        assert!(a.can_create);
        assert!(!a.can_publish);
    }
}
