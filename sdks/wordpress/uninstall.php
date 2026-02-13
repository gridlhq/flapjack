<?php
/**
 * Flapjack Search — Uninstall handler.
 *
 * Fired when the plugin is deleted (not just deactivated).
 * Removes all plugin options, transients, and scheduled events.
 *
 * @package Flapjack\WordPress
 */

// Abort if not called by WordPress uninstall process.
if ( ! defined( 'WP_UNINSTALL_PLUGIN' ) ) {
    exit;
}

// All options created by the plugin.
$options = [
    'flapjack_app_id',
    'flapjack_api_key',
    'flapjack_search_api_key',
    'flapjack_host',
    'flapjack_index_name',
    'flapjack_post_types',
    'flapjack_enable_search',
    'flapjack_enable_instant',
    'flapjack_posts_per_page',
    'flapjack_searchable_attrs',
];

foreach ( $options as $option ) {
    delete_option( $option );
}

// Transients.
delete_transient( 'flapjack_search_activated' );
delete_transient( 'flapjack_reindex_progress' );

// Scheduled cron events.
wp_clear_scheduled_hook( 'flapjack_search_reindex_cron' );
wp_clear_scheduled_hook( 'flapjack_background_reindex_batch' );
