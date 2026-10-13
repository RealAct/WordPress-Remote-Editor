# WordPress Remote Editor

A native desktop app for macOS, Windows and Linux that lets you write and publish posts and pages on any WordPress site, using the site's full block editor (Gutenberg), with every block plugin installed on that site working exactly as it does in wp-admin.

- **Native shell**: real native menus and shortcuts, macOS vibrancy and traffic lights, Windows 11 Mica, GNOME/KDE-friendly styling, OS light/dark mode.
- **Full block compatibility**: the editor pane runs the site's own block editor, so third-party blocks, sidebar plugins and meta boxes all work.
- **Role-aware**: only offers the content types and actions your WordPress role allows (for example, contributors get *Submit for Review* instead of *Publish*).
- **Secure by design**: Application Passwords only, stored in the OS keychain; HTTPS enforced; isolated, origin-locked editor webview. See [docs/SECURITY.md](docs/SECURITY.md).
- **Installers**: `.dmg` (macOS, universal), `.deb` (Linux, plus AppImage), `.msi` (Windows).

Read [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the design and the reasoning behind it.

## Requirements

On the WordPress site:

- WordPress 6.4 or newer, served over HTTPS.
- Application Passwords enabled (the default in WordPress core; some security plugins or hosts turn them off).
- Optional but recommended: the **Remote Editor Bridge** plugin from [`plugin/remote-editor-bridge`](plugin/remote-editor-bridge), which removes the second sign-in when opening the editor.

On your computer: macOS 14+, Windows 10/11 (WebView2 is installed automatically if missing), or a Debian/Ubuntu-based Linux with WebKitGTK 4.1.

## Development

Prerequisites: Node 22+, Rust (stable), and the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your OS. On Debian/Ubuntu:

```sh
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev libdbus-1-dev
```

```sh
npm install
npm run tauri dev        # run the app with hot reload
npm test                 # frontend tests
cd src-tauri && cargo test   # Rust tests
```

Live tests against a real site (for example a local one on `http://localhost:8888`):

```sh
cd src-tauri
WPRE_TEST_SITE=https://example.com WPRE_TEST_USER=me WPRE_TEST_APP_PASSWORD="xxxx xxxx xxxx xxxx xxxx xxxx" \
  cargo test live_tests -- --ignored --nocapture
```

Companion plugin checks:

```sh
cd plugin/remote-editor-bridge
composer install
vendor/bin/phpcs
```

## Building installers

```sh
npm run tauri build -- --bundles deb,appimage                     # on Linux
npm run tauri build -- --target universal-apple-darwin --bundles dmg   # on macOS
npm run tauri build -- --bundles msi                              # on Windows
```

Pushing a `v*` tag runs [`.github/workflows/release.yml`](.github/workflows/release.yml), which builds, signs and notarizes all three on GitHub's macOS, Ubuntu and Windows runners and attaches them to a draft release, along with a zip of the companion plugin. Signing secrets are listed in the workflow file.

## Project layout

| Path | What it is |
| --- | --- |
| `src/` | Platform-adaptive React shell UI |
| `src-tauri/src/` | Rust core: REST client, sign-in, keychain, editor webview, menus |
| `src-tauri/capabilities/` | Least-privilege permission grants for each webview |
| `plugin/remote-editor-bridge/` | Optional companion WordPress plugin |
| `docs/` | Architecture and security notes |

## Name and trademark

"WordPress" is a registered trademark of the WordPress Foundation. Its trademark policy restricts using "WordPress" in product names, so the app name should be cleared (or changed to something like "Remote Editor for WordPress") before public distribution. The companion plugin already uses a compliant name.

## License

GPL-2.0-or-later, the same license as WordPress.
