<?php
/**
 * WP-CLI stubs for unit testing CLI commands without a real WP-CLI installation.
 *
 * @package Flapjack\WordPress\Tests
 */

declare(strict_types=1);

if ( ! defined( 'WP_CLI' ) ) {
    define( 'WP_CLI', true );
}

if ( ! class_exists( 'WP_CLI' ) ) {
    class WP_CLI {
        /** @var array Captured log/success/error messages for test assertions. */
        public static array $captured = [];

        /** @var array Registered CLI commands for test assertions. */
        public static array $registered_commands = [];

        public static function add_command( string $name, $callable ): void {
            self::$registered_commands[] = [ 'name' => $name, 'callable' => $callable ];
        }

        public static function log( string $message ): void {
            self::$captured[] = [ 'type' => 'log', 'message' => $message ];
        }

        public static function success( string $message ): void {
            self::$captured[] = [ 'type' => 'success', 'message' => $message ];
        }

        /**
         * Simulate WP_CLI::error() — records the message and throws to halt
         * execution, just like the real WP_CLI::error() calls exit().
         *
         * @throws \RuntimeException
         */
        public static function error( string $message ): void {
            self::$captured[] = [ 'type' => 'error', 'message' => $message ];
            throw new \RuntimeException( 'WP_CLI::error: ' . $message );
        }

        public static function reset(): void {
            self::$captured = [];
            self::$registered_commands = [];
        }
    }
}
