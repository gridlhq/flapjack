<?php
/**
 * Factory for creating Flapjack Search API clients.
 *
 * @package Flapjack\WordPress
 */

declare(strict_types=1);

namespace Flapjack\WordPress;

use Flapjack\FlapjackSearch\Api\SearchClient;
use Flapjack\FlapjackSearch\Configuration\SearchConfig;

class ClientFactory {

    private ?SearchClient $client = null;

    /**
     * Get a configured SearchClient instance.
     *
     * @throws \RuntimeException If credentials are not configured.
     */
    public function get_client(): SearchClient {
        if ( null !== $this->client ) {
            return $this->client;
        }

        $app_id  = $this->get_app_id();
        $api_key = $this->get_admin_api_key();

        if ( empty( $app_id ) || empty( $api_key ) ) {
            throw new \RuntimeException(
                __( 'Flapjack Search credentials are not configured. Please set your App ID and API Key in Settings > Flapjack Search.', 'flapjack-search' )
            );
        }

        $config = SearchConfig::create( $app_id, $api_key );

        $host = $this->get_host();
        if ( ! empty( $host ) ) {
            $config->setFullHosts( [ $host ] );
        }

        $this->client = SearchClient::createWithConfig( $config );

        return $this->client;
    }

    /**
     * Get a search-only client (uses the search-only API key).
     *
     * @throws \RuntimeException If credentials are not configured.
     */
    public function get_search_client(): SearchClient {
        $app_id     = $this->get_app_id();
        $search_key = $this->get_search_api_key();

        // Never fall back to the admin key — it must not be used for search.
        if ( empty( $app_id ) || empty( $search_key ) ) {
            throw new \RuntimeException(
                __( 'Flapjack Search search-only API key is not configured. Set it in Settings > Flapjack Search.', 'flapjack-search' )
            );
        }

        $config = SearchConfig::create( $app_id, $search_key );

        $host = $this->get_host();
        if ( ! empty( $host ) ) {
            $config->setFullHosts( [ $host ] );
        }

        return SearchClient::createWithConfig( $config );
    }

    /**
     * Test the connection to Flapjack.
     *
     * @return array{success: bool, message: string}
     */
    public function test_connection(): array {
        try {
            $client = $this->get_client();
            $client->listIndices();
            return [
                'success' => true,
                'message' => __( 'Connection successful.', 'flapjack-search' ),
            ];
        } catch ( \Throwable $e ) {
            return [
                'success' => false,
                'message' => $e->getMessage(),
            ];
        }
    }

    /**
     * Check if the plugin is configured with valid credentials.
     */
    public function is_configured(): bool {
        return ! empty( $this->get_app_id() ) && ! empty( $this->get_admin_api_key() );
    }

    /**
     * Reset cached client (useful after settings change).
     */
    public function reset(): void {
        $this->client = null;
    }

    public function get_app_id(): string {
        return (string) get_option( 'flapjack_app_id', '' );
    }

    public function get_admin_api_key(): string {
        return (string) get_option( 'flapjack_api_key', '' );
    }

    public function get_search_api_key(): string {
        return (string) get_option( 'flapjack_search_api_key', '' );
    }

    public function get_host(): string {
        return (string) get_option( 'flapjack_host', '' );
    }

    public function get_index_name(): string {
        return (string) get_option( 'flapjack_index_name', 'wp_posts' );
    }
}
