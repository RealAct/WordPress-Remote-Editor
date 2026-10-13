//! Application Passwords live only in the OS keychain:
//! macOS Keychain, Windows Credential Manager, or Secret Service on Linux.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

const SERVICE: &str = "com.wpremoteeditor.app";

#[derive(Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct Credentials {
    pub username: String,
    pub app_password: String,
}

impl Credentials {
    /// Application Passwords are shown with spaces for readability; WordPress
    /// accepts them either way, so store the compact form.
    pub fn new(username: &str, app_password: &str) -> Self {
        Self {
            username: username.trim().to_string(),
            app_password: app_password
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect(),
        }
    }

    /// `Basic` authorization header value.
    pub fn basic_auth_header(&self) -> Zeroizing<String> {
        use base64::Engine;
        let raw = Zeroizing::new(format!("{}:{}", self.username, self.app_password));
        Zeroizing::new(format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(raw.as_bytes())
        ))
    }
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("username", &self.username)
            .field("app_password", &"<redacted>")
            .finish()
    }
}

fn entry(site_id: &Uuid) -> AppResult<keyring::Entry> {
    Ok(keyring::Entry::new(SERVICE, &format!("site:{site_id}"))?)
}

pub fn save(site_id: &Uuid, creds: &Credentials) -> AppResult<()> {
    let json = Zeroizing::new(
        serde_json::to_string(creds).map_err(|e| AppError::Internal(e.to_string()))?,
    );
    entry(site_id)?.set_password(&json)?;
    Ok(())
}

pub fn load(site_id: &Uuid) -> AppResult<Credentials> {
    let json = Zeroizing::new(entry(site_id)?.get_password().map_err(|e| match e {
        keyring::Error::NoEntry => AppError::Unauthorized,
        other => other.into(),
    })?);
    serde_json::from_str(&json)
        .map_err(|_| AppError::Keychain("stored credentials are unreadable".into()))
}

pub fn delete(site_id: &Uuid) -> AppResult<()> {
    match entry(site_id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_spaces_from_application_passwords() {
        let c = Credentials::new(" eli ", "abcd EFGH ijkl MNOP qrst UVWX");
        assert_eq!(c.username, "eli");
        assert_eq!(c.app_password, "abcdEFGHijklMNOPqrstUVWX");
    }

    #[test]
    fn debug_output_redacts_password() {
        let c = Credentials::new("eli", "secret");
        assert!(!format!("{c:?}").contains("secret"));
    }

    #[test]
    fn builds_basic_auth_header() {
        let c = Credentials::new("eli", "pw");
        assert_eq!(c.basic_auth_header().as_str(), "Basic ZWxpOnB3");
    }
}
