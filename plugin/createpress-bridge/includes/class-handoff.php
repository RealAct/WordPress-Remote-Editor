<?php
/**
 * Single-use sign-in handoff from the desktop app to wp-admin.
 *
 * The app authenticates to REST with an Application Password and asks for a
 * handoff link. The link carries a random token that is valid once, for 60
 * seconds, for that user only. Opening it in the app's editor webview sets the
 * normal WordPress auth cookie and redirects to the block editor.
 *
 * @package CreatePressBridge
 */

namespace CreatePressBridge;

defined( 'ABSPATH' ) || exit;

/**
 * Issues and consumes handoff tokens.
 */
final class Handoff {

	const TTL            = 60;
	const MAX_PER_MINUTE = 10;
	const TRANSIENT      = 'createpress_handoff_';
	const RATE_TRANSIENT = 'createpress_handoff_rate_';
	const EMBED_COOKIE   = 'createpress_embedded';

	/**
	 * Registers REST routes under createpress/v1.
	 */
	public static function register_routes() {
		register_rest_route(
			REST_NAMESPACE,
			'/handoff',
			array(
				'methods'             => \WP_REST_Server::CREATABLE,
				'callback'            => array( self::class, 'issue' ),
				'permission_callback' => array( self::class, 'can_issue' ),
				'args'                => array(
					'redirect_to' => array(
						'type'              => 'string',
						'required'          => true,
						'format'            => 'uri',
						'sanitize_callback' => 'esc_url_raw',
					),
				),
			)
		);

		register_rest_route(
			REST_NAMESPACE,
			'/info',
			array(
				'methods'             => \WP_REST_Server::READABLE,
				'callback'            => static function () {
					return rest_ensure_response( array( 'version' => VERSION ) );
				},
				'permission_callback' => 'is_user_logged_in',
			)
		);
	}

	/**
	 * Only requests authenticated with an Application Password, by users who
	 * can edit content, may mint handoff links.
	 *
	 * @return true|\WP_Error
	 */
	public static function can_issue() {
		if ( ! is_user_logged_in() || ! rest_get_authenticated_app_password() ) {
			return new \WP_Error(
				'createpress_app_password_required',
				__( 'Sign-in handoff requires Application Password authentication.', 'createpress-bridge' ),
				array( 'status' => 401 )
			);
		}
		if ( ! current_user_can( 'edit_posts' ) && ! current_user_can( 'edit_pages' ) ) {
			return new \WP_Error(
				'createpress_forbidden',
				__( 'Your role cannot edit content on this site.', 'createpress-bridge' ),
				array( 'status' => 403 )
			);
		}
		return true;
	}

	/**
	 * Restricts the post-login destination to the block editor screens.
	 *
	 * @param string $url Requested destination.
	 * @return string|null Safe destination, or null when not allowed.
	 */
	public static function safe_editor_url( $url ) {
		$url = wp_validate_redirect( $url, '' );
		if ( '' === $url ) {
			return null;
		}
		$admin = admin_url();
		if ( 0 !== strpos( $url, $admin . 'post.php' ) && 0 !== strpos( $url, $admin . 'post-new.php' ) ) {
			return null;
		}
		return $url;
	}

	/**
	 * Hashes a token for storage so a leaked cache entry can't be replayed.
	 *
	 * @param string $token Raw token.
	 * @return string
	 */
	private static function hash( $token ) {
		return hash_hmac( 'sha256', $token, wp_salt( 'auth' ) );
	}

	/**
	 * REST callback: create a handoff link.
	 *
	 * @param \WP_REST_Request $request Request.
	 * @return \WP_REST_Response|\WP_Error
	 */
	public static function issue( \WP_REST_Request $request ) {
		$user_id  = get_current_user_id();
		$redirect = self::safe_editor_url( (string) $request->get_param( 'redirect_to' ) );
		if ( null === $redirect ) {
			return new \WP_Error(
				'createpress_bad_redirect',
				__( 'The editor address is not allowed.', 'createpress-bridge' ),
				array( 'status' => 400 )
			);
		}

		$rate_key = self::RATE_TRANSIENT . $user_id;
		$count    = (int) get_transient( $rate_key );
		if ( $count >= self::MAX_PER_MINUTE ) {
			return new \WP_Error(
				'createpress_rate_limited',
				__( 'Too many sign-in requests. Please wait a minute.', 'createpress-bridge' ),
				array( 'status' => 429 )
			);
		}
		set_transient( $rate_key, $count + 1, MINUTE_IN_SECONDS );

		$token = wp_generate_password( 48, false, false );
		set_transient(
			self::TRANSIENT . self::hash( $token ),
			array(
				'user_id'  => $user_id,
				'redirect' => $redirect,
				'expires'  => time() + self::TTL,
			),
			self::TTL
		);

		$url = add_query_arg(
			array(
				'action' => 'createpress_handoff',
				'token'  => rawurlencode( $token ),
			),
			wp_login_url()
		);

		$response = rest_ensure_response( array( 'url' => $url ) );
		$response->header( 'Cache-Control', 'no-store' );
		return $response;
	}

	/**
	 * Runs on wp-login.php?action=createpress_handoff: verifies the token, signs the
	 * user in and redirects to the editor.
	 */
	public static function consume() {
		nocache_headers();
		header( 'Referrer-Policy: no-referrer' );

		// phpcs:ignore WordPress.Security.NonceVerification.Recommended -- The single-use token is the CSRF protection.
		$token = isset( $_GET['token'] ) ? sanitize_text_field( wp_unslash( $_GET['token'] ) ) : '';
		if ( ! preg_match( '/^[A-Za-z0-9]{48}$/', $token ) ) {
			self::fail();
		}

		$key  = self::TRANSIENT . self::hash( $token );
		$data = get_transient( $key );
		delete_transient( $key ); // Single use, even on failure below.

		if ( ! is_array( $data ) || empty( $data['user_id'] ) || (int) $data['expires'] < time() ) {
			self::fail();
		}

		$user = get_user_by( 'id', (int) $data['user_id'] );
		if ( ! $user || ( ! user_can( $user, 'edit_posts' ) && ! user_can( $user, 'edit_pages' ) ) ) {
			self::fail();
		}

		$redirect = self::safe_editor_url( $data['redirect'] );
		if ( null === $redirect ) {
			$redirect = admin_url( 'post-new.php' );
		}

		wp_set_current_user( $user->ID );
		wp_set_auth_cookie( $user->ID, false, is_ssl() );
		/** This action is documented in wp-includes/user.php */
		do_action( 'wp_login', $user->user_login, $user );

		setcookie(
			self::EMBED_COOKIE,
			'1',
			array(
				'expires'  => 0,
				'path'     => ADMIN_COOKIE_PATH,
				'domain'   => COOKIE_DOMAIN,
				'secure'   => is_ssl(),
				'httponly' => true,
				'samesite' => 'Lax',
			)
		);

		wp_safe_redirect( $redirect );
		exit;
	}

	/**
	 * Ends the request with a generic error.
	 */
	private static function fail() {
		wp_die(
			esc_html__( 'This sign-in link has expired or was already used. Return to CreatePress and try again.', 'createpress-bridge' ),
			esc_html__( 'Sign-in link expired', 'createpress-bridge' ),
			array( 'response' => 403 )
		);
	}
}
