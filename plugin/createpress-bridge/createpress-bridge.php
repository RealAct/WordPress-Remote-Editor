<?php
/**
 * Plugin Name:       CreatePress Bridge
 * Plugin URI:        https://realact.net
 * Description:       Companion plugin for the CreatePress desktop app. Lets the app open the block editor without a second sign-in and blends the editor into the app.
 * Version:           0.1.0
 * Requires at least: 6.4
 * Requires PHP:      7.4
 * Author:            Eli Linares
 * Author URI:        https://realact.net
 * License:           GPL-2.0-or-later
 * License URI:       https://www.gnu.org/licenses/gpl-2.0.html
 * Text Domain:       createpress-bridge
 *
 * @package CreatePressBridge
 */

namespace CreatePressBridge;

defined( 'ABSPATH' ) || exit;

const VERSION        = '0.1.0';
const REST_NAMESPACE = 'createpress/v1';

require_once __DIR__ . '/includes/class-handoff.php';
require_once __DIR__ . '/includes/class-embedded-mode.php';

add_action( 'rest_api_init', array( Handoff::class, 'register_routes' ) );
add_action( 'login_form_createpress_handoff', array( Handoff::class, 'consume' ) );
Embedded_Mode::init();
