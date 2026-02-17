<?php
/**
 * Tests for SearchEndpoint.
 *
 * @package Flapjack\WordPress\Tests\Unit\REST
 */

declare(strict_types=1);

namespace Flapjack\WordPress\Tests\Unit\REST;

use PHPUnit\Framework\TestCase;
use PHPUnit\Framework\MockObject\MockObject;
use Flapjack\WordPress\ClientFactory;
use Flapjack\WordPress\REST\SearchEndpoint;
use Flapjack\FlapjackSearch\Api\SearchClient;

class SearchEndpointTest extends TestCase {

    private ClientFactory&MockObject $client_factory;
    private SearchClient&MockObject $search_client;
    private SearchEndpoint $endpoint;

    protected function setUp(): void {
        wp_stubs_reset();

        $this->search_client  = $this->createMock( SearchClient::class );
        $this->client_factory = $this->createMock( ClientFactory::class );

        $this->client_factory->method( 'get_search_client' )->willReturn( $this->search_client );
        $this->client_factory->method( 'get_index_name' )->willReturn( 'wp_posts' );

        $this->endpoint = new SearchEndpoint( $this->client_factory );
    }

    public function test_returns_error_when_not_configured(): void {
        $this->client_factory = $this->createMock( ClientFactory::class );
        $this->client_factory->method( 'is_configured' )->willReturn( false );

        $endpoint = new SearchEndpoint( $this->client_factory );
        $request  = new \WP_REST_Request( 'GET' );
        $request->set_param( 'q', 'test' );

        $result = $endpoint->handle_search( $request );

        $this->assertInstanceOf( \WP_Error::class, $result );
        $this->assertSame( 'flapjack_not_configured', $result->get_error_code() );
    }

    public function test_returns_search_results(): void {
        $this->client_factory->method( 'is_configured' )->willReturn( true );

        $api_response = [
            'hits'   => [
                [ 'objectID' => '1', 'post_title' => 'Hello World' ],
            ],
            'nbHits' => 1,
        ];

        $this->search_client
            ->expects( $this->once() )
            ->method( 'searchSingleIndex' )
            ->with( 'wp_posts', $this->callback( fn( $p ) => $p['query'] === 'hello' ) )
            ->willReturn( $api_response );

        $request = new \WP_REST_Request( 'GET' );
        $request->set_param( 'q', 'hello' );
        $request->set_param( 'per_page', 20 );
        $request->set_param( 'page', 0 );

        $result = $this->endpoint->handle_search( $request );

        $this->assertInstanceOf( \WP_REST_Response::class, $result );
        $this->assertSame( 200, $result->get_status() );

        $data = $result->get_data();
        $this->assertCount( 1, $data['hits'] );
        $this->assertSame( 'Hello World', $data['hits'][0]['post_title'] );
    }

    public function test_caps_per_page_at_100(): void {
        $this->client_factory->method( 'is_configured' )->willReturn( true );

        $this->search_client
            ->expects( $this->once() )
            ->method( 'searchSingleIndex' )
            ->with( 'wp_posts', $this->callback( fn( $p ) => $p['hitsPerPage'] === 100 ) )
            ->willReturn( [ 'hits' => [], 'nbHits' => 0 ] );

        $request = new \WP_REST_Request( 'GET' );
        $request->set_param( 'q', 'test' );
        $request->set_param( 'per_page', 500 );
        $request->set_param( 'page', 0 );

        $this->endpoint->handle_search( $request );
    }

    public function test_applies_post_type_filter(): void {
        $this->client_factory->method( 'is_configured' )->willReturn( true );

        $this->search_client
            ->expects( $this->once() )
            ->method( 'searchSingleIndex' )
            ->with( 'wp_posts', $this->callback( fn( $p ) => $p['filters'] === 'post_type:page' ) )
            ->willReturn( [ 'hits' => [], 'nbHits' => 0 ] );

        $request = new \WP_REST_Request( 'GET' );
        $request->set_param( 'q', 'test' );
        $request->set_param( 'post_type', 'page' );
        $request->set_param( 'per_page', 20 );
        $request->set_param( 'page', 0 );

        $this->endpoint->handle_search( $request );
    }

    public function test_returns_error_on_api_exception(): void {
        $this->client_factory->method( 'is_configured' )->willReturn( true );

        $this->search_client
            ->method( 'searchSingleIndex' )
            ->willThrowException( new \RuntimeException( 'Connection failed' ) );

        $request = new \WP_REST_Request( 'GET' );
        $request->set_param( 'q', 'test' );
        $request->set_param( 'per_page', 20 );
        $request->set_param( 'page', 0 );

        $result = $this->endpoint->handle_search( $request );

        $this->assertInstanceOf( \WP_Error::class, $result );
        $this->assertSame( 'flapjack_search_error', $result->get_error_code() );
    }

    public function test_no_filter_when_post_type_not_specified(): void {
        $this->client_factory->method( 'is_configured' )->willReturn( true );

        $this->search_client
            ->expects( $this->once() )
            ->method( 'searchSingleIndex' )
            ->with( 'wp_posts', $this->callback( fn( $p ) => ! isset( $p['filters'] ) ) )
            ->willReturn( [ 'hits' => [], 'nbHits' => 0 ] );

        $request = new \WP_REST_Request( 'GET' );
        $request->set_param( 'q', 'test' );
        $request->set_param( 'per_page', 20 );
        $request->set_param( 'page', 0 );

        $this->endpoint->handle_search( $request );
    }

    public function test_register_creates_search_route(): void {
        global $wp_registered_rest_routes;
        $this->endpoint->register();

        $this->assertCount( 1, $wp_registered_rest_routes );
        $route = $wp_registered_rest_routes[0];
        $this->assertSame( 'flapjack-search/v1', $route['namespace'] );
        $this->assertSame( '/search', $route['route'] );
    }

    public function test_forwards_facet_filters_to_search_backend(): void {
        $this->client_factory->method( 'is_configured' )->willReturn( true );

        $this->search_client
            ->expects( $this->once() )
            ->method( 'searchSingleIndex' )
            ->with( 'wp_posts', $this->callback( function ( $p ) {
                return isset( $p['facetFilters'] )
                    && $p['facetFilters'] === [ [ 'taxonomy_product_cat:Electronics' ] ];
            } ) )
            ->willReturn( [ 'hits' => [], 'nbHits' => 0 ] );

        $request = new \WP_REST_Request( 'GET' );
        $request->set_param( 'q', 'laptop' );
        $request->set_param( 'per_page', 20 );
        $request->set_param( 'page', 0 );
        $request->set_param( 'facetFilters', json_encode( [ [ 'taxonomy_product_cat:Electronics' ] ] ) );

        $this->endpoint->handle_search( $request );
    }

    public function test_forwards_numeric_filters_to_search_backend(): void {
        $this->client_factory->method( 'is_configured' )->willReturn( true );

        $this->search_client
            ->expects( $this->once() )
            ->method( 'searchSingleIndex' )
            ->with( 'wp_posts', $this->callback( function ( $p ) {
                return isset( $p['numericFilters'] )
                    && $p['numericFilters'] === [ 'price>=10', 'price<=500' ];
            } ) )
            ->willReturn( [ 'hits' => [], 'nbHits' => 0 ] );

        $request = new \WP_REST_Request( 'GET' );
        $request->set_param( 'q', 'shoes' );
        $request->set_param( 'per_page', 20 );
        $request->set_param( 'page', 0 );
        $request->set_param( 'numericFilters', json_encode( [ 'price>=10', 'price<=500' ] ) );

        $this->endpoint->handle_search( $request );
    }

    public function test_forwards_facets_param_to_search_backend(): void {
        $this->client_factory->method( 'is_configured' )->willReturn( true );

        $this->search_client
            ->expects( $this->once() )
            ->method( 'searchSingleIndex' )
            ->with( 'wp_posts', $this->callback( function ( $p ) {
                return isset( $p['facets'] )
                    && $p['facets'] === [ 'taxonomy_product_cat', 'price' ];
            } ) )
            ->willReturn( [ 'hits' => [], 'nbHits' => 0 ] );

        $request = new \WP_REST_Request( 'GET' );
        $request->set_param( 'q', 'test' );
        $request->set_param( 'per_page', 20 );
        $request->set_param( 'page', 0 );
        $request->set_param( 'facets', json_encode( [ 'taxonomy_product_cat', 'price' ] ) );

        $this->endpoint->handle_search( $request );
    }

    public function test_combines_post_type_filter_with_user_filters(): void {
        $this->client_factory->method( 'is_configured' )->willReturn( true );

        $this->search_client
            ->expects( $this->once() )
            ->method( 'searchSingleIndex' )
            ->with( 'wp_posts', $this->callback( function ( $p ) {
                return isset( $p['filters'] )
                    && $p['filters'] === 'in_stock:1 AND post_type:product';
            } ) )
            ->willReturn( [ 'hits' => [], 'nbHits' => 0 ] );

        $request = new \WP_REST_Request( 'GET' );
        $request->set_param( 'q', 'test' );
        $request->set_param( 'per_page', 20 );
        $request->set_param( 'page', 0 );
        $request->set_param( 'post_type', 'product' );
        $request->set_param( 'filters', 'in_stock:1' );

        $this->endpoint->handle_search( $request );
    }

    public function test_ignores_invalid_json_in_facet_filters(): void {
        $this->client_factory->method( 'is_configured' )->willReturn( true );

        $this->search_client
            ->expects( $this->once() )
            ->method( 'searchSingleIndex' )
            ->with( 'wp_posts', $this->callback( function ( $p ) {
                return ! isset( $p['facetFilters'] );
            } ) )
            ->willReturn( [ 'hits' => [], 'nbHits' => 0 ] );

        $request = new \WP_REST_Request( 'GET' );
        $request->set_param( 'q', 'test' );
        $request->set_param( 'per_page', 20 );
        $request->set_param( 'page', 0 );
        $request->set_param( 'facetFilters', 'not-valid-json{' );

        $this->endpoint->handle_search( $request );
    }

    public function test_search_uses_search_client_not_admin_client(): void {
        $this->client_factory->method( 'is_configured' )->willReturn( true );

        // get_search_client should be used (not get_client).
        $this->client_factory
            ->expects( $this->once() )
            ->method( 'get_search_client' )
            ->willReturn( $this->search_client );

        $this->search_client
            ->method( 'searchSingleIndex' )
            ->willReturn( [ 'hits' => [], 'nbHits' => 0 ] );

        $request = new \WP_REST_Request( 'GET' );
        $request->set_param( 'q', 'test' );
        $request->set_param( 'per_page', 20 );
        $request->set_param( 'page', 0 );

        $this->endpoint->handle_search( $request );
    }

    // ─── JSON param sanitization ────────────────────────────

    public function test_sanitize_json_param_returns_valid_json(): void {
        $input  = '["taxonomy_product_cat:Electronics"]';
        $result = $this->endpoint->sanitize_json_param( $input );

        $this->assertSame( '["taxonomy_product_cat:Electronics"]', $result );
    }

    public function test_sanitize_json_param_rejects_invalid_json(): void {
        $result = $this->endpoint->sanitize_json_param( 'not{valid' );
        $this->assertSame( '', $result );
    }

    public function test_sanitize_json_param_rejects_non_array_json(): void {
        // Scalar JSON values should be rejected (only arrays/objects allowed).
        $result = $this->endpoint->sanitize_json_param( '"just a string"' );
        $this->assertSame( '', $result );
    }

    public function test_sanitize_json_param_rejects_non_string_input(): void {
        $result = $this->endpoint->sanitize_json_param( 123 );
        $this->assertSame( '', $result );
    }

    public function test_sanitize_json_param_handles_nested_arrays(): void {
        $input  = '[["color:red","color:blue"],["size:large"]]';
        $result = $this->endpoint->sanitize_json_param( $input );
        $decoded = json_decode( $result, true );

        $this->assertSame( [ [ 'color:red', 'color:blue' ], [ 'size:large' ] ], $decoded );
    }

    public function test_sanitize_json_param_strips_xss_from_values(): void {
        // JSON with HTML inside — should survive re-encoding without script execution.
        $input  = '["<script>alert(1)</script>"]';
        $result = $this->endpoint->sanitize_json_param( $input );
        $decoded = json_decode( $result, true );

        // Value is preserved as string data, not executed.
        $this->assertSame( [ '<script>alert(1)</script>' ], $decoded );
    }

    public function test_post_type_filter_injection_blocked(): void {
        $this->client_factory->method( 'is_configured' )->willReturn( true );

        $this->search_client
            ->expects( $this->once() )
            ->method( 'searchSingleIndex' )
            ->with( 'wp_posts', $this->callback( function ( $p ) {
                // sanitize_key strips colons, spaces, and uppercases — the injected
                // filter syntax (OR, colons) is destroyed even though the letters remain.
                // "post OR admin:1" → "post_type:postoradmin1" (harmless literal value).
                return isset( $p['filters'] )
                    && ! str_contains( $p['filters'], ' OR ' )
                    && ! str_contains( $p['filters'], 'admin:' );
            } ) )
            ->willReturn( [ 'hits' => [], 'nbHits' => 0 ] );

        $request = new \WP_REST_Request( 'GET' );
        $request->set_param( 'q', 'test' );
        $request->set_param( 'per_page', 20 );
        $request->set_param( 'page', 0 );
        // Attempt filter injection via post_type.
        $request->set_param( 'post_type', 'post OR admin:1' );

        $this->endpoint->handle_search( $request );
    }
}
