<?php
/**
 * Tests for WooCommerce ProductIntegration.
 *
 * @package Flapjack\WordPress\Tests\Unit\WooCommerce
 */

declare(strict_types=1);

namespace Flapjack\WordPress\Tests\Unit\WooCommerce;

use PHPUnit\Framework\TestCase;
use PHPUnit\Framework\MockObject\MockObject;
use Flapjack\WordPress\ClientFactory;
use Flapjack\WordPress\Indexing\IndexManager;
use Flapjack\WordPress\WooCommerce\ProductIntegration;
use Flapjack\WordPress\Tests\Traits\MakesTestPosts;
use Flapjack\FlapjackSearch\Api\SearchClient;

class ProductIntegrationTest extends TestCase {

    use MakesTestPosts;

    private ClientFactory&MockObject $client_factory;
    private SearchClient&MockObject $search_client;
    private IndexManager $index_manager;
    private ProductIntegration $integration;

    protected function setUp(): void {
        wp_stubs_reset();
        wc_stubs_reset();

        $this->search_client  = $this->createMock( SearchClient::class );
        $this->client_factory = $this->createMock( ClientFactory::class );

        $this->client_factory->method( 'get_client' )->willReturn( $this->search_client );
        $this->client_factory->method( 'get_index_name' )->willReturn( 'wp_posts' );

        $this->index_manager = new IndexManager( $this->client_factory );
        $this->integration   = new ProductIntegration( $this->index_manager );

        // Default options.
        update_option( 'flapjack_post_types', [ 'post', 'page', 'product' ] );
        update_option( 'flapjack_searchable_attrs', [ 'post_title', 'post_content', 'post_excerpt' ] );
    }

    // ─── is_woocommerce_active ────────────────────────────────

    public function test_is_woocommerce_active_returns_true_when_class_exists(): void {
        // WooCommerce class is loaded via stubs, so it should exist.
        $this->assertTrue( ProductIntegration::is_woocommerce_active() );
    }

    // ─── enhance_product_record ───────────────────────────────

    public function test_enhance_product_record_adds_product_fields(): void {
        $post = $this->make_post( [
            'ID'        => 42,
            'post_type' => 'product',
            'post_title' => 'Blue Widget',
        ] );

        $product = new \WC_Product( 42 );
        $product->set_name( 'Blue Widget' );
        $product->set_sku( 'BW-001' );
        $product->set_price( '29.99' );
        $product->set_regular_price( '39.99' );
        $product->set_sale_price( '29.99' );
        $product->set_stock_status( 'instock' );
        $product->set_total_sales( 150 );
        $product->set_average_rating( '4.5' );
        $product->set_rating_count( 23 );
        $product->set_review_count( 18 );
        $product->set_featured( true );
        $product->set_catalog_visibility( 'visible' );
        wc_store_test_product( $product );

        $record = [ 'objectID' => '42', 'post_type' => 'product' ];
        $result = $this->integration->enhance_product_record( $record, $post );

        $this->assertSame( 'BW-001', $result['sku'] );
        $this->assertSame( 29.99, $result['price'] );
        $this->assertSame( 39.99, $result['regular_price'] );
        $this->assertSame( 29.99, $result['sale_price'] );
        $this->assertSame( 1, $result['on_sale'] );
        $this->assertSame( 'instock', $result['stock_status'] );
        $this->assertSame( 1, $result['in_stock'] );
        $this->assertSame( 150, $result['total_sales'] );
        $this->assertSame( 4.5, $result['average_rating'] );
        $this->assertSame( 23, $result['rating_count'] );
        $this->assertSame( 18, $result['review_count'] );
        $this->assertSame( 1, $result['featured'] );
        $this->assertSame( 'simple', $result['product_type'] );
        $this->assertSame( 'visible', $result['catalog_visibility'] );
    }

    public function test_enhance_product_record_skips_non_products(): void {
        $post = $this->make_post( [ 'post_type' => 'post' ] );

        $record = [ 'objectID' => '1', 'post_type' => 'post', 'post_title' => 'A Blog Post' ];
        $result = $this->integration->enhance_product_record( $record, $post );

        // Record should be unchanged — no product fields.
        $this->assertSame( $record, $result );
        $this->assertArrayNotHasKey( 'sku', $result );
        $this->assertArrayNotHasKey( 'price', $result );
    }

    public function test_enhance_product_record_returns_original_if_wc_product_not_found(): void {
        $post = $this->make_post( [
            'ID'        => 999,
            'post_type' => 'product',
        ] );

        $record = [ 'objectID' => '999', 'post_type' => 'product' ];
        $result = $this->integration->enhance_product_record( $record, $post );

        // No product in store for ID 999, so record unchanged.
        $this->assertSame( $record, $result );
    }

    public function test_enhance_product_record_marks_not_on_sale(): void {
        $post = $this->make_post( [ 'ID' => 10, 'post_type' => 'product' ] );

        $product = new \WC_Product( 10 );
        $product->set_price( '50.00' );
        $product->set_regular_price( '50.00' );
        // No sale price set.
        wc_store_test_product( $product );

        $record = [ 'objectID' => '10', 'post_type' => 'product' ];
        $result = $this->integration->enhance_product_record( $record, $post );

        $this->assertSame( 0, $result['on_sale'] );
    }

    public function test_enhance_product_record_includes_custom_attributes(): void {
        $post = $this->make_post( [ 'ID' => 20, 'post_type' => 'product' ] );

        $color_attr = new \WC_Product_Attribute();
        $color_attr->set_name( 'color' );
        $color_attr->set_options( [ 'Red', 'Blue', 'Green' ] );
        $color_attr->set_is_taxonomy( false );

        $product = new \WC_Product( 20 );
        $product->set_sku( 'ATTR-TEST' );
        $product->set_price( '10.00' );
        $product->set_regular_price( '10.00' );
        $product->set_attributes( [ $color_attr ] );
        wc_store_test_product( $product );

        $record = [ 'objectID' => '20', 'post_type' => 'product' ];
        $result = $this->integration->enhance_product_record( $record, $post );

        $this->assertArrayHasKey( 'attribute_color', $result );
        $this->assertSame( [ 'Red', 'Blue', 'Green' ], $result['attribute_color'] );
    }

    public function test_enhance_product_record_includes_variation_skus(): void {
        $post = $this->make_post( [ 'ID' => 30, 'post_type' => 'product' ] );

        // Parent variable product.
        $product = new \WC_Product_Variable( 30 );
        $product->set_sku( 'VAR-PARENT' );
        $product->set_price( '25.00' );
        $product->set_regular_price( '25.00' );
        $product->set_children( [ 31, 32 ] );
        wc_store_test_product( $product );

        // Child variations.
        $var1 = new \WC_Product( 31 );
        $var1->set_sku( 'VAR-SMALL' );
        wc_store_test_product( $var1 );

        $var2 = new \WC_Product( 32 );
        $var2->set_sku( 'VAR-LARGE' );
        wc_store_test_product( $var2 );

        $record = [ 'objectID' => '30', 'post_type' => 'product' ];
        $result = $this->integration->enhance_product_record( $record, $post );

        $this->assertArrayHasKey( 'variation_skus', $result );
        $this->assertSame( [ 'VAR-SMALL', 'VAR-LARGE' ], $result['variation_skus'] );
    }

    public function test_enhance_product_record_skips_empty_variation_skus(): void {
        $post = $this->make_post( [ 'ID' => 40, 'post_type' => 'product' ] );

        $product = new \WC_Product_Variable( 40 );
        $product->set_sku( 'VAR-EMPTY' );
        $product->set_price( '10.00' );
        $product->set_regular_price( '10.00' );
        $product->set_children( [ 41 ] );
        wc_store_test_product( $product );

        // Variation with no SKU.
        $var1 = new \WC_Product( 41 );
        $var1->set_sku( '' );
        wc_store_test_product( $var1 );

        $record = [ 'objectID' => '40', 'post_type' => 'product' ];
        $result = $this->integration->enhance_product_record( $record, $post );

        // No variation_skus key when all SKUs are empty.
        $this->assertArrayNotHasKey( 'variation_skus', $result );
    }

    public function test_enhance_product_record_out_of_stock(): void {
        $post = $this->make_post( [ 'ID' => 50, 'post_type' => 'product' ] );

        $product = new \WC_Product( 50 );
        $product->set_sku( 'OOS-001' );
        $product->set_price( '15.00' );
        $product->set_regular_price( '15.00' );
        $product->set_stock_status( 'outofstock' );
        wc_store_test_product( $product );

        $record = [ 'objectID' => '50', 'post_type' => 'product' ];
        $result = $this->integration->enhance_product_record( $record, $post );

        $this->assertSame( 0, $result['in_stock'] );
        $this->assertSame( 'outofstock', $result['stock_status'] );
    }

    // ─── filter_product_visibility ────────────────────────────

    public function test_filter_product_visibility_allows_visible_product(): void {
        $post = $this->make_post( [ 'ID' => 100, 'post_type' => 'product' ] );

        $product = new \WC_Product( 100 );
        $product->set_catalog_visibility( 'visible' );
        wc_store_test_product( $product );

        $this->assertTrue( $this->integration->filter_product_visibility( true, $post ) );
    }

    public function test_filter_product_visibility_blocks_hidden_product(): void {
        $post = $this->make_post( [ 'ID' => 101, 'post_type' => 'product' ] );

        $product = new \WC_Product( 101 );
        $product->set_catalog_visibility( 'hidden' );
        wc_store_test_product( $product );

        $this->assertFalse( $this->integration->filter_product_visibility( true, $post ) );
    }

    public function test_filter_product_visibility_allows_search_only(): void {
        $post = $this->make_post( [ 'ID' => 102, 'post_type' => 'product' ] );

        $product = new \WC_Product( 102 );
        $product->set_catalog_visibility( 'search' );
        wc_store_test_product( $product );

        $this->assertTrue( $this->integration->filter_product_visibility( true, $post ) );
    }

    public function test_filter_product_visibility_allows_catalog_only(): void {
        $post = $this->make_post( [ 'ID' => 103, 'post_type' => 'product' ] );

        $product = new \WC_Product( 103 );
        $product->set_catalog_visibility( 'catalog' );
        wc_store_test_product( $product );

        $this->assertTrue( $this->integration->filter_product_visibility( true, $post ) );
    }

    public function test_filter_product_visibility_skips_non_products(): void {
        $post = $this->make_post( [ 'post_type' => 'post' ] );
        // Non-product posts pass through unchanged.
        $this->assertTrue( $this->integration->filter_product_visibility( true, $post ) );
    }

    public function test_filter_product_visibility_blocks_unknown_product(): void {
        $post = $this->make_post( [ 'ID' => 999, 'post_type' => 'product' ] );
        // No product in store — should return false.
        $this->assertFalse( $this->integration->filter_product_visibility( true, $post ) );
    }

    // ─── enhance_index_settings ───────────────────────────────

    public function test_enhance_index_settings_adds_sku_to_searchable(): void {
        $settings = [
            'searchableAttributes' => [ 'post_title', 'post_content', 'post_excerpt' ],
            'attributesForFaceting' => [ 'filterOnly(post_type)' ],
            'customRanking' => [ 'desc(post_date)' ],
        ];

        $result = $this->integration->enhance_index_settings( $settings, 'wp_posts' );

        // SKU should be inserted right after post_title.
        $searchable = $result['searchableAttributes'];
        $this->assertContains( 'sku', $searchable );
        $this->assertContains( 'variation_skus', $searchable );

        $title_pos = array_search( 'post_title', $searchable, true );
        $sku_pos   = array_search( 'sku', $searchable, true );
        $this->assertSame( $title_pos + 1, $sku_pos, 'SKU should be right after post_title' );
    }

    public function test_enhance_index_settings_adds_product_facets(): void {
        $settings = [
            'searchableAttributes' => [ 'post_title' ],
            'attributesForFaceting' => [ 'filterOnly(post_type)' ],
            'customRanking' => [],
        ];

        $result = $this->integration->enhance_index_settings( $settings, 'wp_posts' );

        $facets = $result['attributesForFaceting'];
        $this->assertContains( 'filterOnly(price)', $facets );
        $this->assertContains( 'filterOnly(on_sale)', $facets );
        $this->assertContains( 'filterOnly(in_stock)', $facets );
        $this->assertContains( 'filterOnly(product_type)', $facets );
    }

    public function test_enhance_index_settings_adds_product_ranking(): void {
        $settings = [
            'searchableAttributes' => [ 'post_title' ],
            'attributesForFaceting' => [],
            'customRanking' => [ 'desc(post_date)' ],
        ];

        $result = $this->integration->enhance_index_settings( $settings, 'wp_posts' );

        $ranking = $result['customRanking'];
        $this->assertContains( 'desc(total_sales)', $ranking );
        $this->assertContains( 'desc(average_rating)', $ranking );
        // Original ranking preserved.
        $this->assertContains( 'desc(post_date)', $ranking );
    }

    public function test_enhance_index_settings_skips_when_products_not_indexed(): void {
        update_option( 'flapjack_post_types', [ 'post', 'page' ] );

        $settings = [
            'searchableAttributes' => [ 'post_title' ],
            'attributesForFaceting' => [ 'filterOnly(post_type)' ],
            'customRanking' => [ 'desc(post_date)' ],
        ];

        $result = $this->integration->enhance_index_settings( $settings, 'wp_posts' );

        // Settings should be unchanged.
        $this->assertSame( $settings, $result );
    }

    public function test_enhance_index_settings_does_not_duplicate_facets(): void {
        $settings = [
            'searchableAttributes' => [ 'post_title', 'sku' ],
            'attributesForFaceting' => [ 'filterOnly(post_type)', 'filterOnly(price)' ],
            'customRanking' => [ 'desc(post_date)', 'desc(total_sales)' ],
        ];

        $result = $this->integration->enhance_index_settings( $settings, 'wp_posts' );

        // Should not have duplicate entries.
        $this->assertSame(
            count( array_unique( $result['attributesForFaceting'] ) ),
            count( $result['attributesForFaceting'] ),
            'Facets should not have duplicates'
        );
        $this->assertSame(
            count( array_unique( $result['customRanking'] ) ),
            count( $result['customRanking'] ),
            'Custom ranking should not have duplicates'
        );
    }

    // ─── register ─────────────────────────────────────────────

    public function test_register_hooks_all_filters_and_actions(): void {
        global $wp_filters, $wp_actions;

        $this->integration->register();

        // Filters.
        $filter_hooks = array_keys( $wp_filters );
        $this->assertContains( 'flapjack_post_record', $filter_hooks );
        $this->assertContains( 'flapjack_should_index_post', $filter_hooks );
        $this->assertContains( 'flapjack_index_settings', $filter_hooks );

        // Actions.
        $action_hooks = array_keys( $wp_actions );
        $this->assertContains( 'woocommerce_update_product', $action_hooks );
        $this->assertContains( 'woocommerce_new_product', $action_hooks );
        $this->assertContains( 'woocommerce_delete_product', $action_hooks );
        $this->assertContains( 'woocommerce_trash_product', $action_hooks );
    }

    // ─── on_product_update ────────────────────────────────────

    public function test_on_product_update_indexes_product(): void {
        // Register filters so enhanced record includes WC fields.
        $this->integration->register();

        $post = $this->make_stored_post( [
            'ID'          => 200,
            'post_type'   => 'product',
            'post_status' => 'publish',
            'post_title'  => 'Test Product',
        ] );

        $product = new \WC_Product( 200 );
        $product->set_sku( 'UPD-001' );
        $product->set_price( '19.99' );
        $product->set_regular_price( '19.99' );
        wc_store_test_product( $product );

        // Expect saveObject to be called with enhanced record.
        $this->search_client->expects( $this->once() )
            ->method( 'saveObject' )
            ->with( 'wp_posts', $this->callback( function ( $record ) {
                return $record['objectID'] === '200'
                    && $record['sku'] === 'UPD-001'
                    && $record['price'] === 19.99;
            } ) );

        $this->integration->on_product_update( 200 );
    }

    public function test_on_product_update_handles_missing_post_gracefully(): void {
        // Post ID 888 does not exist in the store.
        // Should not throw or call saveObject.
        $this->search_client->expects( $this->never() )->method( 'saveObject' );
        $this->integration->on_product_update( 888 );
    }

    // ─── on_product_delete ────────────────────────────────────

    public function test_on_product_delete_removes_from_index(): void {
        $this->search_client->expects( $this->once() )
            ->method( 'deleteObject' )
            ->with( 'wp_posts', '300' );

        $this->integration->on_product_delete( 300 );
    }

    public function test_on_product_delete_handles_404_gracefully(): void {
        $this->search_client->method( 'deleteObject' )
            ->willThrowException( new \RuntimeException( 'Object not found (404)' ) );

        // Should not propagate the exception.
        $this->integration->on_product_delete( 301 );
        $this->assertTrue( true ); // Reached without exception.
    }

    // ─── Full integration: build_record + enhance ─────────────

    public function test_full_record_build_with_product_enhancement(): void {
        $post = $this->make_stored_post( [
            'ID'          => 500,
            'post_type'   => 'product',
            'post_status' => 'publish',
            'post_title'  => 'Premium Gadget',
            'post_content' => 'A premium electronic gadget.',
        ] );

        $product = new \WC_Product( 500 );
        $product->set_name( 'Premium Gadget' );
        $product->set_sku( 'PG-500' );
        $product->set_price( '99.99' );
        $product->set_regular_price( '129.99' );
        $product->set_sale_price( '99.99' );
        $product->set_total_sales( 42 );
        $product->set_average_rating( '4.8' );
        $product->set_rating_count( 15 );
        $product->set_review_count( 12 );
        $product->set_featured( true );
        wc_store_test_product( $product );

        // Register the filter (simulating what register() does).
        $this->integration->register();

        // Build the record through IndexManager (which calls flapjack_post_record filter).
        $record = $this->index_manager->build_record( $post );

        // Standard fields from IndexManager.
        $this->assertSame( '500', $record['objectID'] );
        $this->assertSame( 'Premium Gadget', $record['post_title'] );
        $this->assertSame( 'product', $record['post_type'] );

        // WooCommerce fields added by the filter.
        $this->assertSame( 'PG-500', $record['sku'] );
        $this->assertSame( 99.99, $record['price'] );
        $this->assertSame( 1, $record['on_sale'] );
        $this->assertSame( 42, $record['total_sales'] );
        $this->assertSame( 4.8, $record['average_rating'] );
        $this->assertSame( 1, $record['featured'] );
    }
}
