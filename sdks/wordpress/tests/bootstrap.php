<?php
/**
 * PHPUnit bootstrap — stubs WordPress functions and classes for unit testing
 * without requiring a full WP environment.
 *
 * @package Flapjack\WordPress\Tests
 */

declare(strict_types=1);

// Composer autoloader.
require_once dirname( __DIR__ ) . '/vendor/autoload.php';

// Define WP constants used by the plugin.
if ( ! defined( 'ABSPATH' ) ) {
    define( 'ABSPATH', '/tmp/wordpress/' );
}
if ( ! defined( 'FLAPJACK_SEARCH_VERSION' ) ) {
    define( 'FLAPJACK_SEARCH_VERSION', '0.1.0-test' );
}
if ( ! defined( 'FLAPJACK_SEARCH_FILE' ) ) {
    define( 'FLAPJACK_SEARCH_FILE', dirname( __DIR__ ) . '/flapjack-search.php' );
}
if ( ! defined( 'FLAPJACK_SEARCH_DIR' ) ) {
    define( 'FLAPJACK_SEARCH_DIR', dirname( __DIR__ ) . '/' );
}
if ( ! defined( 'FLAPJACK_SEARCH_URL' ) ) {
    define( 'FLAPJACK_SEARCH_URL', 'https://example.com/wp-content/plugins/flapjack-search/' );
}
if ( ! defined( 'FLAPJACK_SEARCH_BASENAME' ) ) {
    define( 'FLAPJACK_SEARCH_BASENAME', 'flapjack-search/flapjack-search.php' );
}

// Define WP_UNINSTALL_PLUGIN so uninstall.php can be tested.
if ( ! defined( 'WP_UNINSTALL_PLUGIN' ) ) {
    define( 'WP_UNINSTALL_PLUGIN', true );
}

// Load WordPress stubs.
require_once __DIR__ . '/stubs/wordpress-stubs.php';

// Load WP-CLI stubs.
require_once __DIR__ . '/stubs/wp-cli-stubs.php';
require_once __DIR__ . '/stubs/cli-table-stub.php';

// Load WooCommerce stubs.
require_once __DIR__ . '/stubs/woocommerce-stubs.php';
