<?php
/**
 * Tests for QueryInterceptor.
 *
 * @package Flapjack\WordPress\Tests\Unit\Search
 */

declare(strict_types=1);

namespace Flapjack\WordPress\Tests\Unit\Search;

use PHPUnit\Framework\TestCase;
use PHPUnit\Framework\MockObject\MockObject;
use Flapjack\WordPress\ClientFactory;
use Flapjack\WordPress\Search\QueryInterceptor;
use Flapjack\FlapjackSearch\Api\SearchClient;

class QueryInterceptorTest extends TestCase {

    private ClientFactory&MockObject $client_factory;
    private SearchClient&MockObject $search_client;
    private QueryInterceptor $interceptor;

    protected function setUp(): void {
        wp_stubs_reset();

        update_option( 'flapjack_enable_search', true );
        update_option( 'flapjack_app_id', 'test-id' );
        update_option( 'flapjack_api_key', 'test-key' );
        update_option( 'flapjack_posts_per_page', 20 );

        $this->search_client  = $this->createMock( SearchClient::class );
        $this->client_factory = $this->createMock( ClientFactory::class );

        $this->client_factory->method( 'get_client' )->willReturn( $this->search_client );
        $this->client_factory->method( 'get_index_name' )->willReturn( 'wp_posts' );
        $this->client_factory->method( 'is_configured' )->willReturn( true );

        $this->interceptor = new QueryInterceptor( $this->client_factory );
    }

    // ─── Interception logic ───────────────────────────────────

    public function test_returns_null_for_non_search_query(): void {
        $query = new \WP_Query();
        $query->set_is_search( false );

        $result = $this->interceptor->intercept_search( null, $query );
        $this->assertNull( $result );
    }

    public function test_returns_null_when_bypassed(): void {
        $query = new \WP_Query( [ 's' => 'test', 'flapjack_bypass' => true ] );
        $query->set_is_search( true );

        $result = $this->interceptor->intercept_search( null, $query );
        $this->assertNull( $result );
    }

    public function test_returns_null_when_empty_search_query(): void {
        $query = new \WP_Query( [ 's' => '' ] );
        $query->set_is_search( true );

        $result = $this->interceptor->intercept_search( null, $query );
        $this->assertNull( $result );
    }

    public function test_returns_null_when_not_configured(): void {
        $factory = $this->createMock( ClientFactory::class );
        $factory->method( 'is_configured' )->willReturn( false );

        $interceptor = new QueryInterceptor( $factory );

        $query = new \WP_Query( [ 's' => 'test' ] );
        $query->set_is_search( true );

        $result = $interceptor->intercept_search( null, $query );
        $this->assertNull( $result );
    }

    // ─── Successful search ────────────────────────────────────

    public function test_intercepts_search_and_returns_posts(): void {
        global $wp_posts_store;
        $post1 = new \WP_Post( [ 'ID' => 10, 'post_title' => 'Result 1', 'post_type' => 'post', 'post_status' => 'publish' ] );
        $post2 = new \WP_Post( [ 'ID' => 20, 'post_title' => 'Result 2', 'post_type' => 'post', 'post_status' => 'publish' ] );
        $wp_posts_store[10] = $post1;
        $wp_posts_store[20] = $post2;

        $this->search_client
            ->expects( $this->once() )
            ->method( 'searchSingleIndex' )
            ->with( 'wp_posts', $this->callback( fn( $p ) => $p['query'] === 'hello' ) )
            ->willReturn( [
                'hits'        => [
                    [ 'objectID' => '10', 'post_title' => 'Result 1' ],
                    [ 'objectID' => '20', 'post_title' => 'Result 2' ],
                ],
                'nbHits'      => 2,
                'hitsPerPage' => 20,
            ] );

        $query = new \WP_Query( [ 's' => 'hello', 'posts_per_page' => 20, 'paged' => 1 ] );
        $query->set_is_search( true );

        $result = $this->interceptor->intercept_search( null, $query );

        $this->assertIsArray( $result );
        $this->assertCount( 2, $result );
        $this->assertSame( 2, $query->found_posts );
        $this->assertSame( 1, $query->max_num_pages );
    }

    public function test_returns_empty_array_for_no_hits(): void {
        $this->search_client
            ->method( 'searchSingleIndex' )
            ->willReturn( [ 'hits' => [], 'nbHits' => 0 ] );

        $query = new \WP_Query( [ 's' => 'nonexistent', 'posts_per_page' => 20, 'paged' => 1 ] );
        $query->set_is_search( true );

        $result = $this->interceptor->intercept_search( null, $query );

        $this->assertIsArray( $result );
        $this->assertEmpty( $result );
        $this->assertSame( 0, $query->found_posts );
        $this->assertSame( 0, $query->max_num_pages );
    }

    // ─── Pagination ───────────────────────────────────────────

    public function test_passes_correct_pagination_params(): void {
        $this->search_client
            ->expects( $this->once() )
            ->method( 'searchSingleIndex' )
            ->with( 'wp_posts', $this->callback( function ( $params ) {
                return $params['page'] === 2 && $params['hitsPerPage'] === 10;
            } ) )
            ->willReturn( [ 'hits' => [], 'nbHits' => 0 ] );

        $query = new \WP_Query( [ 's' => 'test', 'paged' => 3, 'posts_per_page' => 10 ] );
        $query->set_is_search( true );

        $this->interceptor->intercept_search( null, $query );
    }

    public function test_page_zero_for_first_page(): void {
        $this->search_client
            ->expects( $this->once() )
            ->method( 'searchSingleIndex' )
            ->with( 'wp_posts', $this->callback( fn( $p ) => $p['page'] === 0 ) )
            ->willReturn( [ 'hits' => [], 'nbHits' => 0 ] );

        $query = new \WP_Query( [ 's' => 'test', 'paged' => 1, 'posts_per_page' => 20 ] );
        $query->set_is_search( true );

        $this->interceptor->intercept_search( null, $query );
    }

    // ─── Post type filtering ──────────────────────────────────

    public function test_filters_by_single_post_type(): void {
        $this->search_client
            ->expects( $this->once() )
            ->method( 'searchSingleIndex' )
            ->with( 'wp_posts', $this->callback( fn( $p ) => $p['filters'] === 'post_type:page' ) )
            ->willReturn( [ 'hits' => [], 'nbHits' => 0 ] );

        $query = new \WP_Query( [ 's' => 'test', 'post_type' => 'page', 'paged' => 1, 'posts_per_page' => 20 ] );
        $query->set_is_search( true );

        $this->interceptor->intercept_search( null, $query );
    }

    public function test_filters_by_multiple_post_types(): void {
        $this->search_client
            ->expects( $this->once() )
            ->method( 'searchSingleIndex' )
            ->with( 'wp_posts', $this->callback( function ( $p ) {
                return isset( $p['filters'] )
                    && str_contains( $p['filters'], 'post_type:post' )
                    && str_contains( $p['filters'], 'post_type:page' )
                    && str_contains( $p['filters'], ' OR ' );
            } ) )
            ->willReturn( [ 'hits' => [], 'nbHits' => 0 ] );

        $query = new \WP_Query( [ 's' => 'test', 'post_type' => [ 'post', 'page' ], 'paged' => 1, 'posts_per_page' => 20 ] );
        $query->set_is_search( true );

        $this->interceptor->intercept_search( null, $query );
    }

    public function test_no_filter_for_any_post_type(): void {
        $this->search_client
            ->expects( $this->once() )
            ->method( 'searchSingleIndex' )
            ->with( 'wp_posts', $this->callback( fn( $p ) => ! isset( $p['filters'] ) ) )
            ->willReturn( [ 'hits' => [], 'nbHits' => 0 ] );

        $query = new \WP_Query( [ 's' => 'test', 'post_type' => 'any', 'paged' => 1, 'posts_per_page' => 20 ] );
        $query->set_is_search( true );

        $this->interceptor->intercept_search( null, $query );
    }

    // ─── Fallback on error ────────────────────────────────────

    public function test_falls_back_to_native_search_on_api_error(): void {
        $this->search_client
            ->method( 'searchSingleIndex' )
            ->willThrowException( new \RuntimeException( 'API unavailable' ) );

        $query = new \WP_Query( [ 's' => 'test', 'paged' => 1, 'posts_per_page' => 20 ] );
        $query->set_is_search( true );

        $result = $this->interceptor->intercept_search( null, $query );

        // Null means WP_Query proceeds with its own SQL.
        $this->assertNull( $result );
    }

    // ─── Filter hook ──────────────────────────────────────────

    public function test_search_params_filter_is_applied(): void {
        add_filter( 'flapjack_search_params', function ( array $params ) {
            $params['facets'] = [ 'post_type' ];
            return $params;
        } );

        $this->search_client
            ->expects( $this->once() )
            ->method( 'searchSingleIndex' )
            ->with( 'wp_posts', $this->callback( fn( $p ) => isset( $p['facets'] ) && $p['facets'] === [ 'post_type' ] ) )
            ->willReturn( [ 'hits' => [], 'nbHits' => 0 ] );

        $query = new \WP_Query( [ 's' => 'test', 'paged' => 1, 'posts_per_page' => 20 ] );
        $query->set_is_search( true );

        $this->interceptor->intercept_search( null, $query );
    }

    // ─── execute_search directly ──────────────────────────────

    public function test_execute_search_returns_api_result(): void {
        $expected = [
            'hits'   => [ [ 'objectID' => '1', 'post_title' => 'Hello' ] ],
            'nbHits' => 1,
        ];

        $this->search_client
            ->method( 'searchSingleIndex' )
            ->willReturn( $expected );

        $query  = new \WP_Query( [ 'paged' => 1, 'posts_per_page' => 20 ] );
        $result = $this->interceptor->execute_search( 'hello', $query );

        $this->assertSame( $expected, $result );
    }

    public function test_default_per_page_when_query_has_zero(): void {
        $this->search_client
            ->expects( $this->once() )
            ->method( 'searchSingleIndex' )
            ->with( 'wp_posts', $this->callback( fn( $p ) => $p['hitsPerPage'] === 20 ) )
            ->willReturn( [ 'hits' => [], 'nbHits' => 0 ] );

        $query = new \WP_Query( [ 'paged' => 1, 'posts_per_page' => 0 ] );
        $this->interceptor->execute_search( 'test', $query );
    }
}
