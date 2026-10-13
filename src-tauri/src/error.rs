use serde::{Serialize, Serializer};

/// Errors returned to the shell UI. Messages are written for end users and
/// never include credentials, tokens or raw response bodies.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("That doesn't look like a valid site address.")]
    InvalidUrl,
    #[error("This site must use HTTPS. Plain HTTP is only allowed for local development sites.")]
    InsecureUrl,
    #[error("Couldn't find the WordPress REST API on this site.")]
    NotWordPress,
    #[error("This site has Application Passwords turned off, so the app can't sign in. Ask the site administrator to enable them.")]
    ApplicationPasswordsUnavailable,
    #[error("WordPress rejected the username or application password.")]
    Unauthorized,
    #[error("Your WordPress role doesn't allow this action.")]
    Forbidden,
    #[error("The item no longer exists on the site.")]
    NotFound,
    #[error("WordPress returned an error: {0}")]
    WordPress(String),
    #[error("Couldn't reach the site: {0}")]
    Network(String),
    #[error("The site sent a response that was too large or malformed.")]
    BadResponse,
    #[error("Couldn't access the system keychain: {0}")]
    Keychain(String),
    #[error("Unknown site.")]
    UnknownSite,
    #[error("Sign-in was cancelled or didn't match the request.")]
    AuthFlow,
    #[error("{0}")]
    Internal(String),
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        // Strip the URL so query strings never reach logs or the UI.
        let e = e.without_url();
        if e.is_timeout() {
            AppError::Network("the connection timed out".into())
        } else if e.is_decode() {
            AppError::BadResponse
        } else {
            AppError::Network(e.to_string())
        }
    }
}

impl From<keyring::Error> for AppError {
    fn from(e: keyring::Error) -> Self {
        AppError::Keychain(e.to_string())
    }
}

impl From<tauri::Error> for AppError {
    fn from(e: tauri::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
