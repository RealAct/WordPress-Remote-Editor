=== CreatePress Bridge ===
Contributors: realact
Tags: block editor, gutenberg, desktop, application passwords, remote editing
Requires at least: 6.4
Tested up to: 6.8
Requires PHP: 7.4
Stable tag: 0.1.0
License: GPL-2.0-or-later
License URI: https://www.gnu.org/licenses/gpl-2.0.html

Companion plugin for the CreatePress desktop app.

== Description ==

CreatePress is a desktop app for macOS, Windows and Linux that edits your site with its own block editor, so every block plugin you use keeps working.

This optional plugin improves that experience:

* **No second sign-in.** The app signs in to the REST API with an Application Password. This plugin lets it exchange that for a single-use, 60-second sign-in link when opening the editor.
* **Native look.** The wp-admin bar and menu are hidden inside the app so the editor blends in.

The plugin adds no settings, stores nothing permanently, and changes nothing for normal wp-admin visits.

= Security =

* Sign-in links can only be requested with Application Password authentication, by users who can edit posts or pages.
* Each link works once, expires after 60 seconds, is stored only as an HMAC hash, and can only redirect to the block editor screens.
* Requests are rate limited per user.
* The standard `wp_login` action fires, so audit and security plugins see the sign-in.

== Changelog ==

= 0.1.0 =
* First release.
