<?php
/**
 * Admin settings page for Flapjack Search.
 *
 * @package Flapjack\WordPress\Admin
 */

declare(strict_types=1);

namespace Flapjack\WordPress\Admin;

class SettingsPage {

    public const OPTION_GROUP = 'flapjack_search_settings';
    public const PAGE_SLUG    = 'flapjack-search';

    /**
     * Register admin hooks.
     */
    public function register(): void {
        add_action( 'admin_menu', [ $this, 'add_menu_page' ] );
        add_action( 'admin_init', [ $this, 'register_settings' ] );
        add_action( 'admin_notices', [ $this, 'activation_notice' ] );
        add_action( 'admin_enqueue_scripts', [ $this, 'enqueue_admin_assets' ] );
        add_action( 'wp_ajax_flapjack_test_connection', [ $this, 'ajax_test_connection' ] );
        add_action( 'wp_ajax_flapjack_reindex', [ $this, 'ajax_reindex' ] );
        add_action( 'wp_ajax_flapjack_reindex_background', [ $this, 'ajax_reindex_background' ] );
        add_action( 'wp_ajax_flapjack_reindex_progress', [ $this, 'ajax_reindex_progress' ] );
        add_action( 'wp_ajax_flapjack_reindex_cancel', [ $this, 'ajax_reindex_cancel' ] );

        // Add settings link on plugins page.
        add_filter( 'plugin_action_links_' . FLAPJACK_SEARCH_BASENAME, [ $this, 'add_settings_link' ] );
    }

    /**
     * Add the settings menu page.
     */
    public function add_menu_page(): void {
        add_options_page(
            __( 'Flapjack Search Settings', 'flapjack-search' ),
            __( 'Flapjack Search', 'flapjack-search' ),
            'manage_options',
            self::PAGE_SLUG,
            [ $this, 'render_settings_page' ]
        );
    }

    /**
     * Register settings fields.
     */
    public function register_settings(): void {
        // Connection section.
        add_settings_section(
            'flapjack_connection',
            __( 'Connection Settings', 'flapjack-search' ),
            [ $this, 'render_connection_section' ],
            self::PAGE_SLUG
        );

        $this->add_text_field( 'flapjack_app_id', __( 'Application ID', 'flapjack-search' ), 'flapjack_connection' );
        $this->add_password_field( 'flapjack_api_key', __( 'Admin API Key', 'flapjack-search' ), 'flapjack_connection' );
        $this->add_password_field( 'flapjack_search_api_key', __( 'Search-Only API Key', 'flapjack-search' ), 'flapjack_connection', __( 'Required for Instant Search. Used for frontend search requests. The Admin API Key is never exposed to visitors.', 'flapjack-search' ) );
        $this->add_text_field( 'flapjack_host', __( 'Custom Host', 'flapjack-search' ), 'flapjack_connection', __( 'Leave blank for Flapjack Cloud. For self-hosted, enter your server URL (e.g., http://localhost:7700).', 'flapjack-search' ) );

        // Indexing section.
        add_settings_section(
            'flapjack_indexing',
            __( 'Indexing Settings', 'flapjack-search' ),
            [ $this, 'render_indexing_section' ],
            self::PAGE_SLUG
        );

        $this->add_text_field( 'flapjack_index_name', __( 'Index Name', 'flapjack-search' ), 'flapjack_indexing' );

        register_setting( self::OPTION_GROUP, 'flapjack_post_types', [
            'type'              => 'array',
            'sanitize_callback' => [ $this, 'sanitize_post_types' ],
            'default'           => [ 'post', 'page' ],
        ] );

        add_settings_field(
            'flapjack_post_types',
            __( 'Post Types to Index', 'flapjack-search' ),
            [ $this, 'render_post_types_field' ],
            self::PAGE_SLUG,
            'flapjack_indexing'
        );

        register_setting( self::OPTION_GROUP, 'flapjack_searchable_attrs', [
            'type'              => 'array',
            'sanitize_callback' => [ $this, 'sanitize_searchable_attrs' ],
            'default'           => [ 'post_title', 'post_content', 'post_excerpt' ],
        ] );

        add_settings_field(
            'flapjack_searchable_attrs',
            __( 'Searchable Attributes', 'flapjack-search' ),
            [ $this, 'render_searchable_attrs_field' ],
            self::PAGE_SLUG,
            'flapjack_indexing'
        );

        // Search section.
        add_settings_section(
            'flapjack_search',
            __( 'Search Settings', 'flapjack-search' ),
            [ $this, 'render_search_section' ],
            self::PAGE_SLUG
        );

        register_setting( self::OPTION_GROUP, 'flapjack_enable_search', [
            'type'              => 'boolean',
            'sanitize_callback' => 'rest_sanitize_boolean',
            'default'           => true,
        ] );

        add_settings_field(
            'flapjack_enable_search',
            __( 'Enable Backend Search', 'flapjack-search' ),
            [ $this, 'render_checkbox_field' ],
            self::PAGE_SLUG,
            'flapjack_search',
            [
                'id'          => 'flapjack_enable_search',
                'description' => __( 'Replace WordPress native search with Flapjack on the backend (via posts_pre_query).', 'flapjack-search' ),
            ]
        );

        register_setting( self::OPTION_GROUP, 'flapjack_enable_instant', [
            'type'              => 'boolean',
            'sanitize_callback' => 'rest_sanitize_boolean',
            'default'           => false,
        ] );

        add_settings_field(
            'flapjack_enable_instant',
            __( 'Enable Instant Search', 'flapjack-search' ),
            [ $this, 'render_checkbox_field' ],
            self::PAGE_SLUG,
            'flapjack_search',
            [
                'id'          => 'flapjack_enable_instant',
                'description' => __( 'Load InstantSearch.js on the frontend for instant, as-you-type search results.', 'flapjack-search' ),
            ]
        );

        register_setting( self::OPTION_GROUP, 'flapjack_posts_per_page', [
            'type'              => 'integer',
            'sanitize_callback' => 'absint',
            'default'           => 20,
        ] );

        add_settings_field(
            'flapjack_posts_per_page',
            __( 'Results Per Page', 'flapjack-search' ),
            [ $this, 'render_number_field' ],
            self::PAGE_SLUG,
            'flapjack_search',
            [
                'id'  => 'flapjack_posts_per_page',
                'min' => 1,
                'max' => 100,
            ]
        );
    }

    /**
     * Render the settings page.
     */
    public function render_settings_page(): void {
        if ( ! current_user_can( 'manage_options' ) ) {
            return;
        }
        ?>
        <div class="wrap">
            <h1><?php echo esc_html( get_admin_page_title() ); ?></h1>

            <div class="notice notice-info inline" style="margin-top:10px;">
                <p><?php esc_html_e( 'Flapjack Search is currently in beta. Please report issues or feedback on our GitHub repository.', 'flapjack-search' ); ?></p>
            </div>

            <form action="options.php" method="post">
                <?php
                settings_fields( self::OPTION_GROUP );
                do_settings_sections( self::PAGE_SLUG );
                submit_button();
                ?>
            </form>

            <hr>
            <h2><?php esc_html_e( 'Tools', 'flapjack-search' ); ?></h2>
            <table class="form-table">
                <tr>
                    <th scope="row"><?php esc_html_e( 'Test Connection', 'flapjack-search' ); ?></th>
                    <td>
                        <button type="button" class="button" id="flapjack-test-connection">
                            <?php esc_html_e( 'Test Connection', 'flapjack-search' ); ?>
                        </button>
                        <span id="flapjack-test-result"></span>
                    </td>
                </tr>
                <tr>
                    <th scope="row"><?php esc_html_e( 'Reindex Content', 'flapjack-search' ); ?></th>
                    <td>
                        <button type="button" class="button" id="flapjack-reindex">
                            <?php esc_html_e( 'Reindex All Content', 'flapjack-search' ); ?>
                        </button>
                        <span id="flapjack-reindex-result"></span>
                        <p class="description">
                            <?php esc_html_e( 'Re-send all configured post types to the Flapjack index. This may take a moment for large sites.', 'flapjack-search' ); ?>
                        </p>
                    </td>
                </tr>
                <tr>
                    <th scope="row"><?php esc_html_e( 'Background Reindex', 'flapjack-search' ); ?></th>
                    <td>
                        <button type="button" class="button" id="flapjack-reindex-background">
                            <?php esc_html_e( 'Start Background Reindex', 'flapjack-search' ); ?>
                        </button>
                        <button type="button" class="button" id="flapjack-reindex-cancel" style="display:none;">
                            <?php esc_html_e( 'Cancel', 'flapjack-search' ); ?>
                        </button>
                        <div id="flapjack-reindex-progress" style="display:none;margin-top:8px;">
                            <div class="flapjack-progress-bar" style="width:100%;background:#ddd;border-radius:3px;overflow:hidden;">
                                <div class="flapjack-progress-fill" style="width:0%;height:20px;background:#0073aa;transition:width 0.3s;"></div>
                            </div>
                            <span class="flapjack-progress-text" style="display:inline-block;margin-top:4px;"></span>
                        </div>
                        <span id="flapjack-reindex-bg-result"></span>
                        <p class="description">
                            <?php esc_html_e( 'Reindex in the background using batched processing. Recommended for large sites (1000+ posts). Uses Action Scheduler if available.', 'flapjack-search' ); ?>
                        </p>
                    </td>
                </tr>
            </table>

        </div>
        <?php
    }

    /**
     * Section descriptions.
     */
    public function render_connection_section(): void {
        echo '<p>' . esc_html__( 'Enter your Flapjack API credentials. You can find these in your Flapjack dashboard.', 'flapjack-search' ) . '</p>';
    }

    public function render_indexing_section(): void {
        echo '<p>' . esc_html__( 'Configure which content gets indexed in Flapjack.', 'flapjack-search' ) . '</p>';
    }

    public function render_search_section(): void {
        echo '<p>' . esc_html__( 'Control how Flapjack integrates with your site\'s search.', 'flapjack-search' ) . '</p>';
    }

    /**
     * Enqueue admin JavaScript on the settings page only.
     *
     * @param string $hook_suffix The current admin page hook suffix.
     */
    public function enqueue_admin_assets( string $hook_suffix = '' ): void {
        if ( 'settings_page_' . self::PAGE_SLUG !== $hook_suffix ) {
            return;
        }

        wp_enqueue_script(
            'flapjack-search-admin',
            FLAPJACK_SEARCH_URL . 'assets/js/admin.js',
            [ 'jquery' ],
            FLAPJACK_SEARCH_VERSION,
            true
        );

        wp_localize_script( 'flapjack-search-admin', 'flapjackAdminConfig', [
            'testNonce'            => wp_create_nonce( 'flapjack_test_connection' ),
            'reindexNonce'         => wp_create_nonce( 'flapjack_reindex' ),
            'reindexBgNonce'       => wp_create_nonce( 'flapjack_reindex_background' ),
            'reindexProgressNonce' => wp_create_nonce( 'flapjack_reindex_progress' ),
            'reindexCancelNonce'   => wp_create_nonce( 'flapjack_reindex_cancel' ),
            'i18n'                 => [
                'testing'     => __( 'Testing...', 'flapjack-search' ),
                'reindexing'  => __( 'Reindexing...', 'flapjack-search' ),
                'starting'    => __( 'Starting background reindex...', 'flapjack-search' ),
                'cancelling'  => __( 'Cancelling...', 'flapjack-search' ),
                'complete'    => __( 'Complete!', 'flapjack-search' ),
                'cancelled'   => __( 'Cancelled.', 'flapjack-search' ),
                'failed'      => __( 'Failed.', 'flapjack-search' ),
                'progressFmt' => __( 'Indexed %1$d of %2$d posts (%3$d%%)', 'flapjack-search' ),
            ],
        ] );
    }

    /**
     * Render a text input field.
     */
    public function render_text_field( array $args ): void {
        $id    = $args['id'];
        $value = get_option( $id, '' );
        printf(
            '<input type="text" id="%1$s" name="%1$s" value="%2$s" class="regular-text">',
            esc_attr( $id ),
            esc_attr( (string) $value )
        );
        if ( ! empty( $args['description'] ) ) {
            printf( '<p class="description">%s</p>', esc_html( $args['description'] ) );
        }
    }

    /**
     * Render a password input field.
     */
    public function render_password_field( array $args ): void {
        $id    = $args['id'];
        $value = get_option( $id, '' );
        printf(
            '<input type="password" id="%1$s" name="%1$s" value="%2$s" class="regular-text">',
            esc_attr( $id ),
            esc_attr( (string) $value )
        );
        if ( ! empty( $args['description'] ) ) {
            printf( '<p class="description">%s</p>', esc_html( $args['description'] ) );
        }
    }

    /**
     * Render a checkbox field.
     */
    public function render_checkbox_field( array $args ): void {
        $id    = $args['id'];
        $value = get_option( $id, false );
        printf(
            '<label><input type="checkbox" id="%1$s" name="%1$s" value="1" %2$s> %3$s</label>',
            esc_attr( $id ),
            checked( $value, true, false ),
            esc_html( $args['description'] ?? '' )
        );
    }

    /**
     * Render a number input field.
     */
    public function render_number_field( array $args ): void {
        $id    = $args['id'];
        $value = get_option( $id, 20 );
        printf(
            '<input type="number" id="%1$s" name="%1$s" value="%2$s" min="%3$d" max="%4$d" class="small-text">',
            esc_attr( $id ),
            esc_attr( (string) $value ),
            (int) ( $args['min'] ?? 1 ),
            (int) ( $args['max'] ?? 100 )
        );
    }

    /**
     * Render post types checkboxes.
     */
    public function render_post_types_field(): void {
        $selected   = (array) get_option( 'flapjack_post_types', [ 'post', 'page' ] );
        $post_types = get_post_types( [ 'public' => true ], 'objects' );

        foreach ( $post_types as $post_type ) {
            printf(
                '<label style="display:block;margin-bottom:5px;"><input type="checkbox" name="flapjack_post_types[]" value="%1$s" %2$s> %3$s <code>(%1$s)</code></label>',
                esc_attr( $post_type->name ),
                checked( in_array( $post_type->name, $selected, true ), true, false ),
                esc_html( $post_type->label )
            );
        }
    }

    /**
     * Render searchable attributes checkboxes.
     */
    public function render_searchable_attrs_field(): void {
        $selected   = (array) get_option( 'flapjack_searchable_attrs', [ 'post_title', 'post_content', 'post_excerpt' ] );
        $attributes = [
            'post_title'   => __( 'Title', 'flapjack-search' ),
            'post_content' => __( 'Content', 'flapjack-search' ),
            'post_excerpt' => __( 'Excerpt', 'flapjack-search' ),
            'taxonomies'   => __( 'Taxonomy Terms (categories, tags)', 'flapjack-search' ),
            'author'       => __( 'Author Name', 'flapjack-search' ),
            'meta'         => __( 'Custom Fields (post meta)', 'flapjack-search' ),
        ];

        foreach ( $attributes as $key => $label ) {
            printf(
                '<label style="display:block;margin-bottom:5px;"><input type="checkbox" name="flapjack_searchable_attrs[]" value="%1$s" %2$s> %3$s</label>',
                esc_attr( $key ),
                checked( in_array( $key, $selected, true ), true, false ),
                esc_html( $label )
            );
        }
    }

    /**
     * Sanitize post types array.
     *
     * @param mixed $value
     * @return string[]
     */
    public function sanitize_post_types( $value ): array {
        if ( ! is_array( $value ) ) {
            return [ 'post', 'page' ];
        }
        return array_map( 'sanitize_key', $value );
    }

    /**
     * Sanitize searchable attributes array.
     *
     * @param mixed $value
     * @return string[]
     */
    public function sanitize_searchable_attrs( $value ): array {
        if ( ! is_array( $value ) ) {
            return [ 'post_title', 'post_content', 'post_excerpt' ];
        }
        $allowed = [ 'post_title', 'post_content', 'post_excerpt', 'taxonomies', 'author', 'meta' ];
        return array_intersect( array_map( 'sanitize_key', $value ), $allowed );
    }

    /**
     * Show activation notice.
     */
    public function activation_notice(): void {
        if ( ! get_transient( 'flapjack_search_activated' ) ) {
            return;
        }
        delete_transient( 'flapjack_search_activated' );
        printf(
            '<div class="notice notice-success is-dismissible"><p>%s <a href="%s">%s</a></p></div>',
            esc_html__( 'Flapjack Search activated!', 'flapjack-search' ),
            esc_url( admin_url( 'options-general.php?page=' . self::PAGE_SLUG ) ),
            esc_html__( 'Configure your API credentials to get started.', 'flapjack-search' )
        );
    }

    /**
     * Add settings link to plugins page.
     *
     * @param string[] $links
     * @return string[]
     */
    public function add_settings_link( array $links ): array {
        $settings_link = sprintf(
            '<a href="%s">%s</a>',
            esc_url( admin_url( 'options-general.php?page=' . self::PAGE_SLUG ) ),
            esc_html__( 'Settings', 'flapjack-search' )
        );
        array_unshift( $links, $settings_link );
        return $links;
    }

    /**
     * AJAX: Test connection.
     */
    public function ajax_test_connection(): void {
        check_ajax_referer( 'flapjack_test_connection' );

        if ( ! current_user_can( 'manage_options' ) ) {
            wp_send_json_error( [ 'message' => __( 'Permission denied.', 'flapjack-search' ) ] );
        }

        $factory = \Flapjack\WordPress\Plugin::get_instance()->get_client_factory();
        $result  = $factory->test_connection();

        if ( $result['success'] ) {
            wp_send_json_success( $result );
        } else {
            wp_send_json_error( $result );
        }
    }

    /**
     * AJAX: Trigger reindex.
     */
    public function ajax_reindex(): void {
        check_ajax_referer( 'flapjack_reindex' );

        if ( ! current_user_can( 'manage_options' ) ) {
            wp_send_json_error( [ 'message' => __( 'Permission denied.', 'flapjack-search' ) ] );
        }

        try {
            $factory       = \Flapjack\WordPress\Plugin::get_instance()->get_client_factory();
            $index_manager = new \Flapjack\WordPress\Indexing\IndexManager( $factory );
            $result        = $index_manager->reindex_all();

            wp_send_json_success( [
                'message' => sprintf(
                    /* translators: %d: number of objects indexed */
                    __( 'Reindex complete. %d objects indexed.', 'flapjack-search' ),
                    $result['total']
                ),
            ] );
        } catch ( \Throwable $e ) {
            wp_send_json_error( [ 'message' => $e->getMessage() ] );
        }
    }

    /**
     * AJAX: Start background reindex.
     */
    public function ajax_reindex_background(): void {
        check_ajax_referer( 'flapjack_reindex_background' );

        if ( ! current_user_can( 'manage_options' ) ) {
            wp_send_json_error( [ 'message' => __( 'Permission denied.', 'flapjack-search' ) ] );
        }

        try {
            $factory   = \Flapjack\WordPress\Plugin::get_instance()->get_client_factory();
            $indexer   = new \Flapjack\WordPress\Indexing\BackgroundIndexer( $factory );
            $progress  = $indexer->start_reindex();

            wp_send_json_success( $progress );
        } catch ( \Throwable $e ) {
            wp_send_json_error( [ 'message' => $e->getMessage() ] );
        }
    }

    /**
     * AJAX: Get background reindex progress.
     */
    public function ajax_reindex_progress(): void {
        check_ajax_referer( 'flapjack_reindex_progress' );

        if ( ! current_user_can( 'manage_options' ) ) {
            wp_send_json_error( [ 'message' => __( 'Permission denied.', 'flapjack-search' ) ] );
        }

        $factory  = \Flapjack\WordPress\Plugin::get_instance()->get_client_factory();
        $indexer  = new \Flapjack\WordPress\Indexing\BackgroundIndexer( $factory );
        $progress = $indexer->get_progress();

        if ( $progress ) {
            wp_send_json_success( $progress );
        } else {
            wp_send_json_error( [ 'message' => __( 'No reindex in progress.', 'flapjack-search' ) ] );
        }
    }

    /**
     * AJAX: Cancel background reindex.
     */
    public function ajax_reindex_cancel(): void {
        check_ajax_referer( 'flapjack_reindex_cancel' );

        if ( ! current_user_can( 'manage_options' ) ) {
            wp_send_json_error( [ 'message' => __( 'Permission denied.', 'flapjack-search' ) ] );
        }

        $factory  = \Flapjack\WordPress\Plugin::get_instance()->get_client_factory();
        $indexer  = new \Flapjack\WordPress\Indexing\BackgroundIndexer( $factory );
        $cancelled = $indexer->cancel_reindex();

        if ( $cancelled ) {
            wp_send_json_success( [ 'message' => __( 'Reindex cancelled.', 'flapjack-search' ) ] );
        } else {
            wp_send_json_error( [ 'message' => __( 'No reindex in progress to cancel.', 'flapjack-search' ) ] );
        }
    }

    /**
     * Helper to register a text field.
     */
    private function add_text_field( string $id, string $label, string $section, string $description = '' ): void {
        register_setting( self::OPTION_GROUP, $id, [
            'type'              => 'string',
            'sanitize_callback' => 'sanitize_text_field',
            'default'           => '',
        ] );

        add_settings_field(
            $id,
            $label,
            [ $this, 'render_text_field' ],
            self::PAGE_SLUG,
            $section,
            [ 'id' => $id, 'description' => $description ]
        );
    }

    /**
     * Helper to register a password field.
     */
    private function add_password_field( string $id, string $label, string $section, string $description = '' ): void {
        register_setting( self::OPTION_GROUP, $id, [
            'type'              => 'string',
            'sanitize_callback' => 'sanitize_text_field',
            'default'           => '',
        ] );

        add_settings_field(
            $id,
            $label,
            [ $this, 'render_password_field' ],
            self::PAGE_SLUG,
            $section,
            [ 'id' => $id, 'description' => $description ]
        );
    }
}
