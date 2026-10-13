<?php
/**
 * Plugin Name:       Remote Editor Bridge
 * Plugin URI:        https://github.com/RealAct/wordpress-remote-editor
 * Description:       Companion plugin for the WordPress Remote Editor desktop app. Lets the app open the block editor without a second sign-in and blends the editor into the app.
 * Version:           0.1.0
 * Requires at least: 6.4
 * Requires PHP:      7.4
 * Author:            WordPress Remote Editor contributors
 * License:           GPL-2.0-or-later
 * License URI:       https://www.gnu.org/licenses/gpl-2.0.html
 * Text Domain:       remote-editor-bridge
 *
 * @package RemoteEditorBridge
 */

namespace RemoteEditorBridge;

defined( 'ABSPATH' ) || exit;

const VERSION        = '0.1.0';
const REST_NAMESPACE = 'wpre/v1';

require_once __DIR__ . '/includes/class-handoff.php';
require_once __DIR__ . '/includes/class-embedded-mode.php';

add_action( 'rest_api_init', array( Handoff::class, 'register_routes' ) );
add_action( 'login_form_wpre_handoff', array( Handoff::class, 'consume' ) );
Embedded_Mode::init();
