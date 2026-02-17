<?php
/**
 * Tests for ClientFactory.
 *
 * @package Flapjack\WordPress\Tests\Unit
 */

declare(strict_types=1);

namespace Flapjack\WordPress\Tests\Unit;

use PHPUnit\Framework\TestCase;
use Flapjack\WordPress\ClientFactory;

class ClientFactoryTest extends TestCase {

    protected function setUp(): void {
        wp_stubs_reset();
    }

    public function test_is_configured_returns_false_when_no_credentials(): void {
        $factory = new ClientFactory();
        $this->assertFalse( $factory->is_configured() );
    }

    public function test_is_configured_returns_false_when_only_app_id_set(): void {
        update_option( 'flapjack_app_id', 'test-app-id' );
        $factory = new ClientFactory();
        $this->assertFalse( $factory->is_configured() );
    }

    public function test_is_configured_returns_false_when_only_api_key_set(): void {
        update_option( 'flapjack_api_key', 'test-api-key' );
        $factory = new ClientFactory();
        $this->assertFalse( $factory->is_configured() );
    }

    public function test_is_configured_returns_true_when_both_set(): void {
        update_option( 'flapjack_app_id', 'test-app-id' );
        update_option( 'flapjack_api_key', 'test-api-key' );
        $factory = new ClientFactory();
        $this->assertTrue( $factory->is_configured() );
    }

    public function test_get_client_throws_when_not_configured(): void {
        $factory = new ClientFactory();
        $this->expectException( \RuntimeException::class );
        $this->expectExceptionMessage( 'credentials are not configured' );
        $factory->get_client();
    }

    public function test_get_client_returns_search_client_when_configured(): void {
        update_option( 'flapjack_app_id', 'test-app-id' );
        update_option( 'flapjack_api_key', 'test-api-key' );
        update_option( 'flapjack_host', 'http://localhost:7700' );

        $factory = new ClientFactory();
        $client  = $factory->get_client();

        $this->assertInstanceOf( \Flapjack\FlapjackSearch\Api\SearchClient::class, $client );
    }

    public function test_get_client_returns_cached_instance(): void {
        update_option( 'flapjack_app_id', 'test-app-id' );
        update_option( 'flapjack_api_key', 'test-api-key' );
        update_option( 'flapjack_host', 'http://localhost:7700' );

        $factory = new ClientFactory();
        $client1 = $factory->get_client();
        $client2 = $factory->get_client();

        $this->assertSame( $client1, $client2 );
    }

    public function test_reset_clears_cached_client(): void {
        update_option( 'flapjack_app_id', 'test-app-id' );
        update_option( 'flapjack_api_key', 'test-api-key' );
        update_option( 'flapjack_host', 'http://localhost:7700' );

        $factory = new ClientFactory();
        $client1 = $factory->get_client();
        $factory->reset();
        $client2 = $factory->get_client();

        $this->assertNotSame( $client1, $client2 );
    }

    public function test_get_search_client_throws_when_not_configured(): void {
        $factory = new ClientFactory();
        $this->expectException( \RuntimeException::class );
        $factory->get_search_client();
    }

    public function test_get_search_client_throws_when_no_search_key(): void {
        update_option( 'flapjack_app_id', 'test-app-id' );
        update_option( 'flapjack_api_key', 'test-admin-key' );
        update_option( 'flapjack_host', 'http://localhost:7700' );
        // No search-only key set — must NOT fall back to admin key.

        $factory = new ClientFactory();
        $this->expectException( \RuntimeException::class );
        $this->expectExceptionMessage( 'search-only API key' );
        $factory->get_search_client();
    }

    public function test_get_search_client_uses_search_key_when_available(): void {
        update_option( 'flapjack_app_id', 'test-app-id' );
        update_option( 'flapjack_api_key', 'test-admin-key' );
        update_option( 'flapjack_search_api_key', 'test-search-key' );
        update_option( 'flapjack_host', 'http://localhost:7700' );

        $factory = new ClientFactory();
        $client  = $factory->get_search_client();

        $this->assertInstanceOf( \Flapjack\FlapjackSearch\Api\SearchClient::class, $client );
    }

    public function test_get_app_id_returns_option_value(): void {
        update_option( 'flapjack_app_id', 'my-app' );
        $factory = new ClientFactory();
        $this->assertSame( 'my-app', $factory->get_app_id() );
    }

    public function test_get_app_id_returns_empty_string_when_not_set(): void {
        $factory = new ClientFactory();
        $this->assertSame( '', $factory->get_app_id() );
    }

    public function test_get_index_name_returns_default(): void {
        $factory = new ClientFactory();
        $this->assertSame( 'wp_posts', $factory->get_index_name() );
    }

    public function test_get_index_name_returns_custom_value(): void {
        update_option( 'flapjack_index_name', 'my_custom_index' );
        $factory = new ClientFactory();
        $this->assertSame( 'my_custom_index', $factory->get_index_name() );
    }

    public function test_get_host_returns_empty_string_by_default(): void {
        $factory = new ClientFactory();
        $this->assertSame( '', $factory->get_host() );
    }

    public function test_get_host_returns_configured_value(): void {
        update_option( 'flapjack_host', 'http://search.example.com' );
        $factory = new ClientFactory();
        $this->assertSame( 'http://search.example.com', $factory->get_host() );
    }

    public function test_test_connection_returns_failure_when_not_configured(): void {
        $factory = new ClientFactory();
        $result  = $factory->test_connection();
        $this->assertFalse( $result['success'] );
        $this->assertNotEmpty( $result['message'] );
    }
}
