<?php
/**
 * cli\Table stub for WP-CLI status command testing.
 *
 * @package Flapjack\WordPress\Tests
 */

declare(strict_types=1);

namespace cli;

if ( ! class_exists( 'cli\\Table' ) ) {
    class Table {
        public static ?self $last_instance = null;
        public array $headers = [];
        public array $rows = [];

        public function setHeaders( array $headers ): void {
            $this->headers = $headers;
        }

        public function setRows( array $rows ): void {
            $this->rows = $rows;
        }

        public function display(): void {
            // Capture instance for test assertions.
            self::$last_instance = $this;
        }

        public static function reset(): void {
            self::$last_instance = null;
        }
    }
}
