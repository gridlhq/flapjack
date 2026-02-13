<?php
/**
 * REST API endpoint for public search.
 *
 * @package Flapjack\WordPress\REST
 */

declare(strict_types=1);

namespace Flapjack\WordPress\REST;

use Flapjack\WordPress\ClientFactory;

class SearchEndpoint {

    private const NAMESPACE = 'flapjack-search/v1';

    private ClientFactory $client_factory;

    public function __construct( ClientFactory $client_factory ) {
        $this->client_factory = $client_factory;
    }

    /**
     * Register the REST route.
     */
    public function register(): void {
        register_rest_route( self::NAMESPACE, '/search', [
            'methods'             => 'GET',
            'callback'            => [ $this, 'handle_search' ],
            'permission_callback' => '__return_true',
            'args'                => [
                'q' => [
                    'required'          => true,
                    'type'              => 'string',
                    'sanitize_callback' => 'sanitize_text_field',
                    'validate_callback' => function ( $value ) {
                        return is_string( $value ) && strlen( $value ) > 0;
                    },
                ],
                'page' => [
                    'type'              => 'integer',
                    'default'           => 0,
                    'sanitize_callback' => 'absint',
                ],
                'per_page' => [
                    'type'              => 'integer',
                    'default'           => 20,
                    'sanitize_callback' => 'absint',
                ],
                'post_type' => [
                    'type'              => 'string',
                    'sanitize_callback' => 'sanitize_text_field',
                ],
                // JSON filter params: sanitize_text_field strips percent-encoded
                // chars which destroys URL-encoded JSON. Use wp_unslash + validate.
                'facets' => [
                    'type'              => 'string',
                    'sanitize_callback' => [ $this, 'sanitize_json_param' ],
                ],
                'facetFilters' => [
                    'type'              => 'string',
                    'sanitize_callback' => [ $this, 'sanitize_json_param' ],
                ],
                'numericFilters' => [
                    'type'              => 'string',
                    'sanitize_callback' => [ $this, 'sanitize_json_param' ],
                ],
                'tagFilters' => [
                    'type'              => 'string',
                    'sanitize_callback' => [ $this, 'sanitize_json_param' ],
                ],
                'filters' => [
                    'type'              => 'string',
                    'sanitize_callback' => 'sanitize_text_field',
                ],
            ],
        ] );
    }

    /**
     * Handle a search request.
     *
     * @param \WP_REST_Request $request
     * @return \WP_REST_Response|\WP_Error
     */
    public function handle_search( \WP_REST_Request $request ) {
        if ( ! $this->client_factory->is_configured() ) {
            return new \WP_Error(
                'flapjack_not_configured',
                __( 'Flapjack Search is not configured.', 'flapjack-search' ),
                [ 'status' => 503 ]
            );
        }

        try {
            $client = $this->client_factory->get_search_client();
            $index  = $this->client_factory->get_index_name();

            $params = [
                'query'       => $request->get_param( 'q' ),
                'hitsPerPage' => min( (int) $request->get_param( 'per_page' ), 100 ),
                'page'        => (int) $request->get_param( 'page' ),
            ];

            // Forward facet/filter params from InstantSearch widgets.
            $facets = $request->get_param( 'facets' );
            if ( ! empty( $facets ) ) {
                $decoded = json_decode( $facets, true );
                if ( is_array( $decoded ) ) {
                    $params['facets'] = $decoded;
                }
            }

            $facet_filters = $request->get_param( 'facetFilters' );
            if ( ! empty( $facet_filters ) ) {
                $decoded = json_decode( $facet_filters, true );
                if ( is_array( $decoded ) ) {
                    $params['facetFilters'] = $decoded;
                }
            }

            $numeric_filters = $request->get_param( 'numericFilters' );
            if ( ! empty( $numeric_filters ) ) {
                $decoded = json_decode( $numeric_filters, true );
                if ( is_array( $decoded ) ) {
                    $params['numericFilters'] = $decoded;
                }
            }

            $tag_filters = $request->get_param( 'tagFilters' );
            if ( ! empty( $tag_filters ) ) {
                $decoded = json_decode( $tag_filters, true );
                if ( is_array( $decoded ) ) {
                    $params['tagFilters'] = $decoded;
                }
            }

            $filters = $request->get_param( 'filters' );
            $post_type = $request->get_param( 'post_type' );
            if ( ! empty( $post_type ) ) {
                // Validate post_type is alphanumeric + underscores only to prevent filter injection.
                $post_type = sanitize_key( $post_type );
                if ( ! empty( $post_type ) ) {
                    $type_filter = 'post_type:' . $post_type;
                    $params['filters'] = ! empty( $filters ) ? $filters . ' AND ' . $type_filter : $type_filter;
                }
            } elseif ( ! empty( $filters ) ) {
                $params['filters'] = $filters;
            }

            $result = $client->searchSingleIndex( $index, $params );

            return new \WP_REST_Response( $result, 200 );
        } catch ( \Throwable $e ) {
            return new \WP_Error(
                'flapjack_search_error',
                $e->getMessage(),
                [ 'status' => 500 ]
            );
        }
    }

    /**
     * Sanitize a JSON-encoded string parameter.
     *
     * sanitize_text_field() strips percent-encoded characters (%7B, %7D etc.)
     * which destroys URL-encoded JSON. Instead we wp_unslash, validate as JSON,
     * and return the raw string (json_decode handles the actual parsing).
     *
     * @param mixed $value
     * @return string Sanitized JSON string, or empty string if invalid.
     */
    public function sanitize_json_param( $value ): string {
        if ( ! is_string( $value ) ) {
            return '';
        }
        $value = wp_unslash( $value );
        // Validate it decodes as JSON array/object.
        $decoded = json_decode( $value, true );
        if ( null === $decoded || ! is_array( $decoded ) ) {
            return '';
        }
        // Re-encode to ensure clean JSON (strips any non-JSON content).
        return (string) wp_json_encode( $decoded );
    }
}
