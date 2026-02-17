<?php
/**
 * Tests for IndexEndpoint.
 *
 * @package Flapjack\WordPress\Tests\Unit\REST
 */

declare(strict_types=1);

namespace Flapjack\WordPress\Tests\Unit\REST;

use PHPUnit\Framework\TestCase;
use PHPUnit\Framework\MockObject\MockObject;
use Flapjack\WordPress\Indexing\IndexManager;
use Flapjack\WordPress\REST\IndexEndpoint;

class IndexEndpointTest extends TestCase {

    private IndexManager&MockObject $index_manager;
    private IndexEndpoint $endpoint;

    protected function setUp(): void {
        wp_stubs_reset();

        $this->index_manager = $this->createMock( IndexManager::class );
        $this->endpoint      = new IndexEndpoint( $this->index_manager );
    }

    public function test_handle_reindex_returns_success(): void {
        $this->index_manager
            ->expects( $this->once() )
            ->method( 'reindex_all' )
            ->willReturn( [ 'total' => 150, 'batches' => 1 ] );

        $request = new \WP_REST_Request( 'POST' );
        $result  = $this->endpoint->handle_reindex( $request );

        $this->assertInstanceOf( \WP_REST_Response::class, $result );
        $this->assertSame( 200, $result->get_status() );

        $data = $result->get_data();
        $this->assertTrue( $data['success'] );
        $this->assertSame( 150, $data['total'] );
        $this->assertSame( 1, $data['batches'] );
    }

    public function test_handle_reindex_returns_error_on_failure(): void {
        $this->index_manager
            ->method( 'reindex_all' )
            ->willThrowException( new \RuntimeException( 'Reindex failed' ) );

        $request = new \WP_REST_Request( 'POST' );
        $result  = $this->endpoint->handle_reindex( $request );

        $this->assertInstanceOf( \WP_Error::class, $result );
        $this->assertSame( 'flapjack_reindex_error', $result->get_error_code() );
    }

    public function test_handle_index_post_indexes_existing_post(): void {
        global $wp_posts_store;
        $post = new \WP_Post( [ 'ID' => 42, 'post_title' => 'Test', 'post_status' => 'publish', 'post_type' => 'post' ] );
        $wp_posts_store[42] = $post;

        $this->index_manager
            ->expects( $this->once() )
            ->method( 'index_post' )
            ->with( $post )
            ->willReturn( [ 'taskID' => 1 ] );

        $request = new \WP_REST_Request( 'PUT' );
        $request->set_param( 'id', 42 );

        $result = $this->endpoint->handle_index_post( $request );

        $this->assertInstanceOf( \WP_REST_Response::class, $result );
        $data = $result->get_data();
        $this->assertTrue( $data['success'] );
    }

    public function test_handle_index_post_returns_404_for_missing_post(): void {
        $request = new \WP_REST_Request( 'PUT' );
        $request->set_param( 'id', 99999 );

        $result = $this->endpoint->handle_index_post( $request );

        $this->assertInstanceOf( \WP_Error::class, $result );
        $this->assertSame( 'flapjack_post_not_found', $result->get_error_code() );
    }

    public function test_handle_index_post_returns_error_on_failure(): void {
        global $wp_posts_store;
        $post = new \WP_Post( [ 'ID' => 42, 'post_title' => 'Test', 'post_status' => 'publish', 'post_type' => 'post' ] );
        $wp_posts_store[42] = $post;

        $this->index_manager
            ->method( 'index_post' )
            ->willThrowException( new \RuntimeException( 'API error' ) );

        $request = new \WP_REST_Request( 'PUT' );
        $request->set_param( 'id', 42 );

        $result = $this->endpoint->handle_index_post( $request );

        $this->assertInstanceOf( \WP_Error::class, $result );
        $this->assertSame( 'flapjack_index_error', $result->get_error_code() );
    }

    public function test_handle_delete_post_deletes_from_index(): void {
        $this->index_manager
            ->expects( $this->once() )
            ->method( 'delete_post' )
            ->with( 42 )
            ->willReturn( [ 'taskID' => 1 ] );

        $request = new \WP_REST_Request( 'DELETE' );
        $request->set_param( 'id', 42 );

        $result = $this->endpoint->handle_delete_post( $request );

        $this->assertInstanceOf( \WP_REST_Response::class, $result );
        $data = $result->get_data();
        $this->assertTrue( $data['success'] );
    }

    public function test_handle_delete_post_returns_error_on_failure(): void {
        $this->index_manager
            ->method( 'delete_post' )
            ->willThrowException( new \RuntimeException( 'Delete failed' ) );

        $request = new \WP_REST_Request( 'DELETE' );
        $request->set_param( 'id', 42 );

        $result = $this->endpoint->handle_delete_post( $request );

        $this->assertInstanceOf( \WP_Error::class, $result );
        $this->assertSame( 'flapjack_delete_error', $result->get_error_code() );
    }

    public function test_check_permission_returns_true_for_admins(): void {
        global $wp_current_user_can;
        $wp_current_user_can = true;
        $this->assertTrue( $this->endpoint->check_permission() );
    }

    public function test_check_permission_returns_false_for_non_admins(): void {
        global $wp_current_user_can;
        $wp_current_user_can = false;
        $this->assertFalse( $this->endpoint->check_permission() );
    }

    public function test_register_creates_rest_routes(): void {
        global $wp_registered_rest_routes;
        $this->endpoint->register();

        // Should register 3 routes: /reindex, /index/:id (PUT), /index/:id (DELETE).
        $this->assertCount( 3, $wp_registered_rest_routes );

        $routes = array_map( fn( $r ) => $r['route'], $wp_registered_rest_routes );
        $this->assertContains( '/reindex', $routes );
    }
}
