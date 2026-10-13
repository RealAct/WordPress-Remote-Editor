<?php
/**
 * Cleans up on uninstall. Handoff tokens are short-lived transients, so there
 * is no persistent data to remove beyond any that remain.
 *
 * @package RemoteEditorBridge
 */

defined( 'WP_UNINSTALL_PLUGIN' ) || exit;

global $wpdb;
// phpcs:ignore WordPress.DB.DirectDatabaseQuery.DirectQuery, WordPress.DB.DirectDatabaseQuery.NoCaching -- One-off cleanup of this plugin's transients.
$wpdb->query(
	$wpdb->prepare(
		"DELETE FROM {$wpdb->options} WHERE option_name LIKE %s OR option_name LIKE %s",
		$wpdb->esc_like( '_transient_wpre_handoff_' ) . '%',
		$wpdb->esc_like( '_transient_timeout_wpre_handoff_' ) . '%'
	)
);
