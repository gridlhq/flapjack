<?php
/**
 * WooCommerce facet configuration for InstantSearch.js.
 *
 * Generates JS-ready configuration for WooCommerce-specific InstantSearch
 * widgets: price range slider, stock filter, category refinement list,
 * on-sale toggle, and star rating filter.
 *
 * @package Flapjack\WordPress\WooCommerce
 */

declare(strict_types=1);

namespace Flapjack\WordPress\WooCommerce;

class FacetConfig {

    /**
     * Check if WooCommerce facets should be enabled.
     *
     * Requires WooCommerce active + product post type in the indexed types.
     */
    public static function is_enabled(): bool {
        if ( ! class_exists( 'WooCommerce' ) ) {
            return false;
        }
        $post_types = (array) get_option( 'flapjack_post_types', [ 'post', 'page' ] );
        return in_array( 'product', $post_types, true );
    }

    /**
     * Get the facet configuration for the frontend JS.
     *
     * @return array<string, mixed> Facet configuration keyed by widget type.
     */
    public static function get_config(): array {
        if ( ! self::is_enabled() ) {
            return [];
        }

        $currency_symbol = function_exists( 'get_woocommerce_currency_symbol' )
            ? get_woocommerce_currency_symbol()
            : '$';

        return [
            'enabled'        => true,
            'currencySymbol' => $currency_symbol,
            'widgets'        => self::get_widgets(),
        ];
    }

    /**
     * Get the list of facet widgets to render.
     *
     * @return array<int, array{type: string, attribute: string, label: string, options?: array}>
     */
    public static function get_widgets(): array {
        $widgets = [
            [
                'type'      => 'refinementList',
                'attribute' => 'taxonomy_product_cat',
                'label'     => __( 'Categories', 'flapjack-search' ),
                'options'   => [
                    'limit'         => 10,
                    'showMore'      => true,
                    'showMoreLimit' => 20,
                    'searchable'    => true,
                    'sortBy'        => [ 'count:desc', 'name:asc' ],
                ],
            ],
            [
                'type'      => 'rangeSlider',
                'attribute' => 'price',
                'label'     => __( 'Price', 'flapjack-search' ),
                'options'   => [
                    'precision' => 0,
                ],
            ],
            [
                'type'      => 'toggleRefinement',
                'attribute' => 'in_stock',
                'label'     => __( 'In Stock', 'flapjack-search' ),
                'options'   => [
                    'on'  => 1,
                    'off' => null,
                ],
            ],
            [
                'type'      => 'toggleRefinement',
                'attribute' => 'on_sale',
                'label'     => __( 'On Sale', 'flapjack-search' ),
                'options'   => [
                    'on'  => 1,
                    'off' => null,
                ],
            ],
            [
                'type'      => 'ratingMenu',
                'attribute' => 'average_rating',
                'label'     => __( 'Rating', 'flapjack-search' ),
                'options'   => [],
            ],
        ];

        /**
         * Filter the WooCommerce facet widgets displayed on the search page.
         *
         * @param array $widgets List of widget configurations.
         */
        return (array) apply_filters( 'flapjack_woocommerce_facet_widgets', $widgets );
    }

    /**
     * Get the container IDs for the facet sidebar.
     *
     * @return array<string, string> Map of widget attribute → container element ID.
     */
    public static function get_container_ids(): array {
        return [
            'taxonomy_product_cat' => 'flapjack-facet-categories',
            'price'                => 'flapjack-facet-price',
            'in_stock'             => 'flapjack-facet-stock',
            'on_sale'              => 'flapjack-facet-sale',
            'average_rating'       => 'flapjack-facet-rating',
        ];
    }
}
