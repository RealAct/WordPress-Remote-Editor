<?php
/**
 * Blends the block editor into the desktop app when it is opened there.
 *
 * The app also injects equivalent styles itself, so this is a progressive
 * enhancement that avoids a flash of wp-admin chrome while loading.
 *
 * @package RemoteEditorBridge
 */

namespace RemoteEditorBridge;

defined( 'ABSPATH' ) || exit;

/**
 * Embedded-mode styling.
 */
final class Embedded_Mode {

	/**
	 * Hooks into wp-admin.
	 */
	public static function init() {
		add_filter( 'admin_body_class', array( self::class, 'body_class' ) );
		add_action( 'admin_enqueue_scripts', array( self::class, 'enqueue' ) );
	}

	/**
	 * Whether this admin request comes from the desktop app's editor.
	 *
	 * @return bool
	 */
	public static function is_embedded() {
		return isset( $_COOKIE[ Handoff::EMBED_COOKIE ] ) && '1' === $_COOKIE[ Handoff::EMBED_COOKIE ];
	}

	/**
	 * Adds a body class for embedded requests.
	 *
	 * @param string $classes Space-separated classes.
	 * @return string
	 */
	public static function body_class( $classes ) {
		return self::is_embedded() ? $classes . ' wpre-embedded' : $classes;
	}

	/**
	 * Loads the embedded stylesheet on block editor screens only.
	 */
	public static function enqueue() {
		if ( ! self::is_embedded() ) {
			return;
		}
		$screen = get_current_screen();
		if ( ! $screen || ! $screen->is_block_editor() ) {
			return;
		}
		wp_enqueue_style(
			'remote-editor-bridge-embedded',
			plugins_url( 'assets/embedded.css', __DIR__ ),
			array(),
			VERSION
		);
	}
}
