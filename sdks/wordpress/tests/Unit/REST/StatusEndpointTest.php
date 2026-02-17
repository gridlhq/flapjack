<?php
/**
 * Tests for StatusEndpoint.
 *
 * @package Flapjack\WordPress\Tests\Unit\REST
 */

declare(strict_types=1);

namespace Flapjack\WordPress\Tests\Unit\REST;

use PHPUnit\Framework\TestCase;
use PHPUnit\Framework\MockObject\MockObject;
use Flapjack\WordPress\ClientFactory;
use Flapjack\WordPress\Indexing\IndexManager;
use Flapjack\WordPress\REST\StatusEndpoint;

class StatusEndpointTest extends TestCase {

    private ClientFactory&MockObject $client_factory;
    private IndexManager&MockObject $index_manager;
    private StatusEndpoint $endpoint;

    protected function setUp(): void {
        wp_stubs_reset();

        update_option( 'flapjack_enable_search', true );
        update_option( 'flapjack_enable_instant', false );
        update_option( 'flapjack_post_types', [ 'post', 'page' ] );

        $this->client_factory = $this->createMock( ClientFactory::class );
        $this->index_manager  = $this->createMock( IndexManager::class );

        $this->endpoint = new StatusEndpoint( $this->client_factory, $this->index_manager );
    }

    public function test_handle_status_returns_plugin_info(): void {
        $this->client_factory->method( 'is_configured' )->willReturn( true );
        $this->index_manager->method( 'get_index_stats' )->willReturn( [
            'exists' => true,
            'count'  => 100,
            'name'   => 'wp_posts',
        ] );

        $request = new \WP_REST_Request( 'GET' );
        $result  = $this->endpoint->handle_status( $request );

        $this->assertInstanceOf( \WP_REST_Response::class, $result );
        $this->assertSame( 200, $result->get_status() );

        $data = $result->get_data();
        $this->assertSame( FLAPJACK_SEARCH_VERSION, $data['plugin_version'] );
        $this->assertTrue( $data['configured'] );
        $this->assertTrue( $data['search_enabled'] );
        $this->assertFalse( $data['instant_enabled'] );
        $this->assertSame( 100, $data['index']['count'] );
        $this->assertContains( 'post', $data['indexed_types'] );
        $this->assertContains( 'page', $data['indexed_types'] );
    }

    public function test_handle_status_shows_wp_post_count(): void {
        global $wp_posts_store;
        $wp_posts_store[1] = new \WP_Post( [ 'ID' => 1, 'post_type' => 'post', 'post_status' => 'publish' ] );
        $wp_posts_store[2] = new \WP_Post( [ 'ID' => 2, 'post_type' => 'post', 'post_status' => 'publish' ] );
        $wp_posts_store[3] = new \WP_Post( [ 'ID' => 3, 'post_type' => 'page', 'post_status' => 'publish' ] );

        $this->client_factory->method( 'is_configured' )->willReturn( true );
        $this->index_manager->method( 'get_index_stats' )->willReturn( [
            'exists' => true, 'count' => 3, 'name' => 'wp_posts',
        ] );

        $request = new \WP_REST_Request( 'GET' );
        $result  = $this->endpoint->handle_status( $request );
        $data    = $result->get_data();

        $this->assertSame( 3, $data['wp_post_count'] );
    }

    public function test_handle_test_connection_success(): void {
        $this->client_factory
            ->method( 'test_connection' )
            ->willReturn( [ 'success' => true, 'message' => 'Connection successful.' ] );

        $request = new \WP_REST_Request( 'POST' );
        $result  = $this->endpoint->handle_test_connection( $request );

        $this->assertSame( 200, $result->get_status() );
        $data = $result->get_data();
        $this->assertTrue( $data['success'] );
    }

    public function test_handle_test_connection_failure(): void {
        $this->client_factory
            ->method( 'test_connection' )
            ->willReturn( [ 'success' => false, 'message' => 'Connection refused.' ] );

        $request = new \WP_REST_Request( 'POST' );
        $result  = $this->endpoint->handle_test_connection( $request );

        $this->assertSame( 503, $result->get_status() );
        $data = $result->get_data();
        $this->assertFalse( $data['success'] );
    }

    public function test_register_creates_rest_routes(): void {
        global $wp_registered_rest_routes;
        $this->endpoint->register();

        // Should register 2 routes: /status (GET) and /test-connection (POST).
        $this->assertCount( 2, $wp_registered_rest_routes );

        $routes = array_map( fn( $r ) => $r['route'], $wp_registered_rest_routes );
        $this->assertContains( '/status', $routes );
        $this->assertContains( '/test-connection', $routes );
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
}
