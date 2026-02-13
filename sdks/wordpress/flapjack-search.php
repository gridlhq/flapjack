<?php
/**
 * Plugin Name:       Flapjack Search
 * Plugin URI:        https://flapjack.io/integrations/wordpress
 * Description:       Fast, typo-tolerant search for WordPress powered by Flapjack. Drop-in replacement for native WordPress search with instant results, faceted filtering, and WooCommerce support. (Beta)
 * Version:           0.1.0-beta
 * Requires at least: 6.4
 * Requires PHP:      7.4
 * Author:            Flapjack HQ
 * Author URI:        https://flapjack.io
 * License:           GPL-2.0-or-later
 * License URI:       https://www.gnu.org/licenses/gpl-2.0.html
 * Text Domain:       flapjack-search
 * Domain Path:       /languages
 *
 * @package Flapjack\WordPress
 */

declare(strict_types=1);

// Prevent direct access.
if ( ! defined( 'ABSPATH' ) ) {
    exit;
}

// Plugin constants.
define( 'FLAPJACK_SEARCH_VERSION', '0.1.0-beta' );
define( 'FLAPJACK_SEARCH_FILE', __FILE__ );
define( 'FLAPJACK_SEARCH_DIR', plugin_dir_path( __FILE__ ) );
define( 'FLAPJACK_SEARCH_URL', plugin_dir_url( __FILE__ ) );
define( 'FLAPJACK_SEARCH_BASENAME', plugin_basename( __FILE__ ) );

// Autoloader.
if ( file_exists( FLAPJACK_SEARCH_DIR . 'vendor/autoload.php' ) ) {
    require_once FLAPJACK_SEARCH_DIR . 'vendor/autoload.php';
}

/**
 * Boot the plugin.
 *
 * @return void
 */
function flapjack_search_boot(): void {
    $plugin = \Flapjack\WordPress\Plugin::get_instance();
    $plugin->init();
}

// Register activation and deactivation hooks before booting.
register_activation_hook( __FILE__, [ \Flapjack\WordPress\Plugin::class, 'activate' ] );
register_deactivation_hook( __FILE__, [ \Flapjack\WordPress\Plugin::class, 'deactivate' ] );

// Boot on plugins_loaded so all dependencies are available.
add_action( 'plugins_loaded', 'flapjack_search_boot' );