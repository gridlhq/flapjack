<?php
/**
 * Tests for CLI\Commands.
 *
 * @package Flapjack\WordPress\Tests\Unit\CLI
 */

declare(strict_types=1);

namespace Flapjack\WordPress\Tests\Unit\CLI;

use PHPUnit\Framework\TestCase;
use PHPUnit\Framework\MockObject\MockObject;
use Flapjack\WordPress\CLI\Commands;
use Flapjack\WordPress\ClientFactory;
use Flapjack\WordPress\Indexing\IndexManager;

class CommandsTest extends TestCase {

    private ClientFactory&MockObject $client_factory;
    private IndexManager&MockObject $index_manager;
    private Commands $commands;

    protected function setUp(): void {
        wp_stubs_reset();
        \WP_CLI::reset();

        $this->client_factory = $this->createMock( ClientFactory::class );
        $this->index_manager  = $this->createMock( IndexManager::class );
        $this->commands       = new Commands( $this->client_factory, $this->index_manager );
    }

    /**
     * Helper: call a method that may trigger WP_CLI::error() (which throws).
     * Returns the captured messages regardless.
     */
    private function call_catching_error( callable $fn ): array {
        try {
            $fn();
        } catch ( \RuntimeException $e ) {
            // WP_CLI::error() throws — expected for error paths.
        }
        return \WP_CLI::$captured;
    }

    // ─── register() ──────────────────────────────────────────

    public function test_register_adds_all_wp_cli_commands(): void {
        Commands::register( $this->client_factory, $this->index_manager );
        $commands = \WP_CLI::$registered_commands;
        $this->assertCount( 6, $commands );
        $names = array_column( $commands, 'name' );
        $this->assertContains( 'flapjack reindex', $names );
        $this->assertContains( 'flapjack status', $names );
        $this->assertContains( 'flapjack index', $names );
        $this->assertContains( 'flapjack delete', $names );
        $this->assertContains( 'flapjack test', $names );
        $this->assertContains( 'flapjack search', $names );
    }

    // ─── reindex() ───────────────────────────────────────────

    public function test_reindex_success(): void {
        $this->client_factory
            ->method( 'is_configured' )
            ->willReturn( true );

        $this->index_manager
            ->expects( $this->once() )
            ->method( 'reindex_all' )
            ->willReturn( [ 'total' => 150, 'batches' => 1 ] );

        $this->commands->reindex( [], [] );

        $messages = \WP_CLI::$captured;
        $this->assertCount( 2, $messages );
        $this->assertSame( 'log', $messages[0]['type'] );
        $this->assertStringContainsString( 'Starting full reindex', $messages[0]['message'] );
        $this->assertSame( 'success', $messages[1]['type'] );
        $this->assertStringContainsString( '150', $messages[1]['message'] );
    }

    public function test_reindex_not_configured(): void {
        $this->client_factory
            ->method( 'is_configured' )
            ->willReturn( false );

        $messages = $this->call_catching_error( fn() => $this->commands->reindex( [], [] ) );

        $last = end( $messages );
        $this->assertSame( 'error', $last['type'] );
        $this->assertStringContainsString( 'not configured', $last['message'] );
    }

    public function test_reindex_api_error(): void {
        $this->client_factory
            ->method( 'is_configured' )
            ->willReturn( true );

        $this->index_manager
            ->method( 'reindex_all' )
            ->willThrowException( new \RuntimeException( 'Connection refused' ) );

        $messages = $this->call_catching_error( fn() => $this->commands->reindex( [], [] ) );

        $last = end( $messages );
        $this->assertSame( 'error', $last['type'] );
        $this->assertStringContainsString( 'Connection refused', $last['message'] );
    }

    // ─── status() ────────────────────────────────────────────

    public function test_status_outputs_configuration_info(): void {
        \cli\Table::reset();

        $this->client_factory
            ->method( 'is_configured' )
            ->willReturn( true );
        $this->client_factory
            ->method( 'get_app_id' )
            ->willReturn( 'test-app' );
        $this->client_factory
            ->method( 'get_host' )
            ->willReturn( '' );
        $this->client_factory
            ->method( 'get_index_name' )
            ->willReturn( 'wp_posts' );

        $this->index_manager
            ->method( 'get_index_stats' )
            ->willReturn( [ 'exists' => true, 'count' => 42, 'name' => 'wp_posts' ] );

        update_option( 'flapjack_post_types', [ 'post', 'page' ] );
        update_option( 'flapjack_enable_search', true );
        update_option( 'flapjack_enable_instant', false );

        $this->commands->status( [], [] );

        // Status output goes through cli\Table — verify its rows.
        $table = \cli\Table::$last_instance;
        $this->assertNotNull( $table );
        $this->assertSame( [ 'Setting', 'Value' ], $table->headers );
        // Flatten all row values and check for expected content.
        $all_values = implode( ' ', array_merge( ...array_map( 'array_values', $table->rows ) ) );
        $this->assertStringContainsString( 'test-app', $all_values );
        $this->assertStringContainsString( 'Yes', $all_values );
        $this->assertStringContainsString( '42', $all_values );
    }

    public function test_status_shows_not_configured_when_no_credentials(): void {
        \cli\Table::reset();

        $this->client_factory
            ->method( 'is_configured' )
            ->willReturn( false );
        $this->client_factory
            ->method( 'get_app_id' )
            ->willReturn( '' );
        $this->client_factory
            ->method( 'get_host' )
            ->willReturn( '' );
        $this->client_factory
            ->method( 'get_index_name' )
            ->willReturn( 'wp_posts' );

        update_option( 'flapjack_post_types', [ 'post' ] );

        $this->commands->status( [], [] );

        $table = \cli\Table::$last_instance;
        $this->assertNotNull( $table );
        $all_values = implode( ' ', array_merge( ...array_map( 'array_values', $table->rows ) ) );
        $this->assertStringContainsString( 'No', $all_values );
    }

    // ─── index_post() ────────────────────────────────────────

    public function test_index_post_success(): void {
        global $wp_posts_store;
        $post = new \WP_Post( [ 'ID' => 42, 'post_title' => 'Test' ] );
        $wp_posts_store[42] = $post;

        $this->index_manager
            ->expects( $this->once() )
            ->method( 'index_post' )
            ->with( $post );

        $this->commands->index_post( [ '42' ], [] );

        $messages = \WP_CLI::$captured;
        $last     = end( $messages );
        $this->assertSame( 'success', $last['type'] );
        $this->assertStringContainsString( '42', $last['message'] );
    }

    public function test_index_post_not_found(): void {
        $messages = $this->call_catching_error( fn() => $this->commands->index_post( [ '999' ], [] ) );

        $last = end( $messages );
        $this->assertSame( 'error', $last['type'] );
        $this->assertStringContainsString( 'not found', strtolower( $last['message'] ) );
    }

    public function test_index_post_api_error(): void {
        global $wp_posts_store;
        $post = new \WP_Post( [ 'ID' => 42 ] );
        $wp_posts_store[42] = $post;

        $this->index_manager
            ->method( 'index_post' )
            ->willThrowException( new \RuntimeException( 'API timeout' ) );

        $messages = $this->call_catching_error( fn() => $this->commands->index_post( [ '42' ], [] ) );

        $last = end( $messages );
        $this->assertSame( 'error', $last['type'] );
        $this->assertStringContainsString( 'API timeout', $last['message'] );
    }

    // ─── delete_post() ───────────────────────────────────────

    public function test_delete_post_success(): void {
        $this->index_manager
            ->expects( $this->once() )
            ->method( 'delete_post' )
            ->with( 42 );

        $this->commands->delete_post( [ '42' ], [] );

        $messages = \WP_CLI::$captured;
        $last     = end( $messages );
        $this->assertSame( 'success', $last['type'] );
        $this->assertStringContainsString( '42', $last['message'] );
    }

    public function test_delete_post_api_error(): void {
        $this->index_manager
            ->method( 'delete_post' )
            ->willThrowException( new \RuntimeException( 'Network error' ) );

        $messages = $this->call_catching_error( fn() => $this->commands->delete_post( [ '42' ], [] ) );

        $last = end( $messages );
        $this->assertSame( 'error', $last['type'] );
        $this->assertStringContainsString( 'Network error', $last['message'] );
    }

    // ─── test_connection() ───────────────────────────────────

    public function test_test_connection_success(): void {
        $this->client_factory
            ->method( 'test_connection' )
            ->willReturn( [ 'success' => true, 'message' => 'Connection successful.' ] );

        $this->commands->test_connection( [], [] );

        $messages = \WP_CLI::$captured;
        $last     = end( $messages );
        $this->assertSame( 'success', $last['type'] );
        $this->assertStringContainsString( 'Connection successful', $last['message'] );
    }

    public function test_test_connection_failure(): void {
        $this->client_factory
            ->method( 'test_connection' )
            ->willReturn( [ 'success' => false, 'message' => 'Connection refused' ] );

        $messages = $this->call_catching_error( fn() => $this->commands->test_connection( [], [] ) );

        $last = end( $messages );
        $this->assertSame( 'error', $last['type'] );
        $this->assertStringContainsString( 'Connection refused', $last['message'] );
    }

    // ─── search() ────────────────────────────────────────────

    public function test_search_with_results(): void {
        $mock_client = $this->createMock( \Flapjack\FlapjackSearch\Api\SearchClient::class );
        $mock_client
            ->method( 'searchSingleIndex' )
            ->willReturn( [
                'hits'   => [
                    [ 'objectID' => '1', 'post_title' => 'Hello World', 'permalink' => 'https://example.com/?p=1' ],
                    [ 'objectID' => '2', 'post_title' => 'Test Post', 'permalink' => 'https://example.com/?p=2' ],
                ],
                'nbHits' => 2,
            ] );

        $this->client_factory
            ->method( 'get_client' )
            ->willReturn( $mock_client );
        $this->client_factory
            ->method( 'get_index_name' )
            ->willReturn( 'wp_posts' );

        $this->commands->search( [ 'hello' ], [] );

        $messages = \WP_CLI::$captured;
        $this->assertGreaterThanOrEqual( 3, count( $messages ) );
        $this->assertStringContainsString( 'Found 2 results', $messages[0]['message'] );
    }

    public function test_search_no_results(): void {
        $mock_client = $this->createMock( \Flapjack\FlapjackSearch\Api\SearchClient::class );
        $mock_client
            ->method( 'searchSingleIndex' )
            ->willReturn( [ 'hits' => [], 'nbHits' => 0 ] );

        $this->client_factory
            ->method( 'get_client' )
            ->willReturn( $mock_client );
        $this->client_factory
            ->method( 'get_index_name' )
            ->willReturn( 'wp_posts' );

        $this->commands->search( [ 'nonexistent' ], [] );

        $messages = \WP_CLI::$captured;
        $last     = end( $messages );
        $this->assertSame( 'log', $last['type'] );
        $this->assertStringContainsString( 'No results', $last['message'] );
    }

    public function test_search_with_per_page_option(): void {
        $mock_client = $this->createMock( \Flapjack\FlapjackSearch\Api\SearchClient::class );
        $mock_client
            ->expects( $this->once() )
            ->method( 'searchSingleIndex' )
            ->with(
                'wp_posts',
                $this->callback( function ( $params ) {
                    return $params['hitsPerPage'] === 5;
                } )
            )
            ->willReturn( [ 'hits' => [], 'nbHits' => 0 ] );

        $this->client_factory
            ->method( 'get_client' )
            ->willReturn( $mock_client );
        $this->client_factory
            ->method( 'get_index_name' )
            ->willReturn( 'wp_posts' );

        $this->commands->search( [ 'test' ], [ 'per-page' => '5' ] );
    }

    public function test_search_api_error(): void {
        $this->client_factory
            ->method( 'get_client' )
            ->willThrowException( new \RuntimeException( 'Not configured' ) );

        $messages = $this->call_catching_error( fn() => $this->commands->search( [ 'test' ], [] ) );

        $last = end( $messages );
        $this->assertSame( 'error', $last['type'] );
    }
}
