//! Finds a site's REST API and Application Password authorization endpoint
//! the way WordPress documents it: via the `rel="https://api.w.org/"` link.

use super::client::{error_for, http_client, read_limited};
use super::models::{RestIndex, SiteDiscovery};
use crate::error::{AppError, AppResult};
use crate::security::{normalize_site_url, require_same_origin};
use reqwest::header::LINK;
use url::Url;

const API_REL: &str = "https://api.w.org/";

/// Extracts the REST API root from one or more `Link` header values.
pub fn api_root_from_link_headers<'a>(values: impl Iterator<Item = &'a str>) -> Option<String> {
    for value in values {
        for part in value.split(',') {
            let mut segments = part.split(';');
            let target = segments.next()?.trim();
            let is_api = segments.any(|s| {
                let s = s.trim();
                s == format!("rel=\"{API_REL}\"") || s == format!("rel={API_REL}")
            });
            if is_api {
                let t = target.trim_start_matches('<').trim_end_matches('>');
                return Some(t.to_string());
            }
        }
    }
    None
}

pub async fn discover(input: &str, allow_insecure_local: bool) -> AppResult<SiteDiscovery> {
    let entered = normalize_site_url(input, allow_insecure_local)?;
    let http = http_client(None, allow_insecure_local)?;

    // 1. Load the site and follow redirects (e.g. to www.) to learn its real address.
    let resp = http.get(entered.clone()).send().await?;
    let site_url = normalize_site_url(resp.url().as_str(), allow_insecure_local)?;
    let link = api_root_from_link_headers(
        resp.headers()
            .get_all(LINK)
            .iter()
            .filter_map(|v| v.to_str().ok()),
    );
    drop(resp);

    let api_root = match link {
        Some(l) => require_same_origin(&site_url, &l)?,
        None => site_url
            .join("wp-json/")
            .map_err(|_| AppError::InvalidUrl)?,
    };

    // 2. Read the REST index.
    let resp = http
        .get(api_root.clone())
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(match error_for(resp).await {
            AppError::NotFound => AppError::NotWordPress,
            e => e,
        });
    }
    let bytes = read_limited(resp).await?;
    let index: RestIndex = serde_json::from_slice(&bytes).map_err(|_| AppError::NotWordPress)?;
    if !index.namespaces.iter().any(|n| n == "wp/v2") {
        return Err(AppError::NotWordPress);
    }

    // `url` (siteurl) is where wp-admin lives and may differ from `home`.
    // It must use an acceptable scheme, but may be on a different host.
    let admin_base = if index.url.is_empty() {
        site_url.clone()
    } else {
        normalize_site_url(&index.url, allow_insecure_local)?
    };
    let admin_url = admin_base
        .join("wp-admin/")
        .map_err(|_| AppError::BadResponse)?;

    let authorization_endpoint = index
        .authentication
        .get("application-passwords")
        .and_then(|v| v.pointer("/endpoints/authorization"))
        .and_then(|v| v.as_str())
        .and_then(|s| Url::parse(s).ok())
        .filter(|u| u.scheme() == "https" || (allow_insecure_local && u.scheme() == "http"));

    Ok(SiteDiscovery {
        name: if index.name.is_empty() {
            site_url.host_str().unwrap_or_default().to_string()
        } else {
            index.name
        },
        description: index.description,
        site_url,
        admin_url,
        api_root,
        supports_application_passwords: authorization_endpoint.is_some(),
        authorization_endpoint,
        bridge_available: index.namespaces.iter().any(|n| n == "wpre/v1"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_api_link_among_several() {
        let headers = [
            "<https://example.com/wp-json/wp/v2/pages/2>; rel=\"alternate\"; type=\"application/json\"",
            "<https://example.com/wp-json/>; rel=\"https://api.w.org/\"",
        ];
        assert_eq!(
            api_root_from_link_headers(headers.iter().copied()).as_deref(),
            Some("https://example.com/wp-json/")
        );
    }

    #[test]
    fn handles_comma_joined_links_and_plain_permalinks() {
        let headers = ["<https://e.com/?p=1>; rel=shortlink, <https://e.com/?rest_route=/>; rel=\"https://api.w.org/\""];
        assert_eq!(
            api_root_from_link_headers(headers.iter().copied()).as_deref(),
            Some("https://e.com/?rest_route=/")
        );
    }

    #[test]
    fn none_when_absent() {
        assert!(api_root_from_link_headers(["<x>; rel=\"next\""].iter().copied()).is_none());
    }
}
