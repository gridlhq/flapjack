<?php
/**
 * Background reindexing via Action Scheduler (with WP-Cron fallback).
 *
 * Handles batched background reindexing for large sites, with progress
 * tracking via transients. Prefers Action Scheduler (bundled with
 * WooCommerce) and falls back to wp_schedule_single_event().
 *
 * @package Flapjack\WordPress\Indexing
 */

declare(strict_types=1);

namespace Flapjack\WordPress\Indexing;

use Flapjack\WordPress\ClientFactory;

class BackgroundIndexer {

    public const PROGRESS_TRANSIENT = 'flapjack_reindex_progress';
    public const BATCH_HOOK         = 'flapjack_background_reindex_batch';
    public const GROUP              = 'flapjack-search';
    public const BATCH_SIZE         = 200;

    private ClientFactory $client_factory;

    public function __construct( ClientFactory $client_factory ) {
        $this->client_factory = $client_factory;
    }

    /**
     * Register WordPress hooks for batch processing.
     */
    public function register(): void {
        add_action( self::BATCH_HOOK, [ $this, 'process_batch' ], 10, 1 );
    }

    /**
     * Check if Action Scheduler is available.
     */
    public static function is_action_scheduler_available(): bool {
        return function_exists( 'as_schedule_single_action' )
            && function_exists( 'as_unschedule_all_actions' );
    }

    /**
     * Start a background reindex.
     *
     * Counts all indexable posts, stores initial progress, and schedules
     * the first batch.
     *
     * @return array{status: string, total_posts: int, method: string}
     */
    public function start_reindex(): array {
        // Don't start if already in progress.
        $existing = $this->get_progress();
        if ( $existing && 'in_progress' === ( $existing['status'] ?? '' ) ) {
            return $existing;
        }

        $post_types = (array) get_option( 'flapjack_post_types', [ 'post', 'page' ] );
        $total      = $this->count_indexable_posts( $post_types );
        $method     = self::is_action_scheduler_available() ? 'action_scheduler' : 'wp_cron';

        $progress = [
            'status'       => 'in_progress',
            'total_posts'  => $total,
            'processed'    => 0,
            'current_page' => 1,
            'total_pages'  => $total > 0 ? (int) ceil( $total / self::BATCH_SIZE ) : 0,
            'batches_done' => 0,
            'started_at'   => time(),
            'completed_at' => null,
            'error'        => null,
            'method'       => $method,
        ];

        set_transient( self::PROGRESS_TRANSIENT, $progress, HOUR_IN_SECONDS );

        if ( $total > 0 ) {
            $this->schedule_batch( 1, $method );
        } else {
            // No posts to index — mark complete immediately.
            $progress['status']       = 'complete';
            $progress['completed_at'] = time();
            set_transient( self::PROGRESS_TRANSIENT, $progress, HOUR_IN_SECONDS );
        }

        return $progress;
    }

    /**
     * Process a single batch of posts.
     *
     * @param int $page The batch page number (1-indexed).
     */
    public function process_batch( int $page ): void {
        $progress = $this->get_progress();
        if ( ! $progress || 'in_progress' !== ( $progress['status'] ?? '' ) ) {
            return;
        }

        try {
            $client     = $this->client_factory->get_client();
            $index_name = $this->client_factory->get_index_name();
            $post_types = (array) get_option( 'flapjack_post_types', [ 'post', 'page' ] );

            $index_manager = new IndexManager( $this->client_factory );

            $query = new \WP_Query( [
                'post_type'       => $post_types,
                'post_status'     => 'publish',
                'posts_per_page'  => self::BATCH_SIZE,
                'paged'           => $page,
                'orderby'         => 'ID',
                'order'           => 'ASC',
                'flapjack_bypass' => true,
            ] );

            $records = [];
            foreach ( $query->posts as $post ) {
                if ( $index_manager->should_index_post( $post ) ) {
                    $records[] = $index_manager->build_record( $post );
                }
            }

            if ( ! empty( $records ) ) {
                $client->saveObjects( $index_name, $records );
            }

            // Update progress.
            $progress['processed']    += count( $records );
            $progress['current_page']  = $page;
            $progress['total_pages']   = max( $progress['total_pages'], $query->max_num_pages );
            $progress['batches_done']++;

            $has_more = $page < $progress['total_pages'];

            if ( $has_more ) {
                set_transient( self::PROGRESS_TRANSIENT, $progress, HOUR_IN_SECONDS );
                $this->schedule_batch( $page + 1, $progress['method'] );
            } else {
                // Final batch — configure settings and mark complete.
                $index_manager->configure_index_settings( $index_name );

                $progress['status']       = 'complete';
                $progress['completed_at'] = time();
                set_transient( self::PROGRESS_TRANSIENT, $progress, HOUR_IN_SECONDS );
            }
        } catch ( \Throwable $e ) {
            $progress['status'] = 'failed';
            $progress['error']  = $e->getMessage();
            set_transient( self::PROGRESS_TRANSIENT, $progress, HOUR_IN_SECONDS );
        }
    }

    /**
     * Get the current reindex progress.
     *
     * @return array|null Progress data or null if no reindex in progress/recent.
     */
    public function get_progress(): ?array {
        $progress = get_transient( self::PROGRESS_TRANSIENT );
        return is_array( $progress ) ? $progress : null;
    }

    /**
     * Cancel an in-progress reindex.
     *
     * @return bool Whether a reindex was cancelled.
     */
    public function cancel_reindex(): bool {
        $progress = $this->get_progress();
        if ( ! $progress || 'in_progress' !== ( $progress['status'] ?? '' ) ) {
            return false;
        }

        // Unschedule pending batches.
        if ( self::is_action_scheduler_available() ) {
            as_unschedule_all_actions( self::BATCH_HOOK, null, self::GROUP );
        } else {
            wp_clear_scheduled_hook( self::BATCH_HOOK );
        }

        $progress['status'] = 'cancelled';
        set_transient( self::PROGRESS_TRANSIENT, $progress, HOUR_IN_SECONDS );

        return true;
    }

    /**
     * Schedule a batch for processing.
     *
     * @param int    $page   The page number to process.
     * @param string $method 'action_scheduler' or 'wp_cron'.
     */
    private function schedule_batch( int $page, string $method ): void {
        if ( 'action_scheduler' === $method && self::is_action_scheduler_available() ) {
            as_schedule_single_action( time(), self::BATCH_HOOK, [ $page ], self::GROUP );
        } else {
            wp_schedule_single_event( time(), self::BATCH_HOOK, [ $page ] );
        }
    }

    /**
     * Count total indexable posts.
     *
     * @param string[] $post_types Post types to count.
     * @return int Total number of published posts of the given types.
     */
    private function count_indexable_posts( array $post_types ): int {
        $total = 0;
        foreach ( $post_types as $post_type ) {
            $counts = wp_count_posts( $post_type );
            $total += (int) ( $counts->publish ?? 0 );
        }
        return $total;
    }
}
