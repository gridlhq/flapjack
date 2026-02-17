<?php
/**
 * Tests for the main Plugin class.
 *
 * @package Flapjack\WordPress\Tests\Unit
 */

declare(strict_types=1);

namespace Flapjack\WordPress\Tests\Unit;

use PHPUnit\Framework\TestCase;
use Flapjack\WordPress\Plugin;

class PluginTest extends TestCase {

    protected function setUp(): void {
        wp_stubs_reset();
    }

    public function test_get_instance_returns_singleton(): void {
        // Use reflection to reset the singleton for testing.
        $reflection = new \ReflectionClass( Plugin::class );
        $instance_prop = $reflection->getProperty( 'instance' );
        $instance_prop->setValue( null, null );

        $instance1 = Plugin::get_instance();
        $instance2 = Plugin::get_instance();

        $this->assertSame( $instance1, $instance2 );
    }

    public function test_activate_sets_default_options(): void {
        Plugin::activate();

        $this->assertSame( '', get_option( 'flapjack_app_id' ) );
        $this->assertSame( '', get_option( 'flapjack_api_key' ) );
        $this->assertSame( 'wp_posts', get_option( 'flapjack_index_name' ) );
        $this->assertSame( [ 'post', 'page' ], get_option( 'flapjack_post_types' ) );
        $this->assertTrue( get_option( 'flapjack_enable_search' ) );
        $this->assertFalse( get_option( 'flapjack_enable_instant' ) );
        $this->assertSame( 20, get_option( 'flapjack_posts_per_page' ) );
        $this->assertSame( [ 'post_title', 'post_content', 'post_excerpt' ], get_option( 'flapjack_searchable_attrs' ) );
    }

    public function test_activate_does_not_overwrite_existing_options(): void {
        update_option( 'flapjack_app_id', 'existing-id' );
        update_option( 'flapjack_index_name', 'custom_index' );

        Plugin::activate();

        $this->assertSame( 'existing-id', get_option( 'flapjack_app_id' ) );
        $this->assertSame( 'custom_index', get_option( 'flapjack_index_name' ) );
    }

    public function test_activate_sets_activation_transient(): void {
        Plugin::activate();
        $this->assertTrue( get_transient( 'flapjack_search_activated' ) );
    }

    public function test_deactivate_completes_without_error(): void {
        // Set up some state that deactivation should clean up.
        set_transient( 'flapjack_search_activated', true );

        Plugin::deactivate();

        // Verify deactivation ran successfully (flush_rewrite_rules and
        // wp_clear_scheduled_hook are stubs, but we verify no exceptions).
        $this->assertInstanceOf( Plugin::class, Plugin::get_instance() );
    }

    public function test_activate_then_deactivate_lifecycle(): void {
        Plugin::activate();

        // Verify activation state.
        $this->assertTrue( get_transient( 'flapjack_search_activated' ) );
        $this->assertSame( 'wp_posts', get_option( 'flapjack_index_name' ) );

        Plugin::deactivate();

        // Options should persist after deactivation (only uninstall removes them).
        $this->assertSame( 'wp_posts', get_option( 'flapjack_index_name' ) );
    }

    public function test_init_creates_client_factory(): void {
        $reflection = new \ReflectionClass( Plugin::class );
        $instance_prop = $reflection->getProperty( 'instance' );
        $instance_prop->setValue( null, null );

        $plugin = Plugin::get_instance();
        $plugin->init();

        $factory = $plugin->get_client_factory();
        $this->assertInstanceOf( \Flapjack\WordPress\ClientFactory::class, $factory );
    }

    public function test_init_registers_text_domain_hook(): void {
        global $wp_actions;

        $reflection = new \ReflectionClass( Plugin::class );
        $instance_prop = $reflection->getProperty( 'instance' );
        $instance_prop->setValue( null, null );

        $plugin = Plugin::get_instance();
        $plugin->init();

        $hook_names = array_keys( $wp_actions );
        $this->assertContains( 'init', $hook_names );
    }

    public function test_init_registers_rest_api_hook(): void {
        global $wp_actions;

        $reflection = new \ReflectionClass( Plugin::class );
        $instance_prop = $reflection->getProperty( 'instance' );
        $instance_prop->setValue( null, null );

        $plugin = Plugin::get_instance();
        $plugin->init();

        $hook_names = array_keys( $wp_actions );
        $this->assertContains( 'rest_api_init', $hook_names );
    }

    public function test_init_registers_background_indexer(): void {
        global $wp_actions;

        $reflection = new \ReflectionClass( Plugin::class );
        $instance_prop = $reflection->getProperty( 'instance' );
        $instance_prop->setValue( null, null );

        $plugin = Plugin::get_instance();
        $plugin->init();

        // BackgroundIndexer registers the batch hook.
        $hook_names = array_keys( $wp_actions );
        $this->assertContains(
            \Flapjack\WordPress\Indexing\BackgroundIndexer::BATCH_HOOK,
            $hook_names,
            'BackgroundIndexer batch hook should be registered'
        );
    }

    public function test_deactivate_clears_background_reindex_hook(): void {
        global $wp_scheduled;

        // Schedule something on the batch hook.
        $wp_scheduled[\Flapjack\WordPress\Indexing\BackgroundIndexer::BATCH_HOOK] = [ 'test' ];

        Plugin::deactivate();

        $this->assertArrayNotHasKey(
            \Flapjack\WordPress\Indexing\BackgroundIndexer::BATCH_HOOK,
            $wp_scheduled
        );
    }

    public function test_deactivate_deletes_progress_transient(): void {
        set_transient( \Flapjack\WordPress\Indexing\BackgroundIndexer::PROGRESS_TRANSIENT, [ 'status' => 'in_progress' ] );

        Plugin::deactivate();

        $this->assertFalse( get_transient( \Flapjack\WordPress\Indexing\BackgroundIndexer::PROGRESS_TRANSIENT ) );
    }

    public function test_init_registers_gutenberg_block(): void {
        global $wp_actions;

        $reflection = new \ReflectionClass( Plugin::class );
        $instance_prop = $reflection->getProperty( 'instance' );
        $instance_prop->setValue( null, null );

        $plugin = Plugin::get_instance();
        $plugin->init();

        // SearchBlock registers on `init` — verify a SearchBlock callback is bound.
        $init_callbacks = $wp_actions['init'] ?? [];
        $found = false;
        foreach ( $init_callbacks as $entry ) {
            $cb = $entry['callback'];
            if ( is_array( $cb ) && $cb[0] instanceof \Flapjack\WordPress\Block\SearchBlock ) {
                $found = true;
                break;
            }
        }
        $this->assertTrue( $found, 'SearchBlock should be registered on the init hook' );
    }
}
