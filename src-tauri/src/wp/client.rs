//! Authenticated WordPress REST client. All requests go through the Rust core;
//! the shell webview never sees credentials or talks to the site directly.

use crate::error::{AppError, AppResult};
use crate::keychain::Credentials;
use crate::security::{is_local_dev_host, same_origin};
use reqwest::header::{HeaderMap, ACCEPT, AUTHORIZATION};
use reqwest::{Method, Response, StatusCode};
use serde::de::DeserializeOwned;
use std::time::Duration;
use url::Url;
use zeroize::Zeroizing;

pub const USER_AGENT: &str = concat!("WordPressRemoteEditor/", env!("CARGO_PKG_VERSION"));
const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const MAX_REDIRECTS: usize = 5;

/// Builds an HTTP client. `pin_origin` restricts redirects to that origin
/// (used for authenticated clients); `None` allows HTTPS redirects anywhere
/// (used for unauthenticated discovery, e.g. example.com -> www.example.com).
pub fn http_client(
    pin_origin: Option<Url>,
    allow_insecure_local: bool,
) -> AppResult<reqwest::Client> {
    let policy = reqwest::redirect::Policy::custom(move |attempt| {
        if attempt.previous().len() >= MAX_REDIRECTS {
            return attempt.error("too many redirects");
        }
        let next = attempt.url();
        let ok_scheme = next.scheme() == "https"
            || (allow_insecure_local
                && next.scheme() == "http"
                && next.host_str().is_some_and(is_local_dev_host));
        if !ok_scheme {
            return attempt.stop();
        }
        match &pin_origin {
            Some(origin) if !same_origin(origin, next) => attempt.stop(),
            _ => attempt.follow(),
        }
    });
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .https_only(!allow_insecure_local)
        .redirect(policy)
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(AppError::from)
}

pub async fn read_limited(mut resp: Response) -> AppResult<Vec<u8>> {
    if resp
        .content_length()
        .is_some_and(|l| l as usize > MAX_RESPONSE_BYTES)
    {
        return Err(AppError::BadResponse);
    }
    let mut buf = Vec::new();
    while let Some(chunk) = resp.chunk().await? {
        if buf.len() + chunk.len() > MAX_RESPONSE_BYTES {
            return Err(AppError::BadResponse);
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(buf)
}

/// Maps a non-success response to a user-facing error, using WordPress's
/// `{ code, message }` error body when present.
pub async fn error_for(resp: Response) -> AppError {
    let status = resp.status();
    let body = read_limited(resp).await.unwrap_or_default();
    let wp: Option<super::models::RestErrorBody> = serde_json::from_slice(&body).ok();
    match status {
        StatusCode::UNAUTHORIZED => AppError::Unauthorized,
        StatusCode::FORBIDDEN => AppError::Forbidden,
        StatusCode::NOT_FOUND | StatusCode::GONE => AppError::NotFound,
        _ => match wp {
            Some(e) if !e.message.is_empty() => {
                let mut msg: String = strip_tags(&e.message).chars().take(300).collect();
                if msg.is_empty() {
                    msg = e.code;
                }
                AppError::WordPress(msg)
            }
            _ => AppError::WordPress(format!("HTTP {}", status.as_u16())),
        },
    }
}

fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

/// Builds a REST URL for `route` (e.g. `wp/v2/posts`) under `api_root`,
/// supporting both pretty permalinks (`/wp-json/`) and plain permalinks
/// (`/?rest_route=/`).
pub fn route_url(api_root: &Url, route: &str, query: &[(&str, String)]) -> AppResult<Url> {
    let route = route.trim_start_matches('/');
    let uses_rest_route = api_root.query_pairs().any(|(k, _)| k == "rest_route");
    let mut url = if uses_rest_route {
        let mut base = api_root.clone();
        base.set_query(None);
        base.query_pairs_mut()
            .append_pair("rest_route", &format!("/{route}"));
        base
    } else {
        let mut root = api_root.clone();
        if !root.path().ends_with('/') {
            root.set_path(&format!("{}/", root.path()));
        }
        root.join(route).map_err(|_| AppError::InvalidUrl)?
    };
    if !query.is_empty() {
        let mut pairs = url.query_pairs_mut();
        for (k, v) in query {
            pairs.append_pair(k, v);
        }
    }
    Ok(url)
}

pub struct WpClient {
    http: reqwest::Client,
    api_root: Url,
    auth: Zeroizing<String>,
}

impl WpClient {
    pub fn new(api_root: Url, creds: &Credentials, allow_insecure_local: bool) -> AppResult<Self> {
        Ok(Self {
            http: http_client(Some(api_root.clone()), allow_insecure_local)?,
            auth: creds.basic_auth_header(),
            api_root,
        })
    }

    async fn send(
        &self,
        method: Method,
        route: &str,
        query: &[(&str, String)],
        body: Option<&serde_json::Value>,
    ) -> AppResult<(Vec<u8>, HeaderMap)> {
        let url = route_url(&self.api_root, route, query)?;
        let mut req = self
            .http
            .request(method, url)
            .header(AUTHORIZATION, self.auth.as_str())
            .header(ACCEPT, "application/json");
        if let Some(b) = body {
            req = req.json(b);
        }
        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(error_for(resp).await);
        }
        let headers = resp.headers().clone();
        Ok((read_limited(resp).await?, headers))
    }

    pub async fn get<T: DeserializeOwned>(
        &self,
        route: &str,
        query: &[(&str, String)],
    ) -> AppResult<(T, HeaderMap)> {
        let (bytes, headers) = self.send(Method::GET, route, query, None).await?;
        let value = serde_json::from_slice(&bytes).map_err(|_| AppError::BadResponse)?;
        Ok((value, headers))
    }

    pub async fn post<T: DeserializeOwned>(
        &self,
        route: &str,
        body: &serde_json::Value,
    ) -> AppResult<T> {
        let (bytes, _) = self.send(Method::POST, route, &[], Some(body)).await?;
        serde_json::from_slice(&bytes).map_err(|_| AppError::BadResponse)
    }

    pub async fn delete(&self, route: &str, query: &[(&str, String)]) -> AppResult<()> {
        self.send(Method::DELETE, route, query, None)
            .await
            .map(|_| ())
    }
}

pub fn header_u64(headers: &HeaderMap, name: &str) -> u64 {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pretty_permalink_routes() {
        let root = Url::parse("https://example.com/wp-json/").unwrap();
        let u = route_url(&root, "wp/v2/posts", &[("context", "edit".into())]).unwrap();
        assert_eq!(
            u.as_str(),
            "https://example.com/wp-json/wp/v2/posts?context=edit"
        );
    }

    #[test]
    fn plain_permalink_routes() {
        let root = Url::parse("https://example.com/?rest_route=/").unwrap();
        let u = route_url(&root, "/wp/v2/posts", &[("page", "2".into())]).unwrap();
        assert_eq!(
            u.as_str(),
            "https://example.com/?rest_route=%2Fwp%2Fv2%2Fposts&page=2"
        );
    }

    #[test]
    fn query_values_are_encoded() {
        let root = Url::parse("https://example.com/wp-json/").unwrap();
        let u = route_url(&root, "wp/v2/posts", &[("search", "a&b=c".into())]).unwrap();
        assert!(u.as_str().ends_with("search=a%26b%3Dc"));
    }

    #[test]
    fn strips_html_from_error_messages() {
        assert_eq!(strip_tags("<strong>Error:</strong> nope"), "Error: nope");
    }
}
