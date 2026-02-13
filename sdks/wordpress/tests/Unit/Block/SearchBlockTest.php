<?php
/**
 * Tests for Block\SearchBlock — Gutenberg block registration and rendering.
 *
 * @package Flapjack\WordPress\Tests\Unit\Block
 */

declare(strict_types=1);

namespace Flapjack\WordPress\Tests\Unit\Block;

use PHPUnit\Framework\TestCase;
use Flapjack\WordPress\Block\SearchBlock;

class SearchBlockTest extends TestCase {

    private SearchBlock $block;

    protected function setUp(): void {
        wp_stubs_reset();
        $this->block = new SearchBlock();
    }

    // ─── Hook registration ───────────────────────────────────

    public function test_register_adds_init_action(): void {
        global $wp_actions;
        $this->block->register();

        $hook_names = array_keys( $wp_actions );
        $this->assertContains( 'init', $hook_names );
    }

    public function test_register_binds_register_block_to_init(): void {
        global $wp_actions;
        $this->block->register();

        $callbacks = array_map(
            fn( $a ) => $a['callback'],
            $wp_actions['init']
        );

        $found = false;
        foreach ( $callbacks as $cb ) {
            if ( is_array( $cb ) && $cb[0] instanceof SearchBlock && $cb[1] === 'register_block' ) {
                $found = true;
                break;
            }
        }
        $this->assertTrue( $found, 'register_block should be bound to init action' );
    }

    // ─── Block registration via block.json ───────────────────

    public function test_register_block_registers_block_type(): void {
        global $wp_registered_blocks;
        $this->block->register_block();

        $this->assertArrayHasKey( 'flapjack/search', $wp_registered_blocks );
    }

    public function test_registered_block_has_correct_metadata(): void {
        global $wp_registered_blocks;
        $this->block->register_block();

        $meta = $wp_registered_blocks['flapjack/search'];
        $this->assertSame( 3, $meta['apiVersion'] );
        $this->assertSame( 'Flapjack Search', $meta['title'] );
        $this->assertSame( 'widgets', $meta['category'] );
        $this->assertSame( 'search', $meta['icon'] );
        $this->assertSame( 'flapjack-search', $meta['textdomain'] );
    }

    public function test_registered_block_has_render_callback(): void {
        global $wp_registered_blocks;
        $this->block->register_block();

        $meta = $wp_registered_blocks['flapjack/search'];
        $this->assertSame( 'file:./render.php', $meta['render'] );
    }

    public function test_registered_block_has_editor_script(): void {
        global $wp_registered_blocks;
        $this->block->register_block();

        $meta = $wp_registered_blocks['flapjack/search'];
        $this->assertSame( 'file:./index.js', $meta['editorScript'] );
    }

    public function test_registered_block_has_view_script(): void {
        global $wp_registered_blocks;
        $this->block->register_block();

        $meta = $wp_registered_blocks['flapjack/search'];
        $this->assertSame( 'file:./view.js', $meta['viewScript'] );
    }

    public function test_registered_block_has_styles(): void {
        global $wp_registered_blocks;
        $this->block->register_block();

        $meta = $wp_registered_blocks['flapjack/search'];
        $this->assertSame( 'file:./style.css', $meta['style'] );
        $this->assertSame( 'file:./editor.css', $meta['editorStyle'] );
    }

    public function test_registered_block_has_expected_attributes(): void {
        global $wp_registered_blocks;
        $this->block->register_block();

        $attrs = $wp_registered_blocks['flapjack/search']['attributes'];
        $this->assertArrayHasKey( 'placeholder', $attrs );
        $this->assertArrayHasKey( 'showButton', $attrs );
        $this->assertArrayHasKey( 'buttonText', $attrs );
        $this->assertArrayHasKey( 'showAutocomplete', $attrs );
        $this->assertArrayHasKey( 'maxSuggestions', $attrs );
    }

    public function test_placeholder_attribute_defaults_to_search(): void {
        global $wp_registered_blocks;
        $this->block->register_block();

        $attrs = $wp_registered_blocks['flapjack/search']['attributes'];
        $this->assertSame( 'Search...', $attrs['placeholder']['default'] );
    }

    public function test_show_button_attribute_defaults_to_true(): void {
        global $wp_registered_blocks;
        $this->block->register_block();

        $attrs = $wp_registered_blocks['flapjack/search']['attributes'];
        $this->assertTrue( $attrs['showButton']['default'] );
    }

    public function test_max_suggestions_attribute_defaults_to_five(): void {
        global $wp_registered_blocks;
        $this->block->register_block();

        $attrs = $wp_registered_blocks['flapjack/search']['attributes'];
        $this->assertSame( 5, $attrs['maxSuggestions']['default'] );
    }

    public function test_registered_block_has_keyword_support(): void {
        global $wp_registered_blocks;
        $this->block->register_block();

        $meta = $wp_registered_blocks['flapjack/search'];
        $this->assertContains( 'search', $meta['keywords'] );
        $this->assertContains( 'flapjack', $meta['keywords'] );
        $this->assertContains( 'autocomplete', $meta['keywords'] );
    }

    public function test_registered_block_supports_align(): void {
        global $wp_registered_blocks;
        $this->block->register_block();

        $supports = $wp_registered_blocks['flapjack/search']['supports'];
        $this->assertContains( 'wide', $supports['align'] );
        $this->assertContains( 'full', $supports['align'] );
    }

    public function test_registered_block_supports_color(): void {
        global $wp_registered_blocks;
        $this->block->register_block();

        $supports = $wp_registered_blocks['flapjack/search']['supports'];
        $this->assertTrue( $supports['color']['text'] );
        $this->assertTrue( $supports['color']['background'] );
    }

    public function test_registered_block_disables_html_editing(): void {
        global $wp_registered_blocks;
        $this->block->register_block();

        $supports = $wp_registered_blocks['flapjack/search']['supports'];
        $this->assertFalse( $supports['html'] );
    }

    // ─── Config localization ─────────────────────────────────

    public function test_register_block_localizes_config_to_view_script(): void {
        global $wp_inline_scripts;
        update_option( 'flapjack_app_id', 'my-app' );
        update_option( 'flapjack_search_api_key', 'search-key' );
        update_option( 'flapjack_host', 'http://localhost:7700' );
        update_option( 'flapjack_index_name', 'my_index' );

        $this->block->register_block();

        $this->assertArrayHasKey( 'flapjack-search-view-script', $wp_inline_scripts );
        $scripts = $wp_inline_scripts['flapjack-search-view-script'];
        $this->assertCount( 1, $scripts );
        $this->assertSame( 'before', $scripts[0]['position'] );
        $this->assertStringContainsString( 'flapjackSearchConfig', $scripts[0]['data'] );
    }

    public function test_localized_config_contains_app_id(): void {
        global $wp_inline_scripts;
        update_option( 'flapjack_app_id', 'test-app' );
        update_option( 'flapjack_search_api_key', 'search-key' );

        $this->block->register_block();

        $data = $wp_inline_scripts['flapjack-search-view-script'][0]['data'];
        $this->assertStringContainsString( 'test-app', $data );
    }

    public function test_localized_config_uses_search_key_when_available(): void {
        global $wp_inline_scripts;
        update_option( 'flapjack_app_id', 'app' );
        update_option( 'flapjack_search_api_key', 'search-only-key' );
        update_option( 'flapjack_api_key', 'admin-key' );

        $this->block->register_block();

        $data = $wp_inline_scripts['flapjack-search-view-script'][0]['data'];
        $this->assertStringContainsString( 'search-only-key', $data );
        $this->assertStringNotContainsString( 'admin-key', $data );
    }

    public function test_block_does_not_expose_admin_key_when_no_search_key(): void {
        global $wp_inline_scripts;
        update_option( 'flapjack_app_id', 'app' );
        update_option( 'flapjack_search_api_key', '' );
        update_option( 'flapjack_api_key', 'admin-key-secret' );

        $this->block->register_block();

        // No inline script should be generated when search key is missing.
        $this->assertArrayNotHasKey(
            'flapjack-search-view-script',
            $wp_inline_scripts,
            'Block must not expose admin API key in frontend when no search-only key is set'
        );
    }

    public function test_localized_config_contains_rest_url(): void {
        global $wp_inline_scripts;
        update_option( 'flapjack_search_api_key', 'search-key' );
        $this->block->register_block();

        $data = $wp_inline_scripts['flapjack-search-view-script'][0]['data'];
        $this->assertStringContainsString( 'restUrl', $data );
        $this->assertStringContainsString( 'flapjack-search\\/v1', $data );
    }

    public function test_localized_config_contains_nonce(): void {
        global $wp_inline_scripts;
        update_option( 'flapjack_search_api_key', 'search-key' );
        $this->block->register_block();

        $data = $wp_inline_scripts['flapjack-search-view-script'][0]['data'];
        $this->assertStringContainsString( 'nonce', $data );
    }

    public function test_localized_config_uses_default_index_name(): void {
        global $wp_inline_scripts;
        // Don't set flapjack_index_name — should default to wp_posts.
        update_option( 'flapjack_search_api_key', 'search-key' );
        $this->block->register_block();

        $data = $wp_inline_scripts['flapjack-search-view-script'][0]['data'];
        $this->assertStringContainsString( 'wp_posts', $data );
    }

    // ─── render.php output ───────────────────────────────────

    public function test_render_php_outputs_search_form(): void {
        $attributes = [
            'placeholder'      => 'Find something...',
            'showButton'       => true,
            'buttonText'       => 'Go',
            'showAutocomplete' => true,
            'maxSuggestions'   => 8,
        ];

        $output = $this->render_block( $attributes );

        $this->assertStringContainsString( 'role="search"', $output );
        $this->assertStringContainsString( 'flapjack-search-form', $output );
        $this->assertStringContainsString( 'type="search"', $output );
        $this->assertStringContainsString( 'name="s"', $output );
    }

    public function test_render_php_uses_custom_placeholder(): void {
        $output = $this->render_block( [ 'placeholder' => 'Find products...' ] );

        $this->assertStringContainsString( 'placeholder="Find products..."', $output );
    }

    public function test_render_php_shows_button_when_enabled(): void {
        $output = $this->render_block( [ 'showButton' => true, 'buttonText' => 'Search' ] );

        $this->assertStringContainsString( '<button', $output );
        $this->assertStringContainsString( 'flapjack-search-button', $output );
        $this->assertStringContainsString( 'Search', $output );
    }

    public function test_render_php_hides_button_when_disabled(): void {
        $output = $this->render_block( [ 'showButton' => false ] );

        $this->assertStringNotContainsString( '<button', $output );
        $this->assertStringNotContainsString( 'flapjack-search-button', $output );
    }

    public function test_render_php_uses_custom_button_text(): void {
        $output = $this->render_block( [ 'showButton' => true, 'buttonText' => 'Go!' ] );

        $this->assertStringContainsString( 'Go!', $output );
    }

    public function test_render_php_sets_autocomplete_data_attribute(): void {
        $output = $this->render_block( [ 'showAutocomplete' => true ] );

        $this->assertStringContainsString( 'data-flapjack-autocomplete="true"', $output );
    }

    public function test_render_php_disables_autocomplete_data_attribute(): void {
        $output = $this->render_block( [ 'showAutocomplete' => false ] );

        $this->assertStringContainsString( 'data-flapjack-autocomplete="false"', $output );
    }

    public function test_render_php_sets_max_suggestions_data_attribute(): void {
        $output = $this->render_block( [ 'maxSuggestions' => 10 ] );

        $this->assertStringContainsString( 'data-flapjack-max-suggestions="10"', $output );
    }

    public function test_render_php_includes_autocomplete_dropdown_container(): void {
        $output = $this->render_block( [] );

        $this->assertStringContainsString( 'flapjack-autocomplete-dropdown', $output );
    }

    public function test_render_php_includes_block_wrapper_attributes(): void {
        $output = $this->render_block( [] );

        $this->assertStringContainsString( 'wp-block-flapjack-search', $output );
    }

    public function test_render_php_form_action_points_to_home(): void {
        $output = $this->render_block( [] );

        $this->assertStringContainsString( 'action="https://example.com/"', $output );
    }

    public function test_render_php_input_has_autocomplete_off(): void {
        $output = $this->render_block( [] );

        $this->assertStringContainsString( 'autocomplete="off"', $output );
    }

    public function test_render_php_includes_screen_reader_label(): void {
        $output = $this->render_block( [] );

        $this->assertStringContainsString( 'screen-reader-text', $output );
        $this->assertStringContainsString( 'Search for:', $output );
    }

    public function test_render_php_uses_defaults_when_attributes_missing(): void {
        $output = $this->render_block( [] );

        // Default placeholder
        $this->assertStringContainsString( 'placeholder="Search..."', $output );
        // Default button shown
        $this->assertStringContainsString( '<button', $output );
        $this->assertStringContainsString( 'Search', $output );
        // Default autocomplete enabled
        $this->assertStringContainsString( 'data-flapjack-autocomplete="true"', $output );
        // Default max suggestions
        $this->assertStringContainsString( 'data-flapjack-max-suggestions="5"', $output );
    }

    // ─── Block file existence ────────────────────────────────

    public function test_block_json_exists(): void {
        $this->assertFileExists( SearchBlock::get_block_dir() . '/block.json' );
    }

    public function test_render_php_exists(): void {
        $this->assertFileExists( SearchBlock::get_block_dir() . '/render.php' );
    }

    public function test_style_css_exists(): void {
        $this->assertFileExists( SearchBlock::get_block_dir() . '/style.css' );
    }

    public function test_editor_css_exists(): void {
        $this->assertFileExists( SearchBlock::get_block_dir() . '/editor.css' );
    }

    public function test_index_js_exists(): void {
        $this->assertFileExists( SearchBlock::get_block_dir() . '/index.js' );
    }

    public function test_view_js_exists(): void {
        $this->assertFileExists( SearchBlock::get_block_dir() . '/view.js' );
    }

    public function test_block_json_is_valid_json(): void {
        $json = file_get_contents( SearchBlock::get_block_dir() . '/block.json' );
        $data = json_decode( $json, true );
        $this->assertNotNull( $data, 'block.json must be valid JSON' );
        $this->assertSame( 'flapjack/search', $data['name'] );
    }

    // ─── get_block_dir ───────────────────────────────────────

    public function test_get_block_dir_returns_correct_path(): void {
        $dir = SearchBlock::get_block_dir();
        $this->assertStringEndsWith( 'blocks/flapjack-search', $dir );
    }

    // ─── Helpers ─────────────────────────────────────────────

    /**
     * Include the render.php file with given attributes and capture output.
     */
    private function render_block( array $attributes ): string {
        // Merge with defaults matching block.json.
        $defaults = [
            'placeholder'      => 'Search...',
            'showButton'       => true,
            'buttonText'       => 'Search',
            'showAutocomplete' => true,
            'maxSuggestions'   => 5,
        ];
        $attributes = array_merge( $defaults, $attributes );

        $content = '';
        $block   = null;

        ob_start();
        include SearchBlock::get_block_dir() . '/render.php';
        return ob_get_clean();
    }
}
