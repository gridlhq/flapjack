<?php
/**
 * Tests for IndexManager.
 *
 * @package Flapjack\WordPress\Tests\Unit\Indexing
 */

declare(strict_types=1);

namespace Flapjack\WordPress\Tests\Unit\Indexing;

use PHPUnit\Framework\TestCase;
use PHPUnit\Framework\MockObject\MockObject;
use Flapjack\WordPress\ClientFactory;
use Flapjack\WordPress\Indexing\IndexManager;
use Flapjack\WordPress\Tests\Traits\MakesTestPosts;
use Flapjack\FlapjackSearch\Api\SearchClient;

class IndexManagerTest extends TestCase {

    use MakesTestPosts;

    private ClientFactory&MockObject $client_factory;
    private SearchClient&MockObject $search_client;
    private IndexManager $index_manager;

    protected function setUp(): void {
        wp_stubs_reset();

        $this->search_client  = $this->createMock( SearchClient::class );
        $this->client_factory = $this->createMock( ClientFactory::class );

        $this->client_factory->method( 'get_client' )->willReturn( $this->search_client );
        $this->client_factory->method( 'get_index_name' )->willReturn( 'wp_posts' );

        $this->index_manager = new IndexManager( $this->client_factory );

        // Set default options.
        update_option( 'flapjack_post_types', [ 'post', 'page' ] );
        update_option( 'flapjack_searchable_attrs', [ 'post_title', 'post_content', 'post_excerpt' ] );
    }

    // ─── should_index_post ────────────────────────────────────

    public function test_should_index_published_post(): void {
        $post = $this->make_post( [ 'post_status' => 'publish', 'post_type' => 'post' ] );
        $this->assertTrue( $this->index_manager->should_index_post( $post ) );
    }

    public function test_should_not_index_draft_post(): void {
        $post = $this->make_post( [ 'post_status' => 'draft' ] );
        $this->assertFalse( $this->index_manager->should_index_post( $post ) );
    }

    public function test_should_not_index_password_protected_post(): void {
        $post = $this->make_post( [ 'post_status' => 'publish', 'post_password' => 'secret' ] );
        $this->assertFalse( $this->index_manager->should_index_post( $post ) );
    }

    public function test_should_not_index_unconfigured_post_type(): void {
        $post = $this->make_post( [ 'post_type' => 'custom_cpt', 'post_status' => 'publish' ] );
        $this->assertFalse( $this->index_manager->should_index_post( $post ) );
    }

    public function test_should_index_page(): void {
        $post = $this->make_post( [ 'post_type' => 'page', 'post_status' => 'publish' ] );
        $this->assertTrue( $this->index_manager->should_index_post( $post ) );
    }

    public function test_should_index_respects_filter(): void {
        $post = $this->make_post( [ 'post_status' => 'publish', 'post_type' => 'post' ] );

        // Add a filter that blocks indexing.
        add_filter( 'flapjack_should_index_post', function () {
            return false;
        } );

        $this->assertFalse( $this->index_manager->should_index_post( $post ) );
    }

    // ─── build_record ─────────────────────────────────────────

    public function test_build_record_contains_required_fields(): void {
        $post   = $this->make_post( [
            'ID'           => 42,
            'post_title'   => 'Test Post',
            'post_content' => 'Hello world content',
            'post_excerpt' => 'A short excerpt',
            'post_type'    => 'post',
            'post_status'  => 'publish',
        ] );
        $record = $this->index_manager->build_record( $post );

        $this->assertSame( '42', $record['objectID'] );
        $this->assertSame( 42, $record['post_id'] );
        $this->assertSame( 'Test Post', $record['post_title'] );
        $this->assertSame( 'A short excerpt', $record['post_excerpt'] );
        $this->assertSame( 'Hello world content', $record['post_content'] );
        $this->assertSame( 'post', $record['post_type'] );
        $this->assertSame( 'publish', $record['post_status'] );
        $this->assertArrayHasKey( 'permalink', $record );
        $this->assertArrayHasKey( 'author', $record );
        $this->assertArrayHasKey( 'post_date', $record );
        $this->assertArrayHasKey( 'post_modified', $record );
    }

    public function test_build_record_strips_html_from_content(): void {
        $post   = $this->make_post( [
            'post_content' => '<p>Hello <strong>world</strong></p><script>alert("xss")</script>',
        ] );
        $record = $this->index_manager->build_record( $post );

        $this->assertStringNotContainsString( '<p>', $record['post_content'] );
        $this->assertStringNotContainsString( '<script>', $record['post_content'] );
        $this->assertStringContainsString( 'Hello', $record['post_content'] );
        $this->assertStringContainsString( 'world', $record['post_content'] );
    }

    public function test_build_record_strips_shortcodes(): void {
        $post   = $this->make_post( [
            'post_content' => 'Before [gallery ids="1,2,3"] After',
        ] );
        $record = $this->index_manager->build_record( $post );

        $this->assertStringNotContainsString( '[gallery', $record['post_content'] );
        $this->assertStringContainsString( 'Before', $record['post_content'] );
        $this->assertStringContainsString( 'After', $record['post_content'] );
    }

    public function test_build_record_truncates_long_content(): void {
        $post   = $this->make_post( [
            'post_content' => str_repeat( 'a', 15000 ),
        ] );
        $record = $this->index_manager->build_record( $post );

        $this->assertLessThanOrEqual( 10000, mb_strlen( $record['post_content'] ) );
    }

    public function test_build_record_generates_excerpt_from_content(): void {
        $content = str_repeat( 'word ', 100 );
        $post    = $this->make_post( [
            'post_excerpt' => '',
            'post_content' => $content,
        ] );
        $record  = $this->index_manager->build_record( $post );

        $this->assertNotEmpty( $record['post_excerpt'] );
        $this->assertLessThanOrEqual( 303, mb_strlen( $record['post_excerpt'] ) ); // 300 + "..."
    }

    public function test_build_record_uses_existing_excerpt(): void {
        $post   = $this->make_post( [
            'post_excerpt' => 'My custom excerpt',
            'post_content' => 'Full content here',
        ] );
        $record = $this->index_manager->build_record( $post );

        $this->assertSame( 'My custom excerpt', $record['post_excerpt'] );
    }

    public function test_build_record_includes_author(): void {
        $post   = $this->make_post( [ 'post_author' => '5' ] );
        $record = $this->index_manager->build_record( $post );

        $this->assertIsArray( $record['author'] );
        $this->assertSame( 5, $record['author']['id'] );
        $this->assertSame( 'Test Author', $record['author']['name'] );
    }

    public function test_build_record_includes_menu_order(): void {
        $post   = $this->make_post( [ 'menu_order' => 5 ] );
        $record = $this->index_manager->build_record( $post );

        $this->assertSame( 5, $record['menu_order'] );
    }

    public function test_build_record_includes_comment_count(): void {
        $post   = $this->make_post( [ 'comment_count' => 10 ] );
        $record = $this->index_manager->build_record( $post );

        $this->assertSame( 10, $record['comment_count'] );
    }

    public function test_build_record_respects_filter(): void {
        $post = $this->make_post( [ 'ID' => 99, 'post_title' => 'Original' ] );

        add_filter( 'flapjack_post_record', function ( array $record, \WP_Post $post ) {
            $record['custom_field'] = 'custom_value';
            return $record;
        }, 10, 2 );

        $record = $this->index_manager->build_record( $post );
        $this->assertSame( 'custom_value', $record['custom_field'] );
    }

    public function test_build_record_includes_post_type_label(): void {
        $post   = $this->make_post( [ 'post_type' => 'post' ] );
        $record = $this->index_manager->build_record( $post );

        $this->assertSame( 'Post', $record['post_type_label'] );
    }

    // ─── index_post ───────────────────────────────────────────

    public function test_index_post_calls_save_object(): void {
        $post = $this->make_post( [ 'ID' => 42, 'post_status' => 'publish', 'post_type' => 'post' ] );

        $this->search_client
            ->expects( $this->once() )
            ->method( 'saveObject' )
            ->with( 'wp_posts', $this->callback( fn( $record ) => $record['objectID'] === '42' ) )
            ->willReturn( [ 'taskID' => 1 ] );

        $result = $this->index_manager->index_post( $post );
        $this->assertArrayHasKey( 'taskID', $result );
    }

    public function test_index_post_accepts_post_id(): void {
        global $wp_posts_store;
        $post = $this->make_post( [ 'ID' => 55, 'post_status' => 'publish', 'post_type' => 'post' ] );
        $wp_posts_store[55] = $post;

        $this->search_client
            ->expects( $this->once() )
            ->method( 'saveObject' )
            ->willReturn( [ 'taskID' => 1 ] );

        $this->index_manager->index_post( 55 );
    }

    public function test_index_post_throws_for_invalid_post(): void {
        $this->expectException( \InvalidArgumentException::class );
        $this->index_manager->index_post( 999999 );
    }

    public function test_index_post_deletes_ineligible_post(): void {
        $post = $this->make_post( [ 'ID' => 42, 'post_status' => 'draft', 'post_type' => 'post' ] );

        $this->search_client
            ->expects( $this->once() )
            ->method( 'deleteObject' )
            ->with( 'wp_posts', '42' )
            ->willReturn( [ 'taskID' => 1 ] );

        $this->index_manager->index_post( $post );
    }

    // ─── delete_post ──────────────────────────────────────────

    public function test_delete_post_calls_delete_object(): void {
        $this->search_client
            ->expects( $this->once() )
            ->method( 'deleteObject' )
            ->with( 'wp_posts', '42' )
            ->willReturn( [ 'taskID' => 1 ] );

        $result = $this->index_manager->delete_post( 42 );
        $this->assertArrayHasKey( 'taskID', $result );
    }

    public function test_delete_post_handles_404_gracefully(): void {
        $this->search_client
            ->expects( $this->once() )
            ->method( 'deleteObject' )
            ->willThrowException( new \RuntimeException( 'Object not found (404)' ) );

        $result = $this->index_manager->delete_post( 999 );
        $this->assertTrue( $result['deleted'] );
    }

    public function test_delete_post_rethrows_non_404_errors(): void {
        $this->search_client
            ->expects( $this->once() )
            ->method( 'deleteObject' )
            ->willThrowException( new \RuntimeException( 'Connection refused' ) );

        $this->expectException( \RuntimeException::class );
        $this->expectExceptionMessage( 'Connection refused' );
        $this->index_manager->delete_post( 42 );
    }

    // ─── get_index_stats ──────────────────────────────────────

    public function test_get_index_stats_returns_stats_on_success(): void {
        $this->search_client
            ->method( 'getSettings' )
            ->willReturn( [] );

        $this->search_client
            ->method( 'searchSingleIndex' )
            ->willReturn( [ 'nbHits' => 150 ] );

        $stats = $this->index_manager->get_index_stats();

        $this->assertTrue( $stats['exists'] );
        $this->assertSame( 150, $stats['count'] );
        $this->assertSame( 'wp_posts', $stats['name'] );
    }

    public function test_get_index_stats_returns_defaults_on_failure(): void {
        $this->search_client
            ->method( 'getSettings' )
            ->willThrowException( new \RuntimeException( 'Index not found' ) );

        $stats = $this->index_manager->get_index_stats();

        $this->assertFalse( $stats['exists'] );
        $this->assertSame( 0, $stats['count'] );
    }

    // ─── configure_index_settings ─────────────────────────────

    public function test_configure_index_settings_sends_correct_settings(): void {
        $this->search_client
            ->expects( $this->once() )
            ->method( 'setSettings' )
            ->with(
                'wp_posts',
                $this->callback( function ( array $settings ) {
                    return isset( $settings['searchableAttributes'] )
                        && in_array( 'post_title', $settings['searchableAttributes'], true )
                        && in_array( 'post_content', $settings['searchableAttributes'], true )
                        && isset( $settings['attributesForFaceting'] )
                        && isset( $settings['customRanking'] );
                } )
            );

        $this->index_manager->configure_index_settings( 'wp_posts' );
    }

    public function test_configure_index_settings_respects_searchable_attrs_option(): void {
        update_option( 'flapjack_searchable_attrs', [ 'post_title' ] );

        $this->search_client
            ->expects( $this->once() )
            ->method( 'setSettings' )
            ->with(
                'wp_posts',
                $this->callback( function ( array $settings ) {
                    return $settings['searchableAttributes'] === [ 'post_title' ];
                } )
            );

        $this->index_manager->configure_index_settings( 'wp_posts' );
    }

    public function test_configure_index_settings_includes_author_when_selected(): void {
        update_option( 'flapjack_searchable_attrs', [ 'post_title', 'author' ] );

        $this->search_client
            ->expects( $this->once() )
            ->method( 'setSettings' )
            ->with(
                'wp_posts',
                $this->callback( function ( array $settings ) {
                    return in_array( 'author.name', $settings['searchableAttributes'], true );
                } )
            );

        $this->index_manager->configure_index_settings( 'wp_posts' );
    }

    public function test_configure_index_settings_respects_filter(): void {
        add_filter( 'flapjack_index_settings', function ( array $settings ) {
            $settings['customRanking'] = [ 'asc(menu_order)' ];
            return $settings;
        } );

        $this->search_client
            ->expects( $this->once() )
            ->method( 'setSettings' )
            ->with(
                'wp_posts',
                $this->callback( function ( array $settings ) {
                    return $settings['customRanking'] === [ 'asc(menu_order)' ];
                } )
            );

        $this->index_manager->configure_index_settings( 'wp_posts' );
    }

    // ─── reindex_all ─────────────────────────────────────────

    public function test_reindex_all_indexes_stored_posts(): void {
        global $wp_posts_store;

        // Create 3 published posts.
        for ( $i = 1; $i <= 3; $i++ ) {
            $wp_posts_store[ $i ] = $this->make_post( [
                'ID'          => $i,
                'post_title'  => "Post {$i}",
                'post_status' => 'publish',
                'post_type'   => 'post',
            ] );
        }

        $this->search_client
            ->expects( $this->once() )
            ->method( 'saveObjects' )
            ->with(
                'wp_posts',
                $this->callback( function ( array $records ) {
                    return count( $records ) === 3
                        && $records[0]['objectID'] === '1'
                        && $records[2]['objectID'] === '3';
                } )
            )
            ->willReturn( [ 'objectIDs' => [ '1', '2', '3' ] ] );

        $this->search_client
            ->method( 'setSettings' )
            ->willReturn( [ 'taskID' => 1 ] );

        $result = $this->index_manager->reindex_all();

        $this->assertSame( 3, $result['total'] );
        $this->assertSame( 1, $result['batches'] );
    }

    public function test_reindex_all_returns_zero_for_no_posts(): void {
        $this->search_client
            ->method( 'setSettings' )
            ->willReturn( [ 'taskID' => 1 ] );

        $result = $this->index_manager->reindex_all();

        $this->assertSame( 0, $result['total'] );
        $this->assertSame( 0, $result['batches'] );
    }

    // ─── reindex_atomic ─────────────────────────────────────

    public function test_reindex_atomic_indexes_to_temp_index(): void {
        global $wp_posts_store;

        for ( $i = 1; $i <= 3; $i++ ) {
            $wp_posts_store[ $i ] = $this->make_post( [
                'ID'          => $i,
                'post_title'  => "Post {$i}",
                'post_status' => 'publish',
                'post_type'   => 'post',
            ] );
        }

        // saveObjects should be called with the tmp index name, not the live one.
        $this->search_client
            ->expects( $this->once() )
            ->method( 'saveObjects' )
            ->with(
                $this->matchesRegularExpression( '/^wp_posts_tmp_\d+$/' ),
                $this->callback( fn( $records ) => count( $records ) === 3 )
            )
            ->willReturn( [ 'objectIDs' => [ '1', '2', '3' ] ] );

        $this->search_client
            ->method( 'setSettings' )
            ->willReturn( [ 'taskID' => 1 ] );

        $this->search_client
            ->expects( $this->once() )
            ->method( 'operationIndex' )
            ->willReturn( [ 'taskID' => 2 ] );

        $result = $this->index_manager->reindex_atomic();

        $this->assertSame( 3, $result['total'] );
        $this->assertSame( 1, $result['batches'] );
        $this->assertMatchesRegularExpression( '/^wp_posts_tmp_\d+$/', $result['tmp_index'] );
    }

    public function test_reindex_atomic_configures_settings_on_tmp_index(): void {
        global $wp_posts_store;

        $wp_posts_store[1] = $this->make_post( [
            'ID' => 1, 'post_status' => 'publish', 'post_type' => 'post',
        ] );

        $this->search_client
            ->method( 'saveObjects' )
            ->willReturn( [ 'objectIDs' => [ '1' ] ] );

        // setSettings should be called with the tmp index, not the live index.
        $this->search_client
            ->expects( $this->once() )
            ->method( 'setSettings' )
            ->with(
                $this->matchesRegularExpression( '/^wp_posts_tmp_\d+$/' ),
                $this->isType( 'array' )
            )
            ->willReturn( [ 'taskID' => 1 ] );

        $this->search_client
            ->method( 'operationIndex' )
            ->willReturn( [ 'taskID' => 2 ] );

        $this->index_manager->reindex_atomic();
    }

    public function test_reindex_atomic_moves_tmp_to_live(): void {
        $this->search_client
            ->method( 'setSettings' )
            ->willReturn( [ 'taskID' => 1 ] );

        $this->search_client
            ->expects( $this->once() )
            ->method( 'operationIndex' )
            ->with(
                $this->matchesRegularExpression( '/^wp_posts_tmp_\d+$/' ),
                $this->callback( function ( array $params ) {
                    return $params['operation'] === 'move'
                        && $params['destination'] === 'wp_posts';
                } )
            )
            ->willReturn( [ 'taskID' => 2 ] );

        $this->index_manager->reindex_atomic();
    }

    public function test_reindex_atomic_returns_zero_for_no_posts(): void {
        $this->search_client
            ->method( 'setSettings' )
            ->willReturn( [ 'taskID' => 1 ] );

        $this->search_client
            ->method( 'operationIndex' )
            ->willReturn( [ 'taskID' => 2 ] );

        $result = $this->index_manager->reindex_atomic();

        $this->assertSame( 0, $result['total'] );
        $this->assertSame( 0, $result['batches'] );
        $this->assertArrayHasKey( 'tmp_index', $result );
    }

    public function test_reindex_atomic_tmp_index_name_contains_timestamp(): void {
        $this->search_client
            ->method( 'setSettings' )
            ->willReturn( [ 'taskID' => 1 ] );

        $this->search_client
            ->method( 'operationIndex' )
            ->willReturn( [ 'taskID' => 2 ] );

        $before = time();
        $result = $this->index_manager->reindex_atomic();
        $after  = time();

        // Extract timestamp from tmp_index name.
        $parts     = explode( '_tmp_', $result['tmp_index'] );
        $timestamp = (int) $parts[1];

        $this->assertGreaterThanOrEqual( $before, $timestamp );
        $this->assertLessThanOrEqual( $after, $timestamp );
    }

    public function test_reindex_atomic_uses_configured_post_types(): void {
        global $wp_posts_store;

        update_option( 'flapjack_post_types', [ 'page' ] );

        // Create a post (should be excluded) and a page (should be included).
        $wp_posts_store[1] = $this->make_post( [
            'ID' => 1, 'post_status' => 'publish', 'post_type' => 'post',
        ] );
        $wp_posts_store[2] = $this->make_post( [
            'ID' => 2, 'post_status' => 'publish', 'post_type' => 'page',
        ] );

        $this->search_client
            ->expects( $this->once() )
            ->method( 'saveObjects' )
            ->with(
                $this->isType( 'string' ),
                $this->callback( function ( array $records ) {
                    // Only the page should be indexed.
                    return count( $records ) === 1 && $records[0]['post_type'] === 'page';
                } )
            )
            ->willReturn( [ 'objectIDs' => [ '2' ] ] );

        $this->search_client
            ->method( 'setSettings' )
            ->willReturn( [ 'taskID' => 1 ] );

        $this->search_client
            ->method( 'operationIndex' )
            ->willReturn( [ 'taskID' => 2 ] );

        $result = $this->index_manager->reindex_atomic();
        $this->assertSame( 1, $result['total'] );
    }
}
