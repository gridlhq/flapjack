<?php
/**
 * Tests for SettingsPage.
 *
 * @package Flapjack\WordPress\Tests\Unit\Admin
 */

declare(strict_types=1);

namespace Flapjack\WordPress\Tests\Unit\Admin;

use PHPUnit\Framework\TestCase;
use Flapjack\WordPress\Admin\SettingsPage;

class SettingsPageTest extends TestCase {

    private SettingsPage $settings;

    protected function setUp(): void {
        wp_stubs_reset();
        $this->settings = new SettingsPage();
    }

    // ─── Registration ─────────────────────────────────────────

    public function test_register_adds_admin_hooks(): void {
        global $wp_actions, $wp_filters;
        $this->settings->register();

        $action_hooks = array_keys( $wp_actions );
        $filter_hooks = array_keys( $wp_filters );

        $this->assertContains( 'admin_menu', $action_hooks );
        $this->assertContains( 'admin_init', $action_hooks );
        $this->assertContains( 'admin_notices', $action_hooks );
        $this->assertContains( 'admin_enqueue_scripts', $action_hooks );
        $this->assertContains( 'wp_ajax_flapjack_test_connection', $action_hooks );
        $this->assertContains( 'wp_ajax_flapjack_reindex', $action_hooks );
        $this->assertContains( 'plugin_action_links_' . FLAPJACK_SEARCH_BASENAME, $filter_hooks );
    }

    // ─── Sanitization ─────────────────────────────────────────

    public function test_sanitize_post_types_returns_array_of_sanitized_keys(): void {
        $result = $this->settings->sanitize_post_types( [ 'Post', 'page', 'Custom-Type' ] );
        $this->assertSame( [ 'post', 'page', 'custom-type' ], $result );
    }

    public function test_sanitize_post_types_returns_default_for_non_array(): void {
        $result = $this->settings->sanitize_post_types( 'not_an_array' );
        $this->assertSame( [ 'post', 'page' ], $result );
    }

    public function test_sanitize_post_types_returns_default_for_null(): void {
        $result = $this->settings->sanitize_post_types( null );
        $this->assertSame( [ 'post', 'page' ], $result );
    }

    public function test_sanitize_searchable_attrs_filters_to_allowed_values(): void {
        $result = $this->settings->sanitize_searchable_attrs( [ 'post_title', 'invalid_field', 'author' ] );
        $this->assertSame( [ 'post_title', 'author' ], array_values( $result ) );
    }

    public function test_sanitize_searchable_attrs_returns_default_for_non_array(): void {
        $result = $this->settings->sanitize_searchable_attrs( 'not_an_array' );
        $this->assertSame( [ 'post_title', 'post_content', 'post_excerpt' ], $result );
    }

    public function test_sanitize_searchable_attrs_allows_all_valid_attrs(): void {
        $all = [ 'post_title', 'post_content', 'post_excerpt', 'taxonomies', 'author', 'meta' ];
        $result = $this->settings->sanitize_searchable_attrs( $all );
        $this->assertSame( $all, array_values( $result ) );
    }

    public function test_sanitize_searchable_attrs_rejects_all_invalid(): void {
        $result = $this->settings->sanitize_searchable_attrs( [ 'foo', 'bar', 'baz' ] );
        $this->assertEmpty( $result );
    }

    // ─── Settings link ────────────────────────────────────────

    public function test_add_settings_link_prepends_link(): void {
        $links = [ '<a href="#">Deactivate</a>' ];
        $result = $this->settings->add_settings_link( $links );

        $this->assertCount( 2, $result );
        $this->assertStringContainsString( 'Settings', $result[0] );
        $this->assertStringContainsString( 'flapjack-search', $result[0] );
    }

    // ─── Activation notice ────────────────────────────────────

    public function test_activation_notice_shows_when_transient_set(): void {
        set_transient( 'flapjack_search_activated', true );

        ob_start();
        $this->settings->activation_notice();
        $output = ob_get_clean();

        $this->assertStringContainsString( 'Flapjack Search activated', $output );
        $this->assertStringContainsString( 'Configure your API credentials', $output );

        // Transient should be deleted after showing.
        $this->assertFalse( get_transient( 'flapjack_search_activated' ) );
    }

    public function test_activation_notice_hidden_when_no_transient(): void {
        ob_start();
        $this->settings->activation_notice();
        $output = ob_get_clean();

        $this->assertEmpty( $output );
    }

    // ─── Constants ────────────────────────────────────────────

    public function test_option_group_constant(): void {
        $this->assertSame( 'flapjack_search_settings', SettingsPage::OPTION_GROUP );
    }

    public function test_page_slug_constant(): void {
        $this->assertSame( 'flapjack-search', SettingsPage::PAGE_SLUG );
    }

    // ─── Render methods ──────────────────────────────────────

    public function test_render_text_field_outputs_input(): void {
        update_option( 'flapjack_app_id', 'my-app-id' );

        ob_start();
        $this->settings->render_text_field( [ 'id' => 'flapjack_app_id', 'description' => 'Enter your App ID.' ] );
        $output = ob_get_clean();

        $this->assertStringContainsString( 'type="text"', $output );
        $this->assertStringContainsString( 'name="flapjack_app_id"', $output );
        $this->assertStringContainsString( 'my-app-id', $output );
        $this->assertStringContainsString( 'Enter your App ID.', $output );
    }

    public function test_render_text_field_without_description(): void {
        ob_start();
        $this->settings->render_text_field( [ 'id' => 'flapjack_host' ] );
        $output = ob_get_clean();

        $this->assertStringContainsString( 'type="text"', $output );
        $this->assertStringNotContainsString( '<p class="description">', $output );
    }

    public function test_render_password_field_outputs_password_input(): void {
        update_option( 'flapjack_api_key', 'secret-key' );

        ob_start();
        $this->settings->render_password_field( [ 'id' => 'flapjack_api_key', 'description' => '' ] );
        $output = ob_get_clean();

        $this->assertStringContainsString( 'type="password"', $output );
        $this->assertStringContainsString( 'name="flapjack_api_key"', $output );
        $this->assertStringContainsString( 'secret-key', $output );
    }

    public function test_render_checkbox_field_outputs_checked_when_true(): void {
        update_option( 'flapjack_enable_search', true );

        ob_start();
        $this->settings->render_checkbox_field( [
            'id'          => 'flapjack_enable_search',
            'description' => 'Enable backend search.',
        ] );
        $output = ob_get_clean();

        $this->assertStringContainsString( 'type="checkbox"', $output );
        $this->assertStringContainsString( "checked='checked'", $output );
        $this->assertStringContainsString( 'Enable backend search.', $output );
    }

    public function test_render_checkbox_field_outputs_unchecked_when_false(): void {
        update_option( 'flapjack_enable_instant', false );

        ob_start();
        $this->settings->render_checkbox_field( [
            'id'          => 'flapjack_enable_instant',
            'description' => 'Enable instant search.',
        ] );
        $output = ob_get_clean();

        $this->assertStringContainsString( 'type="checkbox"', $output );
        $this->assertStringNotContainsString( "checked='checked'", $output );
    }

    public function test_render_number_field_outputs_number_input(): void {
        update_option( 'flapjack_posts_per_page', 25 );

        ob_start();
        $this->settings->render_number_field( [
            'id'  => 'flapjack_posts_per_page',
            'min' => 1,
            'max' => 100,
        ] );
        $output = ob_get_clean();

        $this->assertStringContainsString( 'type="number"', $output );
        $this->assertStringContainsString( 'value="25"', $output );
        $this->assertStringContainsString( 'min="1"', $output );
        $this->assertStringContainsString( 'max="100"', $output );
    }

    public function test_render_settings_page_outputs_form(): void {
        ob_start();
        $this->settings->render_settings_page();
        $output = ob_get_clean();

        $this->assertStringContainsString( '<form action="options.php"', $output );
        $this->assertStringContainsString( 'Test Connection', $output );
        $this->assertStringContainsString( 'Reindex All Content', $output );
        $this->assertStringContainsString( 'flapjack-test-connection', $output );
        $this->assertStringContainsString( 'flapjack-reindex', $output );
    }

    public function test_render_connection_section_outputs_text(): void {
        ob_start();
        $this->settings->render_connection_section();
        $output = ob_get_clean();

        $this->assertStringContainsString( 'Flapjack API credentials', $output );
    }

    public function test_render_indexing_section_outputs_text(): void {
        ob_start();
        $this->settings->render_indexing_section();
        $output = ob_get_clean();

        $this->assertStringContainsString( 'content gets indexed', $output );
    }

    public function test_render_search_section_outputs_text(): void {
        ob_start();
        $this->settings->render_search_section();
        $output = ob_get_clean();

        $this->assertStringContainsString( 'search', $output );
    }

    public function test_render_post_types_field_outputs_checkboxes(): void {
        update_option( 'flapjack_post_types', [ 'post' ] );

        ob_start();
        $this->settings->render_post_types_field();
        $output = ob_get_clean();

        $this->assertStringContainsString( 'flapjack_post_types[]', $output );
        $this->assertStringContainsString( 'value="post"', $output );
        $this->assertStringContainsString( 'value="page"', $output );
    }

    public function test_render_searchable_attrs_field_outputs_all_attributes(): void {
        update_option( 'flapjack_searchable_attrs', [ 'post_title', 'post_content' ] );

        ob_start();
        $this->settings->render_searchable_attrs_field();
        $output = ob_get_clean();

        $this->assertStringContainsString( 'flapjack_searchable_attrs[]', $output );
        $this->assertStringContainsString( 'value="post_title"', $output );
        $this->assertStringContainsString( 'value="post_content"', $output );
        $this->assertStringContainsString( 'value="post_excerpt"', $output );
        $this->assertStringContainsString( 'value="taxonomies"', $output );
        $this->assertStringContainsString( 'value="author"', $output );
        $this->assertStringContainsString( 'value="meta"', $output );
    }

    // ─── Admin JS enqueue ────────────────────────────────────

    public function test_enqueue_admin_assets_on_settings_page(): void {
        global $wp_enqueued_scripts;
        $this->settings->enqueue_admin_assets( 'settings_page_flapjack-search' );

        $this->assertArrayHasKey( 'flapjack-search-admin', $wp_enqueued_scripts );
    }

    public function test_enqueue_admin_assets_skips_other_pages(): void {
        global $wp_enqueued_scripts;
        $this->settings->enqueue_admin_assets( 'toplevel_page_other-plugin' );

        $this->assertEmpty( $wp_enqueued_scripts );
    }

    public function test_enqueue_admin_assets_skips_empty_hook(): void {
        global $wp_enqueued_scripts;
        $this->settings->enqueue_admin_assets( '' );

        $this->assertEmpty( $wp_enqueued_scripts );
    }

    public function test_enqueue_admin_assets_registers_correct_script_src(): void {
        global $wp_enqueued_scripts;
        $this->settings->enqueue_admin_assets( 'settings_page_flapjack-search' );

        $script = $wp_enqueued_scripts['flapjack-search-admin'];
        $this->assertStringContainsString( 'assets/js/admin.js', $script['src'] );
    }

    public function test_enqueue_admin_assets_depends_on_jquery(): void {
        global $wp_enqueued_scripts;
        $this->settings->enqueue_admin_assets( 'settings_page_flapjack-search' );

        $script = $wp_enqueued_scripts['flapjack-search-admin'];
        $this->assertContains( 'jquery', $script['deps'] );
    }

    public function test_enqueue_admin_assets_uses_plugin_version(): void {
        global $wp_enqueued_scripts;
        $this->settings->enqueue_admin_assets( 'settings_page_flapjack-search' );

        $script = $wp_enqueued_scripts['flapjack-search-admin'];
        $this->assertSame( FLAPJACK_SEARCH_VERSION, $script['ver'] );
    }

    public function test_enqueue_admin_assets_localizes_config(): void {
        global $wp_localized_scripts;
        $this->settings->enqueue_admin_assets( 'settings_page_flapjack-search' );

        $this->assertArrayHasKey( 'flapjack-search-admin', $wp_localized_scripts );
        $localized = $wp_localized_scripts['flapjack-search-admin'];
        $this->assertSame( 'flapjackAdminConfig', $localized['object_name'] );
    }

    public function test_enqueue_admin_assets_config_contains_nonces(): void {
        global $wp_localized_scripts;
        $this->settings->enqueue_admin_assets( 'settings_page_flapjack-search' );

        $config = $wp_localized_scripts['flapjack-search-admin']['data'];
        $this->assertArrayHasKey( 'testNonce', $config );
        $this->assertArrayHasKey( 'reindexNonce', $config );
        $this->assertNotEmpty( $config['testNonce'] );
        $this->assertNotEmpty( $config['reindexNonce'] );
    }

    public function test_enqueue_admin_assets_config_contains_i18n_strings(): void {
        global $wp_localized_scripts;
        $this->settings->enqueue_admin_assets( 'settings_page_flapjack-search' );

        $config = $wp_localized_scripts['flapjack-search-admin']['data'];
        $this->assertArrayHasKey( 'i18n', $config );
        $this->assertSame( 'Testing...', $config['i18n']['testing'] );
        $this->assertSame( 'Reindexing...', $config['i18n']['reindexing'] );
    }

    public function test_render_settings_page_no_longer_has_inline_script(): void {
        ob_start();
        $this->settings->render_settings_page();
        $output = ob_get_clean();

        $this->assertStringNotContainsString( '<script>', $output );
        $this->assertStringNotContainsString( 'jQuery(function', $output );
    }

    // ─── Background reindex AJAX hooks ───────────────────────

    public function test_register_adds_background_reindex_ajax_hooks(): void {
        global $wp_actions;
        $this->settings->register();

        $hooks = array_keys( $wp_actions );
        $this->assertContains( 'wp_ajax_flapjack_reindex_background', $hooks );
        $this->assertContains( 'wp_ajax_flapjack_reindex_progress', $hooks );
        $this->assertContains( 'wp_ajax_flapjack_reindex_cancel', $hooks );
    }

    public function test_enqueue_admin_assets_config_contains_background_nonces(): void {
        global $wp_localized_scripts;
        $this->settings->enqueue_admin_assets( 'settings_page_flapjack-search' );

        $config = $wp_localized_scripts['flapjack-search-admin']['data'];
        $this->assertArrayHasKey( 'reindexBgNonce', $config );
        $this->assertArrayHasKey( 'reindexProgressNonce', $config );
        $this->assertArrayHasKey( 'reindexCancelNonce', $config );
        $this->assertNotEmpty( $config['reindexBgNonce'] );
        $this->assertNotEmpty( $config['reindexProgressNonce'] );
        $this->assertNotEmpty( $config['reindexCancelNonce'] );
    }

    public function test_enqueue_admin_assets_config_contains_background_i18n(): void {
        global $wp_localized_scripts;
        $this->settings->enqueue_admin_assets( 'settings_page_flapjack-search' );

        $config = $wp_localized_scripts['flapjack-search-admin']['data'];
        $this->assertSame( 'Starting background reindex...', $config['i18n']['starting'] );
        $this->assertSame( 'Cancelling...', $config['i18n']['cancelling'] );
        $this->assertSame( 'Complete!', $config['i18n']['complete'] );
        $this->assertSame( 'Cancelled.', $config['i18n']['cancelled'] );
        $this->assertSame( 'Failed.', $config['i18n']['failed'] );
        $this->assertArrayHasKey( 'progressFmt', $config['i18n'] );
    }

    public function test_render_settings_page_includes_background_reindex_button(): void {
        ob_start();
        $this->settings->render_settings_page();
        $output = ob_get_clean();

        $this->assertStringContainsString( 'flapjack-reindex-background', $output );
        $this->assertStringContainsString( 'flapjack-reindex-cancel', $output );
        $this->assertStringContainsString( 'flapjack-reindex-progress', $output );
        $this->assertStringContainsString( 'Background Reindex', $output );
    }

    public function test_render_settings_page_includes_progress_bar(): void {
        ob_start();
        $this->settings->render_settings_page();
        $output = ob_get_clean();

        $this->assertStringContainsString( 'flapjack-progress-bar', $output );
        $this->assertStringContainsString( 'flapjack-progress-fill', $output );
        $this->assertStringContainsString( 'flapjack-progress-text', $output );
    }

    // ─── AJAX permission denial ─────────────────────────────

    public function test_ajax_test_connection_denies_non_admin(): void {
        global $wp_current_user_can, $wp_last_json_response;
        $wp_current_user_can = false;

        try {
            $this->settings->ajax_test_connection();
            $this->fail( 'Expected WPJsonResponseException' );
        } catch ( \WPJsonResponseException $e ) {
            // Expected — wp_send_json_error throws to simulate wp_die().
        }

        $this->assertNotNull( $wp_last_json_response );
        $this->assertFalse( $wp_last_json_response['success'] );
        $this->assertSame( 'Permission denied.', $wp_last_json_response['data']['message'] );
    }

    public function test_ajax_reindex_denies_non_admin(): void {
        global $wp_current_user_can, $wp_last_json_response;
        $wp_current_user_can = false;

        try {
            $this->settings->ajax_reindex();
            $this->fail( 'Expected WPJsonResponseException' );
        } catch ( \WPJsonResponseException $e ) {
            // Expected.
        }

        $this->assertNotNull( $wp_last_json_response );
        $this->assertFalse( $wp_last_json_response['success'] );
        $this->assertSame( 'Permission denied.', $wp_last_json_response['data']['message'] );
    }

    public function test_ajax_reindex_background_denies_non_admin(): void {
        global $wp_current_user_can, $wp_last_json_response;
        $wp_current_user_can = false;

        try {
            $this->settings->ajax_reindex_background();
            $this->fail( 'Expected WPJsonResponseException' );
        } catch ( \WPJsonResponseException $e ) {
            // Expected.
        }

        $this->assertNotNull( $wp_last_json_response );
        $this->assertFalse( $wp_last_json_response['success'] );
        $this->assertSame( 'Permission denied.', $wp_last_json_response['data']['message'] );
    }

    public function test_ajax_reindex_progress_denies_non_admin(): void {
        global $wp_current_user_can, $wp_last_json_response;
        $wp_current_user_can = false;

        try {
            $this->settings->ajax_reindex_progress();
            $this->fail( 'Expected WPJsonResponseException' );
        } catch ( \WPJsonResponseException $e ) {
            // Expected.
        }

        $this->assertNotNull( $wp_last_json_response );
        $this->assertFalse( $wp_last_json_response['success'] );
        $this->assertSame( 'Permission denied.', $wp_last_json_response['data']['message'] );
    }

    public function test_ajax_reindex_cancel_denies_non_admin(): void {
        global $wp_current_user_can, $wp_last_json_response;
        $wp_current_user_can = false;

        try {
            $this->settings->ajax_reindex_cancel();
            $this->fail( 'Expected WPJsonResponseException' );
        } catch ( \WPJsonResponseException $e ) {
            // Expected.
        }

        $this->assertNotNull( $wp_last_json_response );
        $this->assertFalse( $wp_last_json_response['success'] );
        $this->assertSame( 'Permission denied.', $wp_last_json_response['data']['message'] );
    }

    // ─── Settings page permission check ─────────────────────

    public function test_render_settings_page_returns_nothing_for_non_admin(): void {
        global $wp_current_user_can;
        $wp_current_user_can = false;

        ob_start();
        $this->settings->render_settings_page();
        $output = ob_get_clean();

        $this->assertEmpty( $output );
    }
}
