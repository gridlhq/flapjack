<?php
/**
 * WooCommerce stubs for unit testing.
 *
 * Minimal WC_Product and related class stubs to test WooCommerce integration
 * without requiring a WooCommerce installation.
 *
 * @package Flapjack\WordPress\Tests
 */

declare(strict_types=1);

// ─── WC_Product ───────────────────────────────────────────────

class WC_Product {

    protected int $id = 0;
    protected string $name = '';
    protected string $description = '';
    protected string $short_description = '';
    protected string $sku = '';
    protected string $price = '';
    protected string $regular_price = '';
    protected string $sale_price = '';
    protected string $stock_status = 'instock';
    protected int $total_sales = 0;
    protected string $average_rating = '0';
    protected int $rating_count = 0;
    protected int $review_count = 0;
    protected bool $featured = false;
    protected string $type = 'simple';
    protected string $weight = '';
    protected string $catalog_visibility = 'visible';
    protected int $image_id = 0;
    protected string $permalink = '';
    protected array $attributes = [];
    protected array $category_ids = [];
    protected array $tag_ids = [];

    public function __construct( int $id = 0 ) {
        $this->id = $id;
    }

    public function get_id(): int {
        return $this->id;
    }

    public function get_name(): string {
        return $this->name;
    }

    public function set_name( string $name ): void {
        $this->name = $name;
    }

    public function get_description(): string {
        return $this->description;
    }

    public function set_description( string $description ): void {
        $this->description = $description;
    }

    public function get_short_description(): string {
        return $this->short_description;
    }

    public function set_short_description( string $short_description ): void {
        $this->short_description = $short_description;
    }

    public function get_sku(): string {
        return $this->sku;
    }

    public function set_sku( string $sku ): void {
        $this->sku = $sku;
    }

    public function get_price(): string {
        return $this->price;
    }

    public function set_price( string $price ): void {
        $this->price = $price;
    }

    public function get_regular_price(): string {
        return $this->regular_price;
    }

    public function set_regular_price( string $regular_price ): void {
        $this->regular_price = $regular_price;
    }

    public function get_sale_price(): string {
        return $this->sale_price;
    }

    public function set_sale_price( string $sale_price ): void {
        $this->sale_price = $sale_price;
    }

    public function is_on_sale(): bool {
        return '' !== $this->sale_price && (float) $this->sale_price < (float) $this->regular_price;
    }

    public function get_stock_status(): string {
        return $this->stock_status;
    }

    public function set_stock_status( string $status ): void {
        $this->stock_status = $status;
    }

    public function is_in_stock(): bool {
        return 'instock' === $this->stock_status;
    }

    public function get_total_sales(): int {
        return $this->total_sales;
    }

    public function set_total_sales( int $total_sales ): void {
        $this->total_sales = $total_sales;
    }

    public function get_average_rating(): string {
        return $this->average_rating;
    }

    public function set_average_rating( string $rating ): void {
        $this->average_rating = $rating;
    }

    public function get_rating_count(): int {
        return $this->rating_count;
    }

    public function set_rating_count( int $count ): void {
        $this->rating_count = $count;
    }

    public function get_review_count(): int {
        return $this->review_count;
    }

    public function set_review_count( int $count ): void {
        $this->review_count = $count;
    }

    public function is_featured(): bool {
        return $this->featured;
    }

    public function set_featured( bool $featured ): void {
        $this->featured = $featured;
    }

    public function get_type(): string {
        return $this->type;
    }

    public function set_type( string $type ): void {
        $this->type = $type;
    }

    public function is_type( string $type ): bool {
        return $this->type === $type;
    }

    public function get_weight(): string {
        return $this->weight;
    }

    public function set_weight( string $weight ): void {
        $this->weight = $weight;
    }

    public function get_catalog_visibility(): string {
        return $this->catalog_visibility;
    }

    public function set_catalog_visibility( string $visibility ): void {
        $this->catalog_visibility = $visibility;
    }

    public function get_image_id(): int {
        return $this->image_id;
    }

    public function set_image_id( int $image_id ): void {
        $this->image_id = $image_id;
    }

    public function get_permalink(): string {
        return $this->permalink ?: 'https://example.com/product/' . $this->id;
    }

    public function set_permalink( string $permalink ): void {
        $this->permalink = $permalink;
    }

    public function get_attributes(): array {
        return $this->attributes;
    }

    public function set_attributes( array $attributes ): void {
        $this->attributes = $attributes;
    }

    public function get_category_ids(): array {
        return $this->category_ids;
    }

    public function set_category_ids( array $ids ): void {
        $this->category_ids = $ids;
    }

    public function get_tag_ids(): array {
        return $this->tag_ids;
    }

    public function set_tag_ids( array $ids ): void {
        $this->tag_ids = $ids;
    }

    public function get_children(): array {
        return [];
    }
}

// ─── WC_Product_Variable ──────────────────────────────────────

class WC_Product_Variable extends WC_Product {

    protected array $children = [];

    public function __construct( int $id = 0 ) {
        parent::__construct( $id );
        $this->type = 'variable';
    }

    public function set_children( array $children ): void {
        $this->children = $children;
    }

    public function get_children(): array {
        return $this->children;
    }
}

// ─── WC_Product_Attribute ─────────────────────────────────────

class WC_Product_Attribute {

    private string $name = '';
    private array $options = [];
    private bool $is_taxonomy = false;

    public function get_name(): string {
        return $this->name;
    }

    public function set_name( string $name ): void {
        $this->name = $name;
    }

    public function get_options(): array {
        return $this->options;
    }

    public function set_options( array $options ): void {
        $this->options = $options;
    }

    public function is_taxonomy(): bool {
        return $this->is_taxonomy;
    }

    public function set_is_taxonomy( bool $is_taxonomy ): void {
        $this->is_taxonomy = $is_taxonomy;
    }
}

// ─── WooCommerce class stub ───────────────────────────────────

class WooCommerce {
    // Minimal stub — existence check is all we need.
}

// ─── Global product store for testing ─────────────────────────

global $wc_products_store;
$wc_products_store = [];

/**
 * Store a product for retrieval via wc_get_product().
 */
function wc_store_test_product( WC_Product $product ): void {
    global $wc_products_store;
    $wc_products_store[ $product->get_id() ] = $product;
}

/**
 * Get a product by ID (WooCommerce stub).
 *
 * @param int $product_id
 * @return WC_Product|false
 */
function wc_get_product( int $product_id ): WC_Product|false {
    global $wc_products_store;
    return $wc_products_store[ $product_id ] ?? false;
}

/**
 * Get the WooCommerce currency symbol.
 *
 * @param string $currency Currency code.
 * @return string Currency symbol.
 */
function get_woocommerce_currency_symbol( string $currency = '' ): string {
    global $wc_currency_symbol;
    return $wc_currency_symbol ?? '$';
}

/**
 * Set the WooCommerce currency symbol for testing.
 */
function wc_set_currency_symbol( string $symbol ): void {
    global $wc_currency_symbol;
    $wc_currency_symbol = $symbol;
}

/**
 * Reset WooCommerce stub state between tests.
 */
function wc_stubs_reset(): void {
    global $wc_products_store, $wc_currency_symbol;
    $wc_products_store = [];
    $wc_currency_symbol = '$';
}
