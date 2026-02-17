<?php
/**
 * Minimal WordPress function stubs for unit testing.
 *
 * These stubs allow unit tests to run without a real WordPress installation.
 * They track calls and return configurable values for assertions.
 *
 * @package Flapjack\WordPress\Tests
 */

declare(strict_types=1);

// Global state for stubs.
global $wp_options, $wp_actions, $wp_filters, $wp_transients, $wp_scheduled, $wp_posts_store, $wp_enqueued_scripts, $wp_enqueued_styles, $wp_localized_scripts, $wp_registered_rest_routes, $wp_current_user_can, $wp_last_json_response;

$wp_options              = [];
$wp_actions              = [];
$wp_filters              = [];
$wp_transients           = [];
$wp_scheduled            = [];
$wp_posts_store          = [];
$wp_enqueued_scripts     = [];
$wp_enqueued_styles      = [];
$wp_localized_scripts    = [];
$wp_registered_rest_routes = [];
$wp_current_user_can     = true;
$wp_last_json_response   = null;

/**
 * Reset all WordPress stub state between tests.
 */
function wp_stubs_reset(): void {
    global $wp_options, $wp_actions, $wp_filters, $wp_transients, $wp_scheduled, $wp_posts_store, $wp_enqueued_scripts, $wp_enqueued_styles, $wp_localized_scripts, $wp_registered_rest_routes, $wp_registered_blocks, $wp_inline_scripts, $as_scheduled_actions, $wp_current_user_can, $wp_last_json_response;
    $wp_options              = [];
    $wp_actions              = [];
    $wp_filters              = [];
    $wp_transients           = [];
    $wp_scheduled            = [];
    $wp_posts_store          = [];
    $wp_enqueued_scripts     = [];
    $wp_enqueued_styles      = [];
    $wp_localized_scripts    = [];
    $wp_registered_rest_routes = [];
    $wp_registered_blocks    = [];
    $wp_inline_scripts       = [];
    $as_scheduled_actions    = [];
    $wp_current_user_can     = true;
    $wp_last_json_response   = null;
}

// ─── Options API ──────────────────────────────────────────────

function get_option( string $option, $default = false ) {
    global $wp_options;
    return array_key_exists( $option, $wp_options ) ? $wp_options[ $option ] : $default;
}

function update_option( string $option, $value, $autoload = null ): bool {
    global $wp_options;
    $wp_options[ $option ] = $value;
    return true;
}

function add_option( string $option, $value = '', $deprecated = '', $autoload = 'yes' ): bool {
    global $wp_options;
    if ( ! array_key_exists( $option, $wp_options ) ) {
        $wp_options[ $option ] = $value;
    }
    return true;
}

function delete_option( string $option ): bool {
    global $wp_options;
    unset( $wp_options[ $option ] );
    return true;
}

// ─── Hooks API ────────────────────────────────────────────────

function add_action( string $hook, $callback, int $priority = 10, int $accepted_args = 1 ): void {
    global $wp_actions;
    $wp_actions[ $hook ][] = [
        'callback'      => $callback,
        'priority'      => $priority,
        'accepted_args' => $accepted_args,
    ];
}

function add_filter( string $hook, $callback, int $priority = 10, int $accepted_args = 1 ): void {
    global $wp_filters;
    $wp_filters[ $hook ][] = [
        'callback'      => $callback,
        'priority'      => $priority,
        'accepted_args' => $accepted_args,
    ];
}

function apply_filters( string $hook, $value, ...$args ) {
    global $wp_filters;
    if ( ! isset( $wp_filters[ $hook ] ) ) {
        return $value;
    }
    foreach ( $wp_filters[ $hook ] as $filter ) {
        $cb = $filter['callback'];
        if ( is_callable( $cb ) ) {
            $value = call_user_func( $cb, $value, ...$args );
        }
    }
    return $value;
}

function do_action( string $hook, ...$args ): void {
    global $wp_actions;
    if ( ! isset( $wp_actions[ $hook ] ) ) {
        return;
    }
    foreach ( $wp_actions[ $hook ] as $action ) {
        $cb = $action['callback'];
        if ( is_callable( $cb ) ) {
            call_user_func( $cb, ...$args );
        }
    }
}

function remove_action( string $hook, $callback, int $priority = 10 ): bool {
    return true;
}

function remove_filter( string $hook, $callback, int $priority = 10 ): bool {
    return true;
}

function has_filter( string $hook, $callback = false ) {
    global $wp_filters;
    return isset( $wp_filters[ $hook ] );
}

// ─── Transients API ───────────────────────────────────────────

function set_transient( string $transient, $value, int $expiration = 0 ): bool {
    global $wp_transients;
    $wp_transients[ $transient ] = $value;
    return true;
}

function get_transient( string $transient ) {
    global $wp_transients;
    return $wp_transients[ $transient ] ?? false;
}

function delete_transient( string $transient ): bool {
    global $wp_transients;
    unset( $wp_transients[ $transient ] );
    return true;
}

// ─── Cron API ─────────────────────────────────────────────────

function wp_clear_scheduled_hook( string $hook, array $args = [] ): int {
    global $wp_scheduled;
    if ( isset( $wp_scheduled[ $hook ] ) ) {
        unset( $wp_scheduled[ $hook ] );
        return 1;
    }
    return 0;
}

function wp_schedule_single_event( int $timestamp, string $hook, array $args = [], bool $wp_error = false ) {
    global $wp_scheduled;
    $wp_scheduled[ $hook ][] = [ 'timestamp' => $timestamp, 'args' => $args ];
    return true;
}

// ─── Constants ────────────────────────────────────────────────

if ( ! defined( 'HOUR_IN_SECONDS' ) ) {
    define( 'HOUR_IN_SECONDS', 3600 );
}

// ─── Action Scheduler stubs ──────────────────────────────────

global $as_scheduled_actions;
$as_scheduled_actions = [];

function as_schedule_single_action( int $timestamp, string $hook, array $args = [], string $group = '' ): int {
    global $as_scheduled_actions;
    $id = count( $as_scheduled_actions ) + 1;
    $as_scheduled_actions[] = [
        'id'        => $id,
        'timestamp' => $timestamp,
        'hook'      => $hook,
        'args'      => $args,
        'group'     => $group,
    ];
    return $id;
}

function as_unschedule_all_actions( string $hook, $args = null, string $group = '' ): void {
    global $as_scheduled_actions;
    $as_scheduled_actions = array_filter( $as_scheduled_actions, function ( $action ) use ( $hook, $group ) {
        if ( $action['hook'] !== $hook ) {
            return true;
        }
        if ( $group && $action['group'] !== $group ) {
            return true;
        }
        return false;
    } );
}

function as_stubs_reset(): void {
    global $as_scheduled_actions;
    $as_scheduled_actions = [];
}

// ─── Rewrite API ──────────────────────────────────────────────

function flush_rewrite_rules( bool $hard = true ): void {}

// ─── Registration hooks ───────────────────────────────────────

function register_activation_hook( string $file, $callback ): void {}
function register_deactivation_hook( string $file, $callback ): void {}

// ─── Text Domain ──────────────────────────────────────────────

function load_plugin_textdomain( string $domain, $deprecated = false, $plugin_rel_path = false ): bool {
    return true;
}

function __( string $text, string $domain = 'default' ): string {
    return $text;
}

function _e( string $text, string $domain = 'default' ): void {
    echo $text;
}

function esc_html__( string $text, string $domain = 'default' ): string {
    return $text;
}

function esc_html_e( string $text, string $domain = 'default' ): void {
    echo $text;
}

function esc_attr( string $text ): string {
    return htmlspecialchars( $text, ENT_QUOTES, 'UTF-8' );
}

function esc_html( string $text ): string {
    return htmlspecialchars( $text, ENT_QUOTES, 'UTF-8' );
}

function esc_url( string $url, $protocols = null, $context = 'display' ): string {
    return $url;
}

function esc_url_raw( string $url, $protocols = null ): string {
    return $url;
}

function esc_js( string $text ): string {
    return addslashes( $text );
}

// ─── Sanitization ─────────────────────────────────────────────

function sanitize_text_field( $str ): string {
    return trim( strip_tags( (string) $str ) );
}

function sanitize_key( string $key ): string {
    return preg_replace( '/[^a-z0-9_\-]/', '', strtolower( $key ) );
}

function absint( $maybeint ): int {
    return abs( (int) $maybeint );
}

function rest_sanitize_boolean( $value ): bool {
    return (bool) $value;
}

function wp_strip_all_tags( string $text, bool $remove_breaks = false ): string {
    return strip_tags( $text );
}

// ─── Post functions ───────────────────────────────────────────

if ( ! class_exists( 'WP_Post' ) ) {
    class WP_Post {
        public int $ID = 0;
        public string $post_author = '1';
        public string $post_date = '2026-01-01 00:00:00';
        public string $post_date_gmt = '2026-01-01 00:00:00';
        public string $post_content = '';
        public string $post_title = '';
        public string $post_excerpt = '';
        public string $post_status = 'publish';
        public string $post_type = 'post';
        public string $post_password = '';
        public string $post_name = '';
        public string $post_modified = '2026-01-01 00:00:00';
        public string $post_modified_gmt = '2026-01-01 00:00:00';
        public int $menu_order = 0;
        public int $comment_count = 0;

        public function __construct( array $data = [] ) {
            foreach ( $data as $key => $value ) {
                if ( property_exists( $this, $key ) ) {
                    $this->$key = $value;
                }
            }
        }
    }
}

if ( ! class_exists( 'WP_Term' ) ) {
    class WP_Term {
        public int $term_id = 0;
        public string $name = '';
        public string $slug = '';
        public string $taxonomy = '';

        public function __construct( array $data = [] ) {
            foreach ( $data as $key => $value ) {
                if ( property_exists( $this, $key ) ) {
                    $this->$key = $value;
                }
            }
        }
    }
}

if ( ! class_exists( 'WP_Query' ) ) {
    class WP_Query {
        public array $posts = [];
        public int $found_posts = 0;
        public int $max_num_pages = 0;
        public int $post_count = 0;
        private array $query_vars = [];
        private bool $is_search = false;

        public function __construct( $args = [] ) {
            if ( is_array( $args ) ) {
                $this->query_vars = $args;
                $this->is_search  = ! empty( $args['s'] );
                $this->run_query();
            }
        }

        /**
         * Simulate WP_Query by filtering the global posts store.
         */
        private function run_query(): void {
            global $wp_posts_store;

            if ( empty( $wp_posts_store ) ) {
                return;
            }

            $post_types    = (array) ( $this->query_vars['post_type'] ?? [ 'post' ] );
            $post_status   = $this->query_vars['post_status'] ?? 'publish';
            $per_page      = (int) ( $this->query_vars['posts_per_page'] ?? 10 );
            $paged         = max( 1, (int) ( $this->query_vars['paged'] ?? 1 ) );

            // Filter posts from the store.
            $matching = array_filter( $wp_posts_store, function ( $p ) use ( $post_types, $post_status ) {
                if ( ! in_array( $p->post_type, $post_types, true ) ) {
                    return false;
                }
                if ( $post_status !== 'any' && $p->post_status !== $post_status ) {
                    return false;
                }
                return true;
            } );

            $matching = array_values( $matching );
            $this->found_posts = count( $matching );
            $this->max_num_pages = $per_page > 0 ? (int) ceil( $this->found_posts / $per_page ) : 1;

            // Paginate.
            $offset = ( $paged - 1 ) * $per_page;
            $this->posts = array_slice( $matching, $offset, $per_page );
            $this->post_count = count( $this->posts );
        }

        public function get( string $key, $default = '' ) {
            return $this->query_vars[ $key ] ?? $default;
        }

        public function set( string $key, $value ): void {
            $this->query_vars[ $key ] = $value;
        }

        public function is_search(): bool {
            return $this->is_search;
        }

        public function set_is_search( bool $is_search ): void {
            $this->is_search = $is_search;
        }
    }
}

if ( ! class_exists( 'WP_Error' ) ) {
    class WP_Error {
        private string $code;
        private string $message;
        private array $data;

        public function __construct( string $code = '', string $message = '', $data = [] ) {
            $this->code    = $code;
            $this->message = $message;
            $this->data    = is_array( $data ) ? $data : [ $data ];
        }

        public function get_error_code(): string {
            return $this->code;
        }

        public function get_error_message(): string {
            return $this->message;
        }
    }
}

if ( ! class_exists( 'WP_REST_Request' ) ) {
    class WP_REST_Request {
        private array $params = [];
        private string $method;

        public function __construct( string $method = 'GET', string $route = '' ) {
            $this->method = $method;
        }

        public function get_param( string $key ) {
            return $this->params[ $key ] ?? null;
        }

        public function set_param( string $key, $value ): void {
            $this->params[ $key ] = $value;
        }
    }
}

if ( ! class_exists( 'WP_REST_Response' ) ) {
    class WP_REST_Response {
        private $data;
        private int $status;

        public function __construct( $data = null, int $status = 200 ) {
            $this->data   = $data;
            $this->status = $status;
        }

        public function get_data() {
            return $this->data;
        }

        public function get_status(): int {
            return $this->status;
        }
    }
}

function get_post( $post = null, $output = 'OBJECT', $filter = 'raw' ) {
    global $wp_posts_store;
    if ( $post instanceof WP_Post ) {
        return $post;
    }
    $id = (int) $post;
    return $wp_posts_store[ $id ] ?? null;
}

function get_posts( array $args = [] ): array {
    global $wp_posts_store;
    if ( ! empty( $args['post__in'] ) ) {
        $result = [];
        foreach ( $args['post__in'] as $id ) {
            if ( isset( $wp_posts_store[ $id ] ) ) {
                $result[] = $wp_posts_store[ $id ];
            }
        }
        return $result;
    }
    return array_values( $wp_posts_store );
}

function get_permalink( $post = 0, bool $leavename = false ): string {
    $id = $post instanceof WP_Post ? $post->ID : (int) $post;
    return "https://example.com/?p={$id}";
}

function get_the_author_meta( string $field, int $user_id = 0 ): string {
    return 'Test Author';
}

function get_post_thumbnail_id( $post = null ) {
    return 0;
}

function wp_get_attachment_image_url( int $attachment_id, $size = 'thumbnail' ) {
    return false;
}

function get_object_taxonomies( $object, string $output = 'names' ) {
    if ( $output === 'objects' ) {
        $cat = new \stdClass();
        $cat->name   = 'category';
        $cat->label  = 'Categories';
        $cat->public = true;

        $tag = new \stdClass();
        $tag->name   = 'post_tag';
        $tag->label  = 'Tags';
        $tag->public = true;

        return [ 'category' => $cat, 'post_tag' => $tag ];
    }
    return [ 'category', 'post_tag' ];
}

function get_the_terms( $post, string $taxonomy ) {
    return [];
}

function get_post_type_object( string $post_type ) {
    $obj = new \stdClass();
    $obj->labels = new \stdClass();
    $obj->labels->singular_name = ucfirst( $post_type );
    return $obj;
}

function get_post_types( array $args = [], string $output = 'names' ) {
    $types = [ 'post', 'page' ];
    if ( $output === 'objects' ) {
        $result = [];
        foreach ( $types as $type ) {
            $obj        = new \stdClass();
            $obj->name  = $type;
            $obj->label = ucfirst( $type ) . 's';
            $result[ $type ] = $obj;
        }
        return $result;
    }
    return $types;
}

function wp_count_posts( string $type = 'post' ) {
    global $wp_posts_store;
    $count = 0;
    foreach ( $wp_posts_store as $post ) {
        if ( $post->post_type === $type && $post->post_status === 'publish' ) {
            $count++;
        }
    }
    $obj = new \stdClass();
    $obj->publish = $count;
    $obj->draft   = 0;
    $obj->trash   = 0;
    return $obj;
}

function wp_is_post_revision( $post ): bool {
    return false;
}

function is_wp_error( $thing ): bool {
    return $thing instanceof WP_Error;
}

// ─── Shortcodes ───────────────────────────────────────────────

function strip_shortcodes( string $content ): string {
    return preg_replace( '/\[.*?\]/', '', $content );
}

function excerpt_remove_blocks( string $text ): string {
    return $text;
}

// ─── Admin functions ──────────────────────────────────────────

function is_admin(): bool {
    return false;
}

function current_user_can( string $capability, ...$args ): bool {
    global $wp_current_user_can;
    return (bool) $wp_current_user_can;
}

function admin_url( string $path = '' ): string {
    return 'https://example.com/wp-admin/' . $path;
}

function plugin_dir_path( string $file ): string {
    return dirname( $file ) . '/';
}

function plugin_dir_url( string $file ): string {
    return 'https://example.com/wp-content/plugins/' . basename( dirname( $file ) ) . '/';
}

function plugin_basename( string $file ): string {
    return basename( dirname( $file ) ) . '/' . basename( $file );
}

function get_admin_page_title(): string {
    return 'Flapjack Search Settings';
}

function rest_url( string $path = '' ): string {
    return 'https://example.com/wp-json/' . ltrim( $path, '/' );
}

// ─── Settings API ─────────────────────────────────────────────

function register_setting( string $option_group, string $option_name, array $args = [] ): void {}
function add_settings_section( string $id, string $title, $callback, string $page ): void {}
function add_settings_field( string $id, string $title, $callback, string $page, string $section = 'default', array $args = [] ): void {}
function settings_fields( string $option_group ): void {}
function do_settings_sections( string $page ): void {}
function submit_button(): void { echo '<input type="submit">'; }
function add_options_page( string $page_title, string $menu_title, string $capability, string $menu_slug, $callback = '' ): void {}

// ─── AJAX / Nonces ────────────────────────────────────────────

function check_ajax_referer( string $action, $query_arg = false, bool $die = true ): bool {
    return true;
}

function wp_create_nonce( string $action = '' ): string {
    return 'test_nonce_' . $action;
}

/**
 * Exception thrown by wp_send_json_* stubs to simulate wp_die() termination.
 */
class WPJsonResponseException extends \RuntimeException {
    public array $response;
    public function __construct( array $response ) {
        $this->response = $response;
        parent::__construct( 'wp_send_json terminated' );
    }
}

function wp_send_json_success( $data = null ): void {
    global $wp_last_json_response;
    $wp_last_json_response = [ 'success' => true, 'data' => $data ];
    throw new WPJsonResponseException( $wp_last_json_response );
}
function wp_send_json_error( $data = null ): void {
    global $wp_last_json_response;
    $wp_last_json_response = [ 'success' => false, 'data' => $data ];
    throw new WPJsonResponseException( $wp_last_json_response );
}

// ─── Enqueue API ──────────────────────────────────────────────

function wp_enqueue_script( string $handle, string $src = '', array $deps = [], $ver = false, $args = false ): void {
    global $wp_enqueued_scripts;
    $wp_enqueued_scripts[ $handle ] = [ 'src' => $src, 'deps' => $deps, 'ver' => $ver, 'args' => $args ];
}

function wp_enqueue_style( string $handle, string $src = '', array $deps = [], $ver = false, string $media = 'all' ): void {
    global $wp_enqueued_styles;
    $wp_enqueued_styles[ $handle ] = [ 'src' => $src, 'deps' => $deps, 'ver' => $ver, 'media' => $media ];
}

function wp_localize_script( string $handle, string $object_name, array $l10n ): void {
    global $wp_localized_scripts;
    $wp_localized_scripts[ $handle ] = [ 'object_name' => $object_name, 'data' => $l10n ];
}

// ─── REST API ─────────────────────────────────────────────────

function register_rest_route( string $namespace, string $route, array $args = [] ): void {
    global $wp_registered_rest_routes;
    $wp_registered_rest_routes[] = [ 'namespace' => $namespace, 'route' => $route, 'args' => $args ];
}

function checked( $checked, $current = true, bool $echo = true ): string {
    $result = (string) $checked === (string) $current ? " checked='checked'" : '';
    if ( $echo ) {
        echo $result;
    }
    return $result;
}

// ─── Block API ────────────────────────────────────────────────

global $wp_registered_blocks, $wp_inline_scripts;
$wp_registered_blocks = [];
$wp_inline_scripts    = [];

function register_block_type( $block_type, array $args = [] ) {
    global $wp_registered_blocks;
    if ( is_string( $block_type ) ) {
        // If it's a path to a directory with block.json, read it.
        $block_json_path = rtrim( $block_type, '/' ) . '/block.json';
        if ( file_exists( $block_json_path ) ) {
            $metadata = json_decode( file_get_contents( $block_json_path ), true );
            if ( $metadata ) {
                $name = $metadata['name'] ?? $block_type;
                $wp_registered_blocks[ $name ] = array_merge( $metadata, $args, [ 'path' => $block_type ] );
                $result        = new \stdClass();
                $result->name  = $name;
                $result->style = $metadata['style'] ?? null;
                return $result;
            }
        }
        $wp_registered_blocks[ $block_type ] = $args;
        $result       = new \stdClass();
        $result->name = $block_type;
        return $result;
    }
    return null;
}

function get_block_wrapper_attributes( array $extra_attributes = [] ): string {
    $attrs = '';
    foreach ( $extra_attributes as $key => $value ) {
        $attrs .= ' ' . esc_attr( $key ) . '="' . esc_attr( $value ) . '"';
    }
    return 'class="wp-block-flapjack-search"' . $attrs;
}

function get_search_query( bool $escaped = true ): string {
    return '';
}

function home_url( string $path = '', $scheme = null ): string {
    return 'https://example.com' . $path;
}

function wp_add_inline_script( string $handle, string $data, string $position = 'after' ): bool {
    global $wp_inline_scripts;
    $wp_inline_scripts[ $handle ][] = [ 'data' => $data, 'position' => $position ];
    return true;
}

function wp_set_script_translations( string $handle, string $domain = 'default', string $path = '' ): bool {
    return true;
}

function wp_json_encode( $data, int $options = 0, int $depth = 512 ) {
    return json_encode( $data, $options, $depth );
}

function wp_unslash( $value ) {
    if ( is_string( $value ) ) {
        return stripslashes( $value );
    }
    if ( is_array( $value ) ) {
        return array_map( 'wp_unslash', $value );
    }
    return $value;
}

