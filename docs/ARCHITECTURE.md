# WordPress Remote Editor: Architecture

## Goals

1. Create and edit posts, pages and any other REST-enabled post type on a remote WordPress site, limited to exactly what the signed-in user's role allows.
2. Run the full Gutenberg block editor, including blocks, sidebars and meta boxes added by any plugin installed on that site.
3. Install natively on macOS (`.dmg`), Linux (`.deb`) and Windows (`.msi`) and feel like a native app on each.
4. Follow WordPress standards (REST API, Application Passwords, capabilities, coding standards, GPL) and current desktop security practice.

## The key design decision: where Gutenberg runs

Gutenberg is a JavaScript application, and every block plugin ships JavaScript that is written against the exact versions of `wp.blocks`, `wp.blockEditor`, `wp.data`, React and so on that the site is running. Plugins can also add PHP-rendered meta boxes (ACF, Yoast, WooCommerce) and enqueue arbitrary editor assets.

Three ways to get Gutenberg into a desktop app were considered:

| Approach | Plugin compatibility | Notes |
| --- | --- | --- |
| Bundle our own copy of `@wordpress/*` packages and render blocks locally | Poor | Version skew with the site; plugin block scripts can't be loaded reliably; PHP meta boxes impossible. |
| Load the site's editor scripts into a local shell page | Partial | Needs a server-side asset manifest; meta boxes and many `admin_enqueue_scripts` integrations still break. |
| **Load the site's own block editor screen in an isolated, hardened webview inside a native shell** | **Complete** | The site runs its real editor, so every block, sidebar plugin and meta box works exactly as in wp-admin. |

We use the third approach. It is the only one that can honour "fully compatible with any kind of block plugin". Everything *around* the editor canvas is native: windows, menus, keyboard shortcuts, site and content browser, toolbar, dialogs, notifications, credential storage. The wp-admin chrome (admin bar, admin menu, the "W" back button) is hidden so the editor reads as part of the app, not as a website.

## Framework: Tauri 2 (Rust core + system webview)

| Option | Why not / why |
| --- | --- |
| Electron | Ships a whole Chromium (~150 MB), larger attack surface, looks the same everywhere. |
| Three native apps (SwiftUI, WinUI, GTK) | Most native, but three codebases and we still need a webview for Gutenberg. |
| Qt / Flutter | Custom-drawn widgets, not truly native; still needs a webview. |
| **Tauri 2** | Uses each OS's own webview (WebKit on macOS, WebView2 on Windows, WebKitGTK on Linux), real native menus, dialogs, window effects (macOS vibrancy, Windows Mica), ~10 MB installers, a Rust core with a capability-based permission model, and first-party `.dmg`/`.deb`/`.msi` bundling. |

### Native feel per platform

The shell UI is React, but it is *platform-adaptive*, not a single web look:

- **macOS**: overlay title bar with traffic lights, translucent sidebar (`sidebar` vibrancy), SF Pro, macOS control metrics, ⌘ shortcuts, standard app menu.
- **Windows 11**: Mica backdrop, Segoe UI Variable, Fluent spacing and focus visuals, Ctrl shortcuts, native title bar.
- **Linux (GNOME/KDE)**: native decorations, libadwaita-like header and list styling, system font (Cantarell/Noto).
- All platforms follow the OS light/dark setting and accent colour, use native file dialogs, native notifications and the OS keychain.

## Process and trust model

```
┌──────────────────────── Native window ─────────────────────────┐
│ Shell webview (trusted, bundled assets only)                    │
│  React UI: sites · post lists · toolbar      ── IPC ──┐         │
│                                                       ▼         │
│                                  Rust core (all network + secrets)
│                                   · WordPress REST client       │
│                                   · OS keychain                 │
│                                   · window/webview manager      │
│ ┌───────────────── Editor child webview (untrusted) ──────────┐ │
│ │ https://site/wp-admin/post.php?post=…  (site's real Gutenberg)│ │
│ │ · navigation locked to the site origin                      │ │
│ │ · per-site isolated cookie store                            │ │
│ │ · IPC limited to one command: editor_report                 │ │
│ └─────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

- **Shell webview**: loads only bundled assets, strict CSP, never talks to WordPress directly and never sees a password.
- **Rust core**: the only component that holds credentials or performs authenticated HTTP. HTTPS-only (plain HTTP allowed solely for `localhost`/`.local`/`.test` development sites after explicit opt-in), rustls with the OS trust store, redirects restricted to the same origin, response size limits.
- **Editor webview**: treated as untrusted remote content. It gets a runtime Tauri capability scoped to *that site's origin and that webview label* allowing only `editor_report` (dirty/saving/status/title state for the native toolbar). Native → editor control (save, publish, preview) is done by the core evaluating a fixed script; nothing from the page is ever evaluated by the core.

## Authentication

WordPress **Application Passwords** (core since 5.6) are used for all REST traffic.

1. **Discovery**: fetch the site, follow the `Link: <…>; rel="https://api.w.org/"` header to the REST index, read `authentication["application-passwords"].endpoints.authorization`. Works with pretty and plain permalinks (`?rest_route=`).
2. **Authorise**: open WordPress's own `authorize-application.php` in an isolated, incognito auth window. The user signs in with their normal login (2FA, SSO and security plugins keep working) and approves "WordPress Remote Editor". `success_url` is `wpremoteeditor://auth/callback?state=…`; the core intercepts that navigation inside its own webview (so no other app can register the scheme and steal it), checks `state`, and stores the password.
3. **Manual fallback**: paste a username and an Application Password created under Users → Profile.
4. **Storage**: the password goes straight into the OS keychain (macOS Keychain, Windows Credential Manager, Secret Service/libsecret on Linux) and is held in memory only in zeroizing buffers.
5. **Removal**: removing a site revokes its Application Password on the server (using `/application-passwords/introspect` to find its UUID) and deletes the keychain entry.

### Signing the editor webview in

wp-admin uses cookie authentication, which Application Passwords can't provide. Two modes:

- **With the companion plugin** (`plugin/wp-remote-editor-bridge`, recommended): the core calls `POST /wpre/v1/handoff` authenticated by the Application Password. The plugin returns a single-use, 60-second, hashed token bound to that user; the editor webview opens `wp-login.php?action=wpre_handoff&token=…`, which sets the normal auth cookie and redirects into the editor. No second login.
- **Without the plugin**: the user signs in once inside the editor webview using the site's own login form. The cookie lives in that site's isolated store.

## Roles and capabilities

- `GET /wp/v2/users/me?context=edit` gives the user's roles and full capability map.
- `GET /wp/v2/types?context=edit` gives each post type's capability names (`edit_posts`, `publish_posts`, `create_posts`, …).
- The shell only offers post types the user can create, shows *Publish* vs *Submit for Review* according to `publish_posts`, and hides actions the user can't perform. WordPress still enforces everything server-side; the UI gating is for clarity, not security.

## Repository layout

```
src/                     React shell (platform-adaptive UI)
src-tauri/               Rust core: REST client, auth, keychain, editor webview
  capabilities/          Tauri capability files (least privilege)
plugin/wp-remote-editor-bridge/   Optional companion WordPress plugin (GPL-2.0-or-later, WPCS)
.github/workflows/       CI (lint, test, phpcs) and signed release builds
docs/                    Architecture and security notes
```

## Packaging and distribution

| OS | Artifact | Signing |
| --- | --- | --- |
| macOS | `.dmg` (universal: Apple Silicon + Intel) | Developer ID signing + Apple notarization, hardened runtime |
| Linux | `.deb` (plus AppImage as a bonus) | Detached GPG signature; desktop entry registers the URL scheme |
| Windows | **`.msi`** (WiX) | Authenticode signing |

`.msi` is chosen over `.exe` (NSIS) because it is what IT departments deploy with Intune/Group Policy, supports clean upgrades and repair, and is equally easy for home users. WebView2 is bootstrapped by the installer if missing.

Builds run on GitHub Actions on real macOS, Ubuntu and Windows runners via `tauri-action`. Auto-update uses Tauri's updater with minisign-signed update manifests (added once a release channel exists).

## Roadmap

1. **Scaffold (this commit)**: shell, site onboarding, keychain, REST client, capability gating, embedded editor, companion plugin, packaging config, CI.
2. Media library browser with native drag-and-drop upload.
3. Offline drafts queued locally (encrypted at rest) and synced on reconnect.
4. Multi-site switching with per-site windows, notifications for scheduled posts.
5. Auto-update channel, crash reporting (opt-in).
