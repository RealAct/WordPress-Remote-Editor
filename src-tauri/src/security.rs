//! URL normalisation and origin rules shared by the REST client, the auth
//! window and the editor webview.

use crate::error::{AppError, AppResult};
use url::Url;

/// Hosts where plain HTTP is acceptable (local development only).
pub fn is_local_dev_host(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    host == "localhost"
        || host == "127.0.0.1"
        || host == "[::1]"
        || host == "::1"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host.ends_with(".test")
}

/// Turns what the user typed ("example.com", "https://example.com/blog/")
/// into a canonical site URL with no query, fragment or credentials.
pub fn normalize_site_url(input: &str, allow_insecure_local: bool) -> AppResult<Url> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.len() > 2048 {
        return Err(AppError::InvalidUrl);
    }
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    let mut url = Url::parse(&with_scheme).map_err(|_| AppError::InvalidUrl)?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err(AppError::InvalidUrl);
    }
    let host = url.host_str().ok_or(AppError::InvalidUrl)?.to_string();
    match url.scheme() {
        "https" => {}
        "http" if allow_insecure_local && is_local_dev_host(&host) => {}
        "http" => return Err(AppError::InsecureUrl),
        _ => return Err(AppError::InvalidUrl),
    }
    url.set_query(None);
    url.set_fragment(None);
    // Strip common admin paths people paste by accident.
    let mut path = url.path().trim_end_matches('/').to_string();
    for suffix in ["/wp-admin", "/wp-login.php", "/wp-json"] {
        if let Some(stripped) = path.strip_suffix(suffix) {
            path = stripped.to_string();
        }
    }
    url.set_path(&format!("{path}/"));
    Ok(url)
}

/// True when `candidate` has the same scheme, host and port as `origin`.
pub fn same_origin(origin: &Url, candidate: &Url) -> bool {
    origin.scheme() == candidate.scheme()
        && origin.host_str() == candidate.host_str()
        && origin.port_or_known_default() == candidate.port_or_known_default()
}

/// Rejects a URL handed to us by a server (e.g. the authorization endpoint or
/// the REST root) unless it stays on the same origin as the site and is not a
/// downgrade.
pub fn require_same_origin(site: &Url, candidate: &str) -> AppResult<Url> {
    let url = Url::parse(candidate).map_err(|_| AppError::BadResponse)?;
    if same_origin(site, &url) {
        return Ok(url);
    }
    // Allow http -> https upgrade on the same host.
    if site.scheme() == "http" && url.scheme() == "https" && site.host_str() == url.host_str() {
        return Ok(url);
    }
    Err(AppError::BadResponse)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_https_and_trailing_slash() {
        let u = normalize_site_url("example.com", false).unwrap();
        assert_eq!(u.as_str(), "https://example.com/");
    }

    #[test]
    fn strips_admin_paths_query_and_fragment() {
        let u = normalize_site_url("https://example.com/blog/wp-admin/?x=1#y", false).unwrap();
        assert_eq!(u.as_str(), "https://example.com/blog/");
    }

    #[test]
    fn rejects_plain_http_for_public_hosts() {
        assert!(matches!(
            normalize_site_url("http://example.com", true),
            Err(AppError::InsecureUrl)
        ));
    }

    #[test]
    fn allows_http_for_local_dev_only_when_opted_in() {
        assert!(normalize_site_url("http://mysite.test", true).is_ok());
        assert!(normalize_site_url("http://mysite.test", false).is_err());
    }

    #[test]
    fn rejects_credentials_and_odd_schemes() {
        assert!(normalize_site_url("https://user:pw@example.com", false).is_err());
        assert!(normalize_site_url("javascript:alert(1)", false).is_err());
        assert!(normalize_site_url("file:///etc/passwd", false).is_err());
    }

    #[test]
    fn origin_checks() {
        let site = Url::parse("https://example.com/").unwrap();
        assert!(require_same_origin(&site, "https://example.com/wp-json/").is_ok());
        assert!(require_same_origin(&site, "https://evil.com/wp-json/").is_err());
        assert!(require_same_origin(&site, "http://example.com/wp-json/").is_err());
        assert!(require_same_origin(&site, "https://example.com:8443/").is_err());
    }
}
