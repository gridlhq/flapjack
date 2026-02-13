<?php
/**
 * Tests for WooCommerce FacetConfig.
 *
 * @package Flapjack\WordPress\Tests\Unit\WooCommerce
 */

declare(strict_types=1);

namespace Flapjack\WordPress\Tests\Unit\WooCommerce;

use PHPUnit\Framework\TestCase;
use Flapjack\WordPress\WooCommerce\FacetConfig;

class FacetConfigTest extends TestCase {

    protected function setUp(): void {
        wp_stubs_reset();
        wc_stubs_reset();
    }

    // ─── is_enabled ──────────────────────────────────────────

    public function test_is_enabled_returns_true_when_woocommerce_active_and_product_indexed(): void {
        // WooCommerce class loaded via stubs.
        update_option( 'flapjack_post_types', [ 'post', 'page', 'product' ] );

        $this->assertTrue( FacetConfig::is_enabled() );
    }

    public function test_is_enabled_returns_false_when_product_not_in_post_types(): void {
        update_option( 'flapjack_post_types', [ 'post', 'page' ] );

        $this->assertFalse( FacetConfig::is_enabled() );
    }

    public function test_is_enabled_returns_false_with_default_post_types(): void {
        // Default value is ['post', 'page'] — no 'product'.
        $this->assertFalse( FacetConfig::is_enabled() );
    }

    public function test_is_enabled_returns_true_with_product_only(): void {
        update_option( 'flapjack_post_types', [ 'product' ] );

        $this->assertTrue( FacetConfig::is_enabled() );
    }

    // ─── get_config ──────────────────────────────────────────

    public function test_get_config_returns_empty_array_when_disabled(): void {
        update_option( 'flapjack_post_types', [ 'post', 'page' ] );

        $this->assertSame( [], FacetConfig::get_config() );
    }

    public function test_get_config_returns_config_when_enabled(): void {
        update_option( 'flapjack_post_types', [ 'post', 'product' ] );

        $config = FacetConfig::get_config();

        $this->assertTrue( $config['enabled'] );
        $this->assertArrayHasKey( 'currencySymbol', $config );
        $this->assertArrayHasKey( 'widgets', $config );
    }

    public function test_get_config_includes_currency_symbol(): void {
        update_option( 'flapjack_post_types', [ 'product' ] );

        $config = FacetConfig::get_config();

        $this->assertSame( '$', $config['currencySymbol'] );
    }

    public function test_get_config_uses_custom_currency_symbol(): void {
        update_option( 'flapjack_post_types', [ 'product' ] );
        wc_set_currency_symbol( '€' );

        $config = FacetConfig::get_config();

        $this->assertSame( '€', $config['currencySymbol'] );
    }

    public function test_get_config_enabled_flag_is_true(): void {
        update_option( 'flapjack_post_types', [ 'product' ] );

        $config = FacetConfig::get_config();

        $this->assertTrue( $config['enabled'] );
    }

    // ─── get_widgets ─────────────────────────────────────────

    public function test_get_widgets_returns_five_default_widgets(): void {
        update_option( 'flapjack_post_types', [ 'product' ] );

        $widgets = FacetConfig::get_widgets();

        $this->assertCount( 5, $widgets );
    }

    public function test_get_widgets_includes_category_refinement_list(): void {
        update_option( 'flapjack_post_types', [ 'product' ] );

        $widgets = FacetConfig::get_widgets();
        $category = $this->find_widget_by_attribute( $widgets, 'taxonomy_product_cat' );

        $this->assertNotNull( $category );
        $this->assertSame( 'refinementList', $category['type'] );
        $this->assertSame( 'Categories', $category['label'] );
    }

    public function test_get_widgets_category_has_correct_options(): void {
        update_option( 'flapjack_post_types', [ 'product' ] );

        $widgets = FacetConfig::get_widgets();
        $category = $this->find_widget_by_attribute( $widgets, 'taxonomy_product_cat' );

        $this->assertSame( 10, $category['options']['limit'] );
        $this->assertTrue( $category['options']['showMore'] );
        $this->assertSame( 20, $category['options']['showMoreLimit'] );
        $this->assertTrue( $category['options']['searchable'] );
        $this->assertSame( [ 'count:desc', 'name:asc' ], $category['options']['sortBy'] );
    }

    public function test_get_widgets_includes_price_range_slider(): void {
        update_option( 'flapjack_post_types', [ 'product' ] );

        $widgets = FacetConfig::get_widgets();
        $price = $this->find_widget_by_attribute( $widgets, 'price' );

        $this->assertNotNull( $price );
        $this->assertSame( 'rangeSlider', $price['type'] );
        $this->assertSame( 'Price', $price['label'] );
        $this->assertSame( 0, $price['options']['precision'] );
    }

    public function test_get_widgets_includes_in_stock_toggle(): void {
        update_option( 'flapjack_post_types', [ 'product' ] );

        $widgets = FacetConfig::get_widgets();
        $stock = $this->find_widget_by_attribute( $widgets, 'in_stock' );

        $this->assertNotNull( $stock );
        $this->assertSame( 'toggleRefinement', $stock['type'] );
        $this->assertSame( 'In Stock', $stock['label'] );
        $this->assertSame( 1, $stock['options']['on'] );
        $this->assertNull( $stock['options']['off'] );
    }

    public function test_get_widgets_includes_on_sale_toggle(): void {
        update_option( 'flapjack_post_types', [ 'product' ] );

        $widgets = FacetConfig::get_widgets();
        $sale = $this->find_widget_by_attribute( $widgets, 'on_sale' );

        $this->assertNotNull( $sale );
        $this->assertSame( 'toggleRefinement', $sale['type'] );
        $this->assertSame( 'On Sale', $sale['label'] );
        $this->assertSame( 1, $sale['options']['on'] );
        $this->assertNull( $sale['options']['off'] );
    }

    public function test_get_widgets_includes_rating_menu(): void {
        update_option( 'flapjack_post_types', [ 'product' ] );

        $widgets = FacetConfig::get_widgets();
        $rating = $this->find_widget_by_attribute( $widgets, 'average_rating' );

        $this->assertNotNull( $rating );
        $this->assertSame( 'ratingMenu', $rating['type'] );
        $this->assertSame( 'Rating', $rating['label'] );
    }

    public function test_get_widgets_each_has_required_keys(): void {
        update_option( 'flapjack_post_types', [ 'product' ] );

        $widgets = FacetConfig::get_widgets();

        foreach ( $widgets as $widget ) {
            $this->assertArrayHasKey( 'type', $widget );
            $this->assertArrayHasKey( 'attribute', $widget );
            $this->assertArrayHasKey( 'label', $widget );
            $this->assertArrayHasKey( 'options', $widget );
        }
    }

    public function test_get_widgets_is_filterable(): void {
        update_option( 'flapjack_post_types', [ 'product' ] );

        add_filter( 'flapjack_woocommerce_facet_widgets', function ( $widgets ) {
            // Remove all except the first widget.
            return [ $widgets[0] ];
        } );

        $widgets = FacetConfig::get_widgets();

        $this->assertCount( 1, $widgets );
        $this->assertSame( 'taxonomy_product_cat', $widgets[0]['attribute'] );
    }

    public function test_get_widgets_filter_can_add_custom_widget(): void {
        update_option( 'flapjack_post_types', [ 'product' ] );

        add_filter( 'flapjack_woocommerce_facet_widgets', function ( $widgets ) {
            $widgets[] = [
                'type'      => 'refinementList',
                'attribute' => 'custom_attribute',
                'label'     => 'Custom',
                'options'   => [],
            ];
            return $widgets;
        } );

        $widgets = FacetConfig::get_widgets();

        $this->assertCount( 6, $widgets );
        $custom = $this->find_widget_by_attribute( $widgets, 'custom_attribute' );
        $this->assertNotNull( $custom );
        $this->assertSame( 'Custom', $custom['label'] );
    }

    // ─── get_container_ids ───────────────────────────────────

    public function test_get_container_ids_returns_correct_map(): void {
        $ids = FacetConfig::get_container_ids();

        $this->assertSame( 'flapjack-facet-categories', $ids['taxonomy_product_cat'] );
        $this->assertSame( 'flapjack-facet-price', $ids['price'] );
        $this->assertSame( 'flapjack-facet-stock', $ids['in_stock'] );
        $this->assertSame( 'flapjack-facet-sale', $ids['on_sale'] );
        $this->assertSame( 'flapjack-facet-rating', $ids['average_rating'] );
    }

    public function test_get_container_ids_has_five_entries(): void {
        $ids = FacetConfig::get_container_ids();

        $this->assertCount( 5, $ids );
    }

    public function test_get_container_ids_matches_widget_attributes(): void {
        update_option( 'flapjack_post_types', [ 'product' ] );

        $ids     = FacetConfig::get_container_ids();
        $widgets = FacetConfig::get_widgets();

        foreach ( $widgets as $widget ) {
            $this->assertArrayHasKey( $widget['attribute'], $ids,
                "Container ID missing for attribute: {$widget['attribute']}" );
        }
    }

    // ─── Integration: get_config structure ───────────────────

    public function test_get_config_widgets_match_get_widgets(): void {
        update_option( 'flapjack_post_types', [ 'product' ] );

        $config  = FacetConfig::get_config();
        $widgets = FacetConfig::get_widgets();

        $this->assertSame( $widgets, $config['widgets'] );
    }

    // ─── Helpers ─────────────────────────────────────────────

    private function find_widget_by_attribute( array $widgets, string $attribute ): ?array {
        foreach ( $widgets as $widget ) {
            if ( $widget['attribute'] === $attribute ) {
                return $widget;
            }
        }
        return null;
    }
}
