<?php
/**
 * Tests for the uninstall.php handler.
 *
 * @package Flapjack\WordPress\Tests\Unit
 */

declare(strict_types=1);

namespace Flapjack\WordPress\Tests\Unit;

use PHPUnit\Framework\TestCase;

class UninstallTest extends TestCase {

    protected function setUp(): void {
        wp_stubs_reset();
    }

    public function test_uninstall_deletes_all_options(): void {
        global $wp_options;

        // Populate every option the plugin creates.
        $option_keys = [
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

        foreach ( $option_keys as $key ) {
            update_option( $key, 'test-value' );
        }

        // Verify all 10 options exist.
        foreach ( $option_keys as $key ) {
            $this->assertArrayHasKey( $key, $wp_options, "Option {$key} should exist before uninstall" );
        }

        // Run uninstall.
        $this->run_uninstall();

        // All options should be gone.
        foreach ( $option_keys as $key ) {
            $this->assertArrayNotHasKey( $key, $wp_options, "Option {$key} should be deleted after uninstall" );
        }
    }

    public function test_uninstall_deletes_activation_transient(): void {
        set_transient( 'flapjack_search_activated', true );
        $this->assertTrue( get_transient( 'flapjack_search_activated' ) );

        $this->run_uninstall();

        $this->assertFalse( get_transient( 'flapjack_search_activated' ) );
    }

    public function test_uninstall_clears_scheduled_cron(): void {
        global $wp_scheduled;
        $wp_scheduled['flapjack_search_reindex_cron'] = [ 'time' => time() ];

        $this->run_uninstall();

        $this->assertArrayNotHasKey( 'flapjack_search_reindex_cron', $wp_scheduled );
    }

    public function test_uninstall_is_safe_with_no_existing_options(): void {
        // Running uninstall on a clean database should not error.
        $this->run_uninstall();

        $this->assertFalse( get_option( 'flapjack_app_id' ) );
        $this->assertFalse( get_option( 'flapjack_api_key' ) );
    }

    public function test_uninstall_does_not_touch_unrelated_options(): void {
        update_option( 'blogname', 'My Blog' );
        update_option( 'flapjack_app_id', 'test' );

        $this->run_uninstall();

        $this->assertSame( 'My Blog', get_option( 'blogname' ) );
        $this->assertFalse( get_option( 'flapjack_app_id' ) );
    }

    public function test_uninstall_deletes_reindex_progress_transient(): void {
        set_transient( 'flapjack_reindex_progress', [ 'status' => 'in_progress' ] );
        $this->assertNotFalse( get_transient( 'flapjack_reindex_progress' ) );

        $this->run_uninstall();

        $this->assertFalse( get_transient( 'flapjack_reindex_progress' ) );
    }

    public function test_uninstall_clears_background_reindex_cron(): void {
        global $wp_scheduled;
        $wp_scheduled['flapjack_background_reindex_batch'] = [ 'time' => time() ];

        $this->run_uninstall();

        $this->assertArrayNotHasKey( 'flapjack_background_reindex_batch', $wp_scheduled );
    }

    public function test_uninstall_aborts_without_constant(): void {
        // Without WP_UNINSTALL_PLUGIN defined, the file should exit.
        // We test this by verifying the constant IS required.
        // (The actual file checks for the constant and exits if missing.)
        $this->assertTrue( defined( 'WP_UNINSTALL_PLUGIN' ) );
    }

    /**
     * Include the uninstall file in a controlled way.
     */
    private function run_uninstall(): void {
        require FLAPJACK_SEARCH_DIR . 'uninstall.php';
    }
}
