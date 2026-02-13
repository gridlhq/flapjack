<?php
/**
 * Server-side rendering for the Flapjack Search block.
 *
 * @package Flapjack\WordPress
 *
 * @var array    $attributes Block attributes.
 * @var string   $content    Block inner content.
 * @var WP_Block $block      Block instance.
 */

// Prevent direct access.
if ( ! defined( 'ABSPATH' ) ) {
    exit;
}

$placeholder      = esc_attr( $attributes['placeholder'] ?? __( 'Search...', 'flapjack-search' ) );
$show_button      = $attributes['showButton'] ?? true;
$button_text      = esc_html( $attributes['buttonText'] ?? __( 'Search', 'flapjack-search' ) );
$show_autocomplete = $attributes['showAutocomplete'] ?? true;
$max_suggestions  = absint( $attributes['maxSuggestions'] ?? 5 );
?>
<div <?php echo get_block_wrapper_attributes(); ?>
     data-flapjack-autocomplete="<?php echo $show_autocomplete ? 'true' : 'false'; ?>"
     data-flapjack-max-suggestions="<?php echo $max_suggestions; ?>">
    <form role="search" method="get" action="<?php echo esc_url( home_url( '/' ) ); ?>" class="flapjack-search-form">
        <label class="screen-reader-text"><?php esc_html_e( 'Search for:', 'flapjack-search' ); ?></label>
        <input type="search"
               name="s"
               placeholder="<?php echo $placeholder; ?>"
               value="<?php echo esc_attr( get_search_query() ); ?>"
               class="flapjack-search-input"
               autocomplete="off" />
        <?php if ( $show_button ) : ?>
            <button type="submit" class="flapjack-search-button">
                <?php echo $button_text; ?>
            </button>
        <?php endif; ?>
    </form>
    <div class="flapjack-autocomplete-dropdown" style="display:none;"></div>
</div>
