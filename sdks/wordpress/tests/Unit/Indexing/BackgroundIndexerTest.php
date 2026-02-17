<?php
/**
 * Tests for BackgroundIndexer.
 *
 * @package Flapjack\WordPress\Tests\Unit\Indexing
 */

declare(strict_types=1);

namespace Flapjack\WordPress\Tests\Unit\Indexing;

use PHPUnit\Framework\TestCase;
use PHPUnit\Framework\MockObject\MockObject;
use Flapjack\WordPress\ClientFactory;
use Flapjack\WordPress\Indexing\BackgroundIndexer;
use Flapjack\WordPress\Tests\Traits\MakesTestPosts;
use Flapjack\FlapjackSearch\Api\SearchClient;

class BackgroundIndexerTest extends TestCase {

    use MakesTestPosts;

    private ClientFactory&MockObject $client_factory;
    private SearchClient&MockObject $search_client;
    private BackgroundIndexer $indexer;

    protected function setUp(): void {
        wp_stubs_reset();

        $this->search_client  = $this->createMock( SearchClient::class );
        $this->client_factory = $this->createMock( ClientFactory::class );

        $this->client_factory->method( 'get_client' )->willReturn( $this->search_client );
        $this->client_factory->method( 'get_index_name' )->willReturn( 'wp_posts' );

        $this->indexer = new BackgroundIndexer( $this->client_factory );

        // Set default options.
        update_option( 'flapjack_post_types', [ 'post', 'page' ] );
        update_option( 'flapjack_searchable_attrs', [ 'post_title', 'post_content', 'post_excerpt' ] );
    }

    // ─── Constants ────────────────────────────────────────────

    public function test_progress_transient_constant(): void {
        $this->assertSame( 'flapjack_reindex_progress', BackgroundIndexer::PROGRESS_TRANSIENT );
    }

    public function test_batch_hook_constant(): void {
        $this->assertSame( 'flapjack_background_reindex_batch', BackgroundIndexer::BATCH_HOOK );
    }

    public function test_group_constant(): void {
        $this->assertSame( 'flapjack-search', BackgroundIndexer::GROUP );
    }

    public function test_batch_size_constant(): void {
        $this->assertSame( 200, BackgroundIndexer::BATCH_SIZE );
    }

    // ─── register ─────────────────────────────────────────────

    public function test_register_adds_batch_hook(): void {
        global $wp_actions;
        $this->indexer->register();

        $hooks = array_keys( $wp_actions );
        $this->assertContains( BackgroundIndexer::BATCH_HOOK, $hooks );
    }

    // ─── is_action_scheduler_available ────────────────────────

    public function test_action_scheduler_available_when_functions_exist(): void {
        // as_schedule_single_action and as_unschedule_all_actions are defined in our stubs.
        $this->assertTrue( BackgroundIndexer::is_action_scheduler_available() );
    }

    // ─── start_reindex ─────────────────────────────────────────

    public function test_start_reindex_returns_in_progress_status(): void {
        global $wp_posts_store;

        for ( $i = 1; $i <= 5; $i++ ) {
            $wp_posts_store[ $i ] = $this->make_post( [
                'ID'          => $i,
                'post_status' => 'publish',
                'post_type'   => 'post',
            ] );
        }

        $result = $this->indexer->start_reindex();

        $this->assertSame( 'in_progress', $result['status'] );
        $this->assertSame( 5, $result['total_posts'] );
        $this->assertSame( 0, $result['processed'] );
        $this->assertSame( 1, $result['current_page'] );
        $this->assertSame( 0, $result['batches_done'] );
        $this->assertNull( $result['error'] );
        $this->assertNull( $result['completed_at'] );
    }

    public function test_start_reindex_stores_progress_transient(): void {
        global $wp_posts_store;
        $wp_posts_store[1] = $this->make_post( [
            'ID' => 1, 'post_status' => 'publish', 'post_type' => 'post',
        ] );

        $this->indexer->start_reindex();

        $progress = get_transient( BackgroundIndexer::PROGRESS_TRANSIENT );
        $this->assertIsArray( $progress );
        $this->assertSame( 'in_progress', $progress['status'] );
    }

    public function test_start_reindex_schedules_first_batch_via_action_scheduler(): void {
        global $wp_posts_store, $as_scheduled_actions;

        $wp_posts_store[1] = $this->make_post( [
            'ID' => 1, 'post_status' => 'publish', 'post_type' => 'post',
        ] );

        $this->indexer->start_reindex();

        // Action Scheduler should have one scheduled action.
        $this->assertCount( 1, $as_scheduled_actions );
        $this->assertSame( BackgroundIndexer::BATCH_HOOK, $as_scheduled_actions[0]['hook'] );
        $this->assertSame( [ 1 ], $as_scheduled_actions[0]['args'] );
        $this->assertSame( BackgroundIndexer::GROUP, $as_scheduled_actions[0]['group'] );
    }

    public function test_start_reindex_completes_immediately_for_zero_posts(): void {
        $result = $this->indexer->start_reindex();

        $this->assertSame( 'complete', $result['status'] );
        $this->assertSame( 0, $result['total_posts'] );
        $this->assertNotNull( $result['completed_at'] );
    }

    public function test_start_reindex_does_not_restart_if_already_in_progress(): void {
        // Set an existing in-progress transient.
        set_transient( BackgroundIndexer::PROGRESS_TRANSIENT, [
            'status'      => 'in_progress',
            'total_posts' => 100,
            'processed'   => 50,
        ] );

        $result = $this->indexer->start_reindex();

        // Should return the existing progress, not start a new one.
        $this->assertSame( 'in_progress', $result['status'] );
        $this->assertSame( 50, $result['processed'] );
    }

    public function test_start_reindex_calculates_total_pages(): void {
        global $wp_posts_store;

        // Create enough posts to span multiple batches (batch size is 200).
        for ( $i = 1; $i <= 10; $i++ ) {
            $wp_posts_store[ $i ] = $this->make_post( [
                'ID'          => $i,
                'post_status' => 'publish',
                'post_type'   => 'post',
            ] );
        }

        $result = $this->indexer->start_reindex();

        $this->assertSame( 10, $result['total_posts'] );
        $this->assertSame( 1, $result['total_pages'] ); // 10 posts < 200 batch size = 1 page.
    }

    public function test_start_reindex_counts_multiple_post_types(): void {
        global $wp_posts_store;

        $wp_posts_store[1] = $this->make_post( [
            'ID' => 1, 'post_status' => 'publish', 'post_type' => 'post',
        ] );
        $wp_posts_store[2] = $this->make_post( [
            'ID' => 2, 'post_status' => 'publish', 'post_type' => 'page',
        ] );
        $wp_posts_store[3] = $this->make_post( [
            'ID' => 3, 'post_status' => 'draft', 'post_type' => 'post',
        ] );

        $result = $this->indexer->start_reindex();

        // Only 2 published posts (1 post + 1 page), the draft should not be counted.
        $this->assertSame( 2, $result['total_posts'] );
    }

    public function test_start_reindex_records_method_as_action_scheduler(): void {
        global $wp_posts_store;
        $wp_posts_store[1] = $this->make_post( [
            'ID' => 1, 'post_status' => 'publish', 'post_type' => 'post',
        ] );

        $result = $this->indexer->start_reindex();
        $this->assertSame( 'action_scheduler', $result['method'] );
    }

    public function test_start_reindex_records_started_at_timestamp(): void {
        $before = time();
        $result = $this->indexer->start_reindex();
        $after  = time();

        $this->assertGreaterThanOrEqual( $before, $result['started_at'] );
        $this->assertLessThanOrEqual( $after, $result['started_at'] );
    }

    // ─── process_batch ─────────────────────────────────────────

    public function test_process_batch_indexes_posts(): void {
        global $wp_posts_store;

        for ( $i = 1; $i <= 3; $i++ ) {
            $wp_posts_store[ $i ] = $this->make_post( [
                'ID'          => $i,
                'post_title'  => "Post {$i}",
                'post_status' => 'publish',
                'post_type'   => 'post',
            ] );
        }

        // Set up initial progress.
        set_transient( BackgroundIndexer::PROGRESS_TRANSIENT, [
            'status'       => 'in_progress',
            'total_posts'  => 3,
            'processed'    => 0,
            'current_page' => 1,
            'total_pages'  => 1,
            'batches_done' => 0,
            'started_at'   => time(),
            'completed_at' => null,
            'error'        => null,
            'method'       => 'action_scheduler',
        ] );

        $this->search_client
            ->expects( $this->once() )
            ->method( 'saveObjects' )
            ->with(
                'wp_posts',
                $this->callback( fn( $records ) => count( $records ) === 3 )
            )
            ->willReturn( [ 'objectIDs' => [ '1', '2', '3' ] ] );

        $this->search_client
            ->method( 'setSettings' )
            ->willReturn( [ 'taskID' => 1 ] );

        $this->indexer->process_batch( 1 );

        $progress = get_transient( BackgroundIndexer::PROGRESS_TRANSIENT );
        $this->assertSame( 'complete', $progress['status'] );
        $this->assertSame( 3, $progress['processed'] );
        $this->assertSame( 1, $progress['batches_done'] );
        $this->assertNotNull( $progress['completed_at'] );
    }

    public function test_process_batch_schedules_next_batch_when_more_pages(): void {
        global $wp_posts_store, $as_scheduled_actions;

        // Create enough posts to need 2 pages with the test WP_Query.
        // WP_Query stub uses posts_per_page which defaults to the BATCH_SIZE.
        // We need more posts than BATCH_SIZE for multiple pages.
        // Since our WP_Query stub works differently, let's create a
        // scenario where max_num_pages > current page.
        for ( $i = 1; $i <= 3; $i++ ) {
            $wp_posts_store[ $i ] = $this->make_post( [
                'ID'          => $i,
                'post_status' => 'publish',
                'post_type'   => 'post',
            ] );
        }

        // Set progress with total_pages > 1 to simulate multi-page scenario.
        set_transient( BackgroundIndexer::PROGRESS_TRANSIENT, [
            'status'       => 'in_progress',
            'total_posts'  => 600,
            'processed'    => 0,
            'current_page' => 1,
            'total_pages'  => 3,
            'batches_done' => 0,
            'started_at'   => time(),
            'completed_at' => null,
            'error'        => null,
            'method'       => 'action_scheduler',
        ] );

        $this->search_client
            ->method( 'saveObjects' )
            ->willReturn( [ 'objectIDs' => [] ] );

        $this->indexer->process_batch( 1 );

        // Should have scheduled page 2.
        $batch_actions = array_filter( $as_scheduled_actions, fn( $a ) => $a['hook'] === BackgroundIndexer::BATCH_HOOK );
        $this->assertNotEmpty( $batch_actions );
        $last_action = end( $batch_actions );
        $this->assertSame( [ 2 ], $last_action['args'] );

        // Progress should still be in_progress.
        $progress = get_transient( BackgroundIndexer::PROGRESS_TRANSIENT );
        $this->assertSame( 'in_progress', $progress['status'] );
    }

    public function test_process_batch_marks_failed_on_exception(): void {
        global $wp_posts_store;

        $wp_posts_store[1] = $this->make_post( [
            'ID' => 1, 'post_status' => 'publish', 'post_type' => 'post',
        ] );

        set_transient( BackgroundIndexer::PROGRESS_TRANSIENT, [
            'status'       => 'in_progress',
            'total_posts'  => 1,
            'processed'    => 0,
            'current_page' => 1,
            'total_pages'  => 1,
            'batches_done' => 0,
            'started_at'   => time(),
            'completed_at' => null,
            'error'        => null,
            'method'       => 'action_scheduler',
        ] );

        $this->search_client
            ->method( 'saveObjects' )
            ->willThrowException( new \RuntimeException( 'Connection refused' ) );

        $this->indexer->process_batch( 1 );

        $progress = get_transient( BackgroundIndexer::PROGRESS_TRANSIENT );
        $this->assertSame( 'failed', $progress['status'] );
        $this->assertSame( 'Connection refused', $progress['error'] );
    }

    public function test_process_batch_does_nothing_when_not_in_progress(): void {
        // No progress transient set.
        $this->search_client
            ->expects( $this->never() )
            ->method( 'saveObjects' );

        $this->indexer->process_batch( 1 );
    }

    public function test_process_batch_does_nothing_when_status_is_complete(): void {
        set_transient( BackgroundIndexer::PROGRESS_TRANSIENT, [
            'status' => 'complete',
        ] );

        $this->search_client
            ->expects( $this->never() )
            ->method( 'saveObjects' );

        $this->indexer->process_batch( 1 );
    }

    public function test_process_batch_configures_index_settings_on_final_batch(): void {
        global $wp_posts_store;

        $wp_posts_store[1] = $this->make_post( [
            'ID' => 1, 'post_status' => 'publish', 'post_type' => 'post',
        ] );

        set_transient( BackgroundIndexer::PROGRESS_TRANSIENT, [
            'status'       => 'in_progress',
            'total_posts'  => 1,
            'processed'    => 0,
            'current_page' => 1,
            'total_pages'  => 1,
            'batches_done' => 0,
            'started_at'   => time(),
            'completed_at' => null,
            'error'        => null,
            'method'       => 'action_scheduler',
        ] );

        $this->search_client
            ->method( 'saveObjects' )
            ->willReturn( [ 'objectIDs' => [ '1' ] ] );

        // setSettings should be called on the final batch.
        $this->search_client
            ->expects( $this->once() )
            ->method( 'setSettings' )
            ->with( 'wp_posts', $this->isType( 'array' ) )
            ->willReturn( [ 'taskID' => 1 ] );

        $this->indexer->process_batch( 1 );
    }

    public function test_process_batch_skips_ineligible_posts(): void {
        global $wp_posts_store;

        $wp_posts_store[1] = $this->make_post( [
            'ID' => 1, 'post_status' => 'publish', 'post_type' => 'post',
        ] );
        // Password-protected post should be skipped.
        $wp_posts_store[2] = $this->make_post( [
            'ID' => 2, 'post_status' => 'publish', 'post_type' => 'post', 'post_password' => 'secret',
        ] );

        set_transient( BackgroundIndexer::PROGRESS_TRANSIENT, [
            'status'       => 'in_progress',
            'total_posts'  => 2,
            'processed'    => 0,
            'current_page' => 1,
            'total_pages'  => 1,
            'batches_done' => 0,
            'started_at'   => time(),
            'completed_at' => null,
            'error'        => null,
            'method'       => 'action_scheduler',
        ] );

        $this->search_client
            ->expects( $this->once() )
            ->method( 'saveObjects' )
            ->with(
                'wp_posts',
                $this->callback( fn( $records ) => count( $records ) === 1 && $records[0]['objectID'] === '1' )
            )
            ->willReturn( [ 'objectIDs' => [ '1' ] ] );

        $this->search_client
            ->method( 'setSettings' )
            ->willReturn( [ 'taskID' => 1 ] );

        $this->indexer->process_batch( 1 );

        $progress = get_transient( BackgroundIndexer::PROGRESS_TRANSIENT );
        $this->assertSame( 1, $progress['processed'] );
    }

    public function test_process_batch_handles_empty_page_gracefully(): void {
        // No posts in store but progress says there should be.
        set_transient( BackgroundIndexer::PROGRESS_TRANSIENT, [
            'status'       => 'in_progress',
            'total_posts'  => 0,
            'processed'    => 0,
            'current_page' => 1,
            'total_pages'  => 1,
            'batches_done' => 0,
            'started_at'   => time(),
            'completed_at' => null,
            'error'        => null,
            'method'       => 'action_scheduler',
        ] );

        // saveObjects should NOT be called for empty results.
        $this->search_client
            ->expects( $this->never() )
            ->method( 'saveObjects' );

        $this->search_client
            ->method( 'setSettings' )
            ->willReturn( [ 'taskID' => 1 ] );

        $this->indexer->process_batch( 1 );

        $progress = get_transient( BackgroundIndexer::PROGRESS_TRANSIENT );
        $this->assertSame( 'complete', $progress['status'] );
    }

    // ─── get_progress ──────────────────────────────────────────

    public function test_get_progress_returns_null_when_no_transient(): void {
        $this->assertNull( $this->indexer->get_progress() );
    }

    public function test_get_progress_returns_array_when_transient_set(): void {
        set_transient( BackgroundIndexer::PROGRESS_TRANSIENT, [
            'status'      => 'in_progress',
            'total_posts' => 100,
            'processed'   => 50,
        ] );

        $progress = $this->indexer->get_progress();
        $this->assertIsArray( $progress );
        $this->assertSame( 'in_progress', $progress['status'] );
        $this->assertSame( 100, $progress['total_posts'] );
        $this->assertSame( 50, $progress['processed'] );
    }

    public function test_get_progress_returns_null_for_non_array_transient(): void {
        set_transient( BackgroundIndexer::PROGRESS_TRANSIENT, 'invalid' );
        $this->assertNull( $this->indexer->get_progress() );
    }

    // ─── cancel_reindex ────────────────────────────────────────

    public function test_cancel_reindex_sets_cancelled_status(): void {
        set_transient( BackgroundIndexer::PROGRESS_TRANSIENT, [
            'status'       => 'in_progress',
            'total_posts'  => 100,
            'processed'    => 50,
            'method'       => 'action_scheduler',
        ] );

        $result = $this->indexer->cancel_reindex();

        $this->assertTrue( $result );

        $progress = get_transient( BackgroundIndexer::PROGRESS_TRANSIENT );
        $this->assertSame( 'cancelled', $progress['status'] );
    }

    public function test_cancel_reindex_returns_false_when_not_in_progress(): void {
        $this->assertFalse( $this->indexer->cancel_reindex() );
    }

    public function test_cancel_reindex_returns_false_when_already_complete(): void {
        set_transient( BackgroundIndexer::PROGRESS_TRANSIENT, [
            'status' => 'complete',
        ] );

        $this->assertFalse( $this->indexer->cancel_reindex() );
    }

    public function test_cancel_reindex_unschedules_action_scheduler_actions(): void {
        global $as_scheduled_actions;

        // Schedule some actions.
        as_schedule_single_action( time(), BackgroundIndexer::BATCH_HOOK, [ 2 ], BackgroundIndexer::GROUP );
        as_schedule_single_action( time(), BackgroundIndexer::BATCH_HOOK, [ 3 ], BackgroundIndexer::GROUP );
        $this->assertCount( 2, $as_scheduled_actions );

        set_transient( BackgroundIndexer::PROGRESS_TRANSIENT, [
            'status' => 'in_progress',
            'method' => 'action_scheduler',
        ] );

        $this->indexer->cancel_reindex();

        // All batch actions should be unscheduled.
        $remaining = array_filter( $as_scheduled_actions, fn( $a ) => $a['hook'] === BackgroundIndexer::BATCH_HOOK );
        $this->assertEmpty( $remaining );
    }

    // ─── Full flow integration ─────────────────────────────────

    public function test_full_reindex_flow_start_to_complete(): void {
        global $wp_posts_store;

        for ( $i = 1; $i <= 3; $i++ ) {
            $wp_posts_store[ $i ] = $this->make_post( [
                'ID'          => $i,
                'post_title'  => "Post {$i}",
                'post_status' => 'publish',
                'post_type'   => 'post',
            ] );
        }

        $this->search_client
            ->method( 'saveObjects' )
            ->willReturn( [ 'objectIDs' => [ '1', '2', '3' ] ] );
        $this->search_client
            ->method( 'setSettings' )
            ->willReturn( [ 'taskID' => 1 ] );

        // Start reindex.
        $result = $this->indexer->start_reindex();
        $this->assertSame( 'in_progress', $result['status'] );
        $this->assertSame( 3, $result['total_posts'] );

        // Process the batch.
        $this->indexer->process_batch( 1 );

        // Check final progress.
        $progress = $this->indexer->get_progress();
        $this->assertSame( 'complete', $progress['status'] );
        $this->assertSame( 3, $progress['processed'] );
        $this->assertSame( 1, $progress['batches_done'] );
    }
}
