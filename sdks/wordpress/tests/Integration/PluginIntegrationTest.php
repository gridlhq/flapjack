<?php
/**
 * Integration tests — exercises multiple plugin components together
 * to verify the full search/index lifecycle.
 *
 * Unlike unit tests (which test classes in isolation), these tests wire up
 * real plugin classes together with only the Flapjack SearchClient mocked.
 *
 * @package Flapjack\WordPress\Tests\Integration
 */

declare(strict_types=1);

namespace Flapjack\WordPress\Tests\Integration;

use PHPUnit\Framework\TestCase;
use PHPUnit\Framework\MockObject\MockObject;
use Flapjack\FlapjackSearch\Api\SearchClient;
use Flapjack\WordPress\ClientFactory;
use Flapjack\WordPress\Indexing\IndexManager;
use Flapjack\WordPress\Indexing\PostSyncHooks;
use Flapjack\WordPress\Search\QueryInterceptor;
use Flapjack\WordPress\REST\SearchEndpoint;
use Flapjack\WordPress\REST\IndexEndpoint;
use Flapjack\WordPress\REST\StatusEndpoint;
use Flapjack\WordPress\Tests\Traits\MakesTestPosts;

class PluginIntegrationTest extends TestCase {

    use MakesTestPosts;

    private SearchClient&MockObject $mock_client;
    private ClientFactory&MockObject $client_factory;
    private IndexManager $index_manager;

    protected function setUp(): void {
        wp_stubs_reset();

        // Configure the plugin.
        update_option( 'flapjack_app_id', 'integration-test-app' );
        update_option( 'flapjack_api_key', 'integration-test-key' );
        update_option( 'flapjack_index_name', 'wp_posts' );
        update_option( 'flapjack_post_types', [ 'post', 'page' ] );
        update_option( 'flapjack_enable_search', true );
        update_option( 'flapjack_enable_instant', false );
        update_option( 'flapjack_posts_per_page', 10 );
        update_option( 'flapjack_searchable_attrs', [ 'post_title', 'post_content', 'post_excerpt' ] );

        // Mock the SearchClient (the only external dependency).
        $this->mock_client = $this->createMock( SearchClient::class );

        // Partial mock of ClientFactory — real config methods, mocked client.
        $this->client_factory = $this->createMock( ClientFactory::class );
        $this->client_factory->method( 'get_client' )->willReturn( $this->mock_client );
        $this->client_factory->method( 'get_search_client' )->willReturn( $this->mock_client );
        $this->client_factory->method( 'is_configured' )->willReturn( true );
        $this->client_factory->method( 'get_app_id' )->willReturn( 'integration-test-app' );
        $this->client_factory->method( 'get_index_name' )->willReturn( 'wp_posts' );
        $this->client_factory->method( 'get_host' )->willReturn( '' );

        // Real IndexManager wired to mock client.
        $this->index_manager = new IndexManager( $this->client_factory );
    }

    // ─── Full index → search lifecycle ────────────────────────

    public function test_index_post_then_search_returns_it(): void {
        global $wp_posts_store;

        // 1. Create a post.
        $post = $this->make_post( [
            'ID'           => 1,
            'post_title'   => 'Hello World',
            'post_content' => 'This is the first test post with some content.',
            'post_status'  => 'publish',
            'post_type'    => 'post',
        ] );
        $wp_posts_store[1] = $post;

        // 2. Index it — expect saveObject called with correct record.
        $saved_record = null;
        $this->mock_client
            ->expects( $this->once() )
            ->method( 'saveObject' )
            ->with( 'wp_posts', $this->callback( function ( $record ) use ( &$saved_record ) {
                $saved_record = $record;
                return $record['objectID'] === '1'
                    && $record['post_title'] === 'Hello World'
                    && ! empty( $record['post_content'] )
                    && $record['post_type'] === 'post'
                    && ! empty( $record['permalink'] );
            } ) )
            ->willReturn( [ 'objectID' => '1', 'taskID' => 123 ] );

        $this->index_manager->index_post( $post );

        // 3. Now search — mock the API to return the post we indexed.
        $this->mock_client
            ->method( 'searchSingleIndex' )
            ->willReturn( [
                'hits'        => [ $saved_record ],
                'nbHits'      => 1,
                'hitsPerPage' => 10,
                'page'        => 0,
            ] );

        // 4. Use QueryInterceptor to intercept a WP_Query search.
        $interceptor = new QueryInterceptor( $this->client_factory );
        $query       = new \WP_Query( [ 's' => 'hello' ] );
        $query->set( 'paged', 1 );
        $query->set( 'posts_per_page', 10 );

        $result = $interceptor->intercept_search( null, $query );

        // 5. Verify we got the post back.
        $this->assertIsArray( $result );
        $this->assertCount( 1, $result );
        $this->assertSame( 1, $result[0]->ID );
        $this->assertSame( 'Hello World', $result[0]->post_title );
        $this->assertSame( 1, $query->found_posts );
    }

    // ─── PostSyncHooks → IndexManager integration ────────────

    public function test_post_untrash_triggers_index_via_sync_hooks(): void {
        global $wp_posts_store;

        $post = $this->make_post( [
            'ID'           => 5,
            'post_title'   => 'Auto-synced Post',
            'post_status'  => 'publish',
            'post_type'    => 'post',
        ] );
        $wp_posts_store[5] = $post;

        $this->mock_client
            ->expects( $this->once() )
            ->method( 'saveObject' )
            ->with( 'wp_posts', $this->callback( function ( $record ) {
                return $record['objectID'] === '5'
                    && $record['post_title'] === 'Auto-synced Post';
            } ) )
            ->willReturn( [ 'objectID' => '5' ] );

        // Wire up PostSyncHooks with the real IndexManager.
        // Use on_untrash_post instead of on_save_post because DOING_AUTOSAVE
        // is a global constant that may persist from earlier test suites.
        $sync = new PostSyncHooks( $this->index_manager );
        $sync->on_untrash_post( 5 );
    }

    public function test_post_trash_triggers_delete_via_sync_hooks(): void {
        $this->mock_client
            ->expects( $this->once() )
            ->method( 'deleteObject' )
            ->with( 'wp_posts', '7' )
            ->willReturn( [ 'taskID' => 456 ] );

        $sync = new PostSyncHooks( $this->index_manager );
        $sync->on_trash_post( 7 );
    }

    public function test_status_transition_draft_to_publish_indexes(): void {
        global $wp_posts_store;

        $post = $this->make_post( [
            'ID'           => 8,
            'post_title'   => 'Just Published',
            'post_status'  => 'publish',
            'post_type'    => 'post',
        ] );
        $wp_posts_store[8] = $post;

        $this->mock_client
            ->expects( $this->once() )
            ->method( 'saveObject' )
            ->with( 'wp_posts', $this->callback( function ( $record ) {
                return $record['objectID'] === '8';
            } ) )
            ->willReturn( [ 'objectID' => '8' ] );

        $sync = new PostSyncHooks( $this->index_manager );
        $sync->on_status_transition( 'publish', 'draft', $post );
    }

    public function test_status_transition_publish_to_draft_deletes(): void {
        $post = $this->make_post( [ 'ID' => 9 ] );

        $this->mock_client
            ->expects( $this->once() )
            ->method( 'deleteObject' )
            ->with( 'wp_posts', '9' )
            ->willReturn( [ 'taskID' => 789 ] );

        $sync = new PostSyncHooks( $this->index_manager );
        $sync->on_status_transition( 'draft', 'publish', $post );
    }

    // ─── REST endpoint integration ───────────────────────────

    public function test_search_endpoint_returns_api_results(): void {
        $this->mock_client
            ->method( 'searchSingleIndex' )
            ->with( 'wp_posts', $this->callback( function ( $params ) {
                return $params['query'] === 'test query'
                    && $params['hitsPerPage'] === 10;
            } ) )
            ->willReturn( [
                'hits'   => [
                    [ 'objectID' => '1', 'post_title' => 'Test Post' ],
                ],
                'nbHits' => 1,
            ] );

        $endpoint = new SearchEndpoint( $this->client_factory );
        $request  = new \WP_REST_Request( 'GET', '/flapjack-search/v1/search' );
        $request->set_param( 'q', 'test query' );
        $request->set_param( 'per_page', 10 );
        $request->set_param( 'page', 0 );

        $response = $endpoint->handle_search( $request );

        $this->assertInstanceOf( \WP_REST_Response::class, $response );
        $this->assertSame( 200, $response->get_status() );
        $data = $response->get_data();
        $this->assertSame( 1, $data['nbHits'] );
        $this->assertCount( 1, $data['hits'] );
    }

    public function test_index_endpoint_triggers_reindex(): void {
        // Expect saveObjects and setSettings during reindex.
        $this->mock_client
            ->method( 'saveObjects' )
            ->willReturn( [ 'objectIDs' => [] ] );
        $this->mock_client
            ->method( 'setSettings' )
            ->willReturn( [ 'taskID' => 1 ] );

        $endpoint = new IndexEndpoint( $this->index_manager );
        $request  = new \WP_REST_Request( 'POST', '/flapjack-search/v1/reindex' );

        $response = $endpoint->handle_reindex( $request );

        $this->assertInstanceOf( \WP_REST_Response::class, $response );
        $this->assertSame( 200, $response->get_status() );
        $data = $response->get_data();
        $this->assertTrue( $data['success'] );
        $this->assertArrayHasKey( 'total', $data );
        $this->assertArrayHasKey( 'batches', $data );
    }

    public function test_status_endpoint_returns_full_status(): void {
        $this->mock_client
            ->method( 'getSettings' )
            ->willReturn( [] );
        $this->mock_client
            ->method( 'searchSingleIndex' )
            ->willReturn( [ 'nbHits' => 42 ] );

        $endpoint = new StatusEndpoint( $this->client_factory, $this->index_manager );
        $request  = new \WP_REST_Request( 'GET', '/flapjack-search/v1/status' );

        $response = $endpoint->handle_status( $request );

        $this->assertInstanceOf( \WP_REST_Response::class, $response );
        $this->assertSame( 200, $response->get_status() );

        $data = $response->get_data();
        $this->assertSame( FLAPJACK_SEARCH_VERSION, $data['plugin_version'] );
        $this->assertTrue( $data['configured'] );
        $this->assertTrue( $data['search_enabled'] );
        $this->assertFalse( $data['instant_enabled'] );
        $this->assertArrayHasKey( 'index', $data );
        $this->assertArrayHasKey( 'wp_post_count', $data );
    }

    // ─── Record building integration ─────────────────────────

    public function test_build_record_includes_all_expected_fields(): void {
        $post = $this->make_post( [
            'ID'              => 100,
            'post_title'      => 'Full Record Test',
            'post_content'    => '<p>HTML content with <strong>tags</strong> and [shortcode]blocks[/shortcode].</p>',
            'post_excerpt'    => 'Custom excerpt here.',
            'post_status'     => 'publish',
            'post_type'       => 'post',
            'post_author'     => '1',
            'menu_order'      => 5,
            'comment_count'   => 12,
            'post_date_gmt'   => '2026-01-15 10:00:00',
            'post_modified_gmt' => '2026-01-20 15:30:00',
        ] );

        $record = $this->index_manager->build_record( $post );

        // Core fields.
        $this->assertSame( '100', $record['objectID'] );
        $this->assertSame( 100, $record['post_id'] );
        $this->assertSame( 'Full Record Test', $record['post_title'] );
        $this->assertSame( 'Custom excerpt here.', $record['post_excerpt'] );
        $this->assertSame( 'post', $record['post_type'] );
        $this->assertSame( 'publish', $record['post_status'] );

        // Content should be cleaned (no HTML, no shortcodes).
        $this->assertStringNotContainsString( '<p>', $record['post_content'] );
        $this->assertStringNotContainsString( '<strong>', $record['post_content'] );

        // Permalink.
        $this->assertStringContainsString( '100', $record['permalink'] );

        // Author.
        $this->assertArrayHasKey( 'author', $record );
        $this->assertSame( 1, $record['author']['id'] );
        $this->assertNotEmpty( $record['author']['name'] );

        // Timestamps as Unix timestamps.
        $this->assertIsInt( $record['post_date'] );
        $this->assertIsInt( $record['post_modified'] );
        $this->assertGreaterThan( 0, $record['post_date'] );

        // Meta fields.
        $this->assertSame( 5, $record['menu_order'] );
        $this->assertSame( 12, $record['comment_count'] );
    }

    public function test_non_published_post_should_not_be_indexed(): void {
        $draft = $this->make_post( [ 'ID' => 10, 'post_status' => 'draft' ] );
        $this->assertFalse( $this->index_manager->should_index_post( $draft ) );

        $password_protected = $this->make_post( [ 'ID' => 11, 'post_password' => 'secret' ] );
        $this->assertFalse( $this->index_manager->should_index_post( $password_protected ) );

        $wrong_type = $this->make_post( [ 'ID' => 12, 'post_type' => 'attachment' ] );
        $this->assertFalse( $this->index_manager->should_index_post( $wrong_type ) );

        $published = $this->make_post( [ 'ID' => 13, 'post_status' => 'publish', 'post_type' => 'post' ] );
        $this->assertTrue( $this->index_manager->should_index_post( $published ) );
    }

    // ─── QueryInterceptor bypass logic ───────────────────────

    public function test_interceptor_skips_non_search_queries(): void {
        $interceptor = new QueryInterceptor( $this->client_factory );
        $query       = new \WP_Query( [ 'post_type' => 'post' ] ); // Not a search.

        $result = $interceptor->intercept_search( null, $query );
        $this->assertNull( $result ); // Should pass through.
    }

    public function test_interceptor_skips_bypassed_queries(): void {
        $interceptor = new QueryInterceptor( $this->client_factory );
        $query       = new \WP_Query( [ 's' => 'test', 'flapjack_bypass' => true ] );

        $result = $interceptor->intercept_search( null, $query );
        $this->assertNull( $result );
    }

    public function test_interceptor_falls_back_on_api_error(): void {
        $this->mock_client
            ->method( 'searchSingleIndex' )
            ->willThrowException( new \RuntimeException( 'API down' ) );

        $interceptor = new QueryInterceptor( $this->client_factory );
        $query       = new \WP_Query( [ 's' => 'test' ] );
        $query->set( 'paged', 1 );
        $query->set( 'posts_per_page', 10 );

        $result = $interceptor->intercept_search( null, $query );
        $this->assertNull( $result ); // Falls back to native search.
    }

    // ─── Multi-post pagination integration ───────────────────

    public function test_search_pagination_across_pages(): void {
        global $wp_posts_store;

        // Create 3 posts.
        for ( $i = 1; $i <= 3; $i++ ) {
            $post = $this->make_post( [
                'ID'         => $i,
                'post_title' => "Post {$i}",
                'post_type'  => 'post',
            ] );
            $wp_posts_store[ $i ] = $post;
        }

        // Page 1: return posts 1,2 (per_page=2).
        $this->mock_client
            ->method( 'searchSingleIndex' )
            ->willReturn( [
                'hits'        => [
                    [ 'objectID' => '1' ],
                    [ 'objectID' => '2' ],
                ],
                'nbHits'      => 3,
                'hitsPerPage' => 2,
                'page'        => 0,
            ] );

        $interceptor = new QueryInterceptor( $this->client_factory );
        $query       = new \WP_Query( [ 's' => 'post' ] );
        $query->set( 'paged', 1 );
        $query->set( 'posts_per_page', 2 );

        $result = $interceptor->intercept_search( null, $query );

        $this->assertCount( 2, $result );
        $this->assertSame( 3, $query->found_posts );
        $this->assertSame( 2, $query->max_num_pages );
    }

    // ─── Filter hooks integration ────────────────────────────

    public function test_flapjack_post_record_filter_modifies_record(): void {
        add_filter( 'flapjack_post_record', function ( array $record, \WP_Post $post ) {
            $record['custom_field'] = 'custom_value';
            return $record;
        }, 10, 2 );

        $post   = $this->make_post( [ 'ID' => 50, 'post_title' => 'Filtered Post' ] );
        $record = $this->index_manager->build_record( $post );

        $this->assertSame( 'custom_value', $record['custom_field'] );
    }

    public function test_flapjack_should_index_post_filter_can_exclude(): void {
        add_filter( 'flapjack_should_index_post', function ( bool $should, \WP_Post $post ) {
            // Exclude posts with "EXCLUDE" in title.
            return strpos( $post->post_title, 'EXCLUDE' ) === false;
        }, 10, 2 );

        $included = $this->make_post( [ 'ID' => 60, 'post_title' => 'Normal Post' ] );
        $this->assertTrue( $this->index_manager->should_index_post( $included ) );

        $excluded = $this->make_post( [ 'ID' => 61, 'post_title' => 'EXCLUDE This Post' ] );
        $this->assertFalse( $this->index_manager->should_index_post( $excluded ) );
    }

    // ─── Error resilience integration ────────────────────────

    public function test_sync_hooks_catch_api_errors_without_breaking_save(): void {
        // Mock client that always throws.
        $this->mock_client
            ->method( 'saveObject' )
            ->willThrowException( new \RuntimeException( 'API unavailable' ) );

        $post = $this->make_post( [ 'ID' => 70, 'post_status' => 'publish' ] );
        $sync = new PostSyncHooks( $this->index_manager );

        // Should NOT throw — errors are caught and logged.
        $sync->on_save_post( 70, $post );
        $this->assertTrue( true );
    }

    public function test_delete_post_handles_404_gracefully(): void {
        $this->mock_client
            ->method( 'deleteObject' )
            ->willThrowException( new \RuntimeException( 'Object not found (404)' ) );

        // Should not throw — 404 is handled gracefully.
        $result = $this->index_manager->delete_post( 999 );
        $this->assertArrayHasKey( 'deleted', $result );
        $this->assertTrue( $result['deleted'] );
    }

}
