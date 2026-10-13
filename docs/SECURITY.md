# Security Model

WordPress Remote Editor holds credentials for websites and renders remote content, so it is designed around least privilege and isolation.

## Credentials

- Only **Application Passwords** are used (never the user's main WordPress password). They can be revoked per app from the WordPress profile screen, and the app revokes its own when a site is removed.
- Passwords are stored only in the **OS keychain** (macOS Keychain, Windows Credential Manager, Secret Service/libsecret on Linux), never on disk in plain text, never in logs, never in the shell UI.
- In memory, secrets live in zeroizing buffers and are redacted from debug output.
- Non-secret site metadata (`sites.json`) is written atomically with `0600` permissions on macOS/Linux.

## Network

- HTTPS only. Plain HTTP is allowed solely for `localhost`, `*.local`, `*.test` and `*.localhost`, and only when the user ticks "local development site".
- TLS via rustls with the operating system's trust store.
- Authenticated requests never follow redirects to another origin, so credentials can't be forwarded to a third party. Discovery (unauthenticated) may follow HTTPS redirects, e.g. `example.com` → `www.example.com`.
- URLs supplied by the server (REST root, editor handoff) must be on the site's own origin.
- Response bodies are capped at 16 MB; timeouts are enforced.

## Sign-in

- Authorization happens on the site's own `authorize-application.php` page, so 2FA, SSO and security plugins work unchanged.
- The sign-in window is incognito (no persistent cookies) and has no IPC access.
- The callback (`wpremoteeditor://…`) is intercepted inside that window, not registered with the OS, so another app cannot hijack it. A 256-bit `state` value is checked in constant time.

## Webviews

| Webview | Loads | IPC |
| --- | --- | --- |
| Shell (`main`) | Bundled assets only, strict CSP, prototype freezing | App commands listed in `capabilities/main.json`; cannot call `editor_report` |
| Sign-in (`auth`) | The site's authorization page | None |
| Editor (`editor`) | The site's own wp-admin block editor | Only `editor_report`, granted at runtime for that site's origins; payload is strictly validated |

- The editor webview's navigation is locked to the site's origins; other links open in the system browser.
- Each site has its own cookie/data store, deleted when the site is removed.
- The core controls the editor only by evaluating fixed, built-in scripts. It never evaluates strings that come from a page.

## Companion plugin (`remote-editor-bridge`)

- Handoff links can only be requested over REST with Application Password authentication by users who can edit content.
- Tokens are 48 random characters, single use, valid for 60 seconds, stored only as an HMAC-SHA256 hash, rate limited to 10/minute per user, and can only redirect to `post.php`/`post-new.php`.
- The standard `wp_login` action fires so audit/security plugins see the sign-in.
- Passes WordPress Coding Standards (`phpcs`), escapes all output, and stores nothing permanently.

## Releases

- macOS: Developer ID signed, hardened runtime, notarized.
- Windows: Authenticode-signed `.msi` (configure `bundle.windows.certificateThumbprint` or `signCommand`, e.g. Azure Trusted Signing).
- Linux: `.deb` with a detached signature.
- Auto-updates (when enabled) are verified with a minisign public key embedded in the app.

## Reporting a vulnerability

Please report security issues privately through GitHub Security Advisories on this repository rather than opening a public issue.
