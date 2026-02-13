<?php
/**
 * WooCommerce integration — enhanced product indexing and search.
 *
 * Detects WooCommerce, adds product-specific fields to index records,
 * hooks into WooCommerce product lifecycle events, and configures
 * product-optimized index settings.
 *
 * @package Flapjack\WordPress\WooCommerce
 */

declare(strict_types=1);

namespace Flapjack\WordPress\WooCommerce;

use Flapjack\WordPress\Indexing\IndexManager;

class ProductIntegration {

    private IndexManager $index_manager;

    public function __construct( IndexManager $index_manager ) {
        $this->index_manager = $index_manager;
    }

    /**
     * Check if WooCommerce is active.
     */
    public static function is_woocommerce_active(): bool {
        return class_exists( 'WooCommerce' );
    }

    /**
     * Register WooCommerce-specific hooks.
     */
    public function register(): void {
        // Enhance post records with product data.
        add_filter( 'flapjack_post_record', [ $this, 'enhance_product_record' ], 10, 2 );

        // Control product indexing visibility.
        add_filter( 'flapjack_should_index_post', [ $this, 'filter_product_visibility' ], 10, 2 );

        // Enhance index settings for product search.
        add_filter( 'flapjack_index_settings', [ $this, 'enhance_index_settings' ], 10, 2 );

        // WooCommerce-specific lifecycle hooks.
        add_action( 'woocommerce_update_product', [ $this, 'on_product_update' ] );
        add_action( 'woocommerce_new_product', [ $this, 'on_product_update' ] );
        add_action( 'woocommerce_delete_product', [ $this, 'on_product_delete' ] );
        add_action( 'woocommerce_trash_product', [ $this, 'on_product_delete' ] );
    }

    /**
     * Enhance a post record with WooCommerce product data.
     *
     * Hooked to `flapjack_post_record`. Only modifies records for products.
     *
     * @param array    $record The search record.
     * @param \WP_Post $post   The post object.
     * @return array Enhanced record.
     */
    public function enhance_product_record( array $record, \WP_Post $post ): array {
        if ( 'product' !== $post->post_type ) {
            return $record;
        }

        $product = wc_get_product( $post->ID );
        if ( ! $product ) {
            return $record;
        }

        // Core product fields.
        $record['sku']                = $product->get_sku();
        $record['price']              = (float) $product->get_price();
        $record['regular_price']      = (float) $product->get_regular_price();
        $record['sale_price']         = (float) $product->get_sale_price();
        $record['on_sale']            = $product->is_on_sale() ? 1 : 0;
        $record['stock_status']       = $product->get_stock_status();
        $record['in_stock']           = $product->is_in_stock() ? 1 : 0;
        $record['total_sales']        = (int) $product->get_total_sales();
        $record['average_rating']     = (float) $product->get_average_rating();
        $record['rating_count']       = (int) $product->get_rating_count();
        $record['review_count']       = (int) $product->get_review_count();
        $record['featured']           = $product->is_featured() ? 1 : 0;
        $record['product_type']       = $product->get_type();
        $record['catalog_visibility'] = $product->get_catalog_visibility();

        // Product attributes (color, size, etc.).
        foreach ( $product->get_attributes() as $attribute ) {
            $attr_name = $attribute->get_name();
            if ( $attribute->is_taxonomy() ) {
                $terms = get_the_terms( $post, $attr_name );
                if ( ! empty( $terms ) && ! is_wp_error( $terms ) ) {
                    $record[ 'attribute_' . sanitize_key( $attr_name ) ] = array_map(
                        fn( \WP_Term $term ) => $term->name,
                        $terms
                    );
                }
            } else {
                $record[ 'attribute_' . sanitize_key( $attr_name ) ] = $attribute->get_options();
            }
        }

        // Variable product: index child variation SKUs.
        if ( $product->is_type( 'variable' ) ) {
            $variation_skus = [];
            foreach ( $product->get_children() as $variation_id ) {
                $variation = wc_get_product( $variation_id );
                if ( $variation && $variation->get_sku() ) {
                    $variation_skus[] = $variation->get_sku();
                }
            }
            if ( ! empty( $variation_skus ) ) {
                $record['variation_skus'] = $variation_skus;
            }
        }

        return $record;
    }

    /**
     * Filter product visibility for indexing.
     *
     * Hidden products should not appear in search results.
     *
     * @param bool     $should_index Whether to index the post.
     * @param \WP_Post $post         The post object.
     * @return bool
     */
    public function filter_product_visibility( bool $should_index, \WP_Post $post ): bool {
        if ( 'product' !== $post->post_type ) {
            return $should_index;
        }

        $product = wc_get_product( $post->ID );
        if ( ! $product ) {
            return false;
        }

        // Hidden products should not be indexed.
        if ( 'hidden' === $product->get_catalog_visibility() ) {
            return false;
        }

        return $should_index;
    }

    /**
     * Enhance index settings when products are being indexed.
     *
     * @param array  $settings   The index settings.
     * @param string $index_name The index name.
     * @return array Enhanced settings.
     */
    public function enhance_index_settings( array $settings, string $index_name ): array {
        $post_types = (array) get_option( 'flapjack_post_types', [ 'post', 'page' ] );

        // Only enhance if products are being indexed.
        if ( ! in_array( 'product', $post_types, true ) ) {
            return $settings;
        }

        // Add SKU to searchable attributes (high priority — before content).
        $searchable = $settings['searchableAttributes'] ?? [];
        if ( ! in_array( 'sku', $searchable, true ) ) {
            // Insert after post_title for high relevance.
            $title_pos = array_search( 'post_title', $searchable, true );
            if ( false !== $title_pos ) {
                array_splice( $searchable, $title_pos + 1, 0, [ 'sku', 'variation_skus' ] );
            } else {
                $searchable = array_merge( [ 'sku', 'variation_skus' ], $searchable );
            }
            $settings['searchableAttributes'] = $searchable;
        }

        // Add product-specific faceting attributes.
        $facets = $settings['attributesForFaceting'] ?? [];
        $product_facets = [
            'filterOnly(price)',
            'filterOnly(on_sale)',
            'filterOnly(in_stock)',
            'filterOnly(product_type)',
            'searchable(attribute_pa_color)',
            'searchable(attribute_pa_size)',
        ];
        foreach ( $product_facets as $facet ) {
            if ( ! in_array( $facet, $facets, true ) ) {
                $facets[] = $facet;
            }
        }
        $settings['attributesForFaceting'] = $facets;

        // Add product-specific custom ranking.
        $ranking = $settings['customRanking'] ?? [];
        $product_ranking = [
            'desc(total_sales)',
            'desc(average_rating)',
        ];
        foreach ( $product_ranking as $rule ) {
            if ( ! in_array( $rule, $ranking, true ) ) {
                $ranking[] = $rule;
            }
        }
        $settings['customRanking'] = $ranking;

        return $settings;
    }

    /**
     * Handle product creation or update via WooCommerce hooks.
     *
     * @param int $product_id
     */
    public function on_product_update( int $product_id ): void {
        $post = get_post( $product_id );
        if ( ! $post instanceof \WP_Post ) {
            return;
        }

        try {
            $this->index_manager->index_post( $post );
        } catch ( \Throwable $e ) {
            if ( defined( 'WP_DEBUG' ) && WP_DEBUG ) {
                error_log( sprintf( '[Flapjack Search] Failed to index product %d: %s', $product_id, $e->getMessage() ) );
            }
        }
    }

    /**
     * Handle product deletion via WooCommerce hooks.
     *
     * @param int $product_id
     */
    public function on_product_delete( int $product_id ): void {
        try {
            $this->index_manager->delete_post( $product_id );
        } catch ( \Throwable $e ) {
            if ( defined( 'WP_DEBUG' ) && WP_DEBUG ) {
                error_log( sprintf( '[Flapjack Search] Failed to delete product %d from index: %s', $product_id, $e->getMessage() ) );
            }
        }
    }
}
