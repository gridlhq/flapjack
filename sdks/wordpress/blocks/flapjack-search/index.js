/**
 * Flapjack Search Block — Editor Script
 *
 * Minimal editor registration using wp.blocks and wp.blockEditor.
 * Uses server-side rendering (render.php) for the frontend.
 */
( function( blocks, element, blockEditor, components, i18n ) {
    var el = element.createElement;
    var __ = i18n.__;
    var useBlockProps = blockEditor.useBlockProps;
    var InspectorControls = blockEditor.InspectorControls;
    var PanelBody = components.PanelBody;
    var TextControl = components.TextControl;
    var ToggleControl = components.ToggleControl;
    var RangeControl = components.RangeControl;

    blocks.registerBlockType( 'flapjack/search', {
        edit: function( props ) {
            var attributes = props.attributes;
            var setAttributes = props.setAttributes;
            var blockProps = useBlockProps();

            return el( element.Fragment, {},
                el( InspectorControls, {},
                    el( PanelBody, { title: __( 'Search Settings', 'flapjack-search' ) },
                        el( TextControl, {
                            label: __( 'Placeholder text', 'flapjack-search' ),
                            value: attributes.placeholder,
                            onChange: function( val ) { setAttributes( { placeholder: val } ); }
                        }),
                        el( ToggleControl, {
                            label: __( 'Show search button', 'flapjack-search' ),
                            checked: attributes.showButton,
                            onChange: function( val ) { setAttributes( { showButton: val } ); }
                        }),
                        attributes.showButton && el( TextControl, {
                            label: __( 'Button text', 'flapjack-search' ),
                            value: attributes.buttonText,
                            onChange: function( val ) { setAttributes( { buttonText: val } ); }
                        }),
                        el( ToggleControl, {
                            label: __( 'Show autocomplete', 'flapjack-search' ),
                            checked: attributes.showAutocomplete,
                            onChange: function( val ) { setAttributes( { showAutocomplete: val } ); }
                        }),
                        attributes.showAutocomplete && el( RangeControl, {
                            label: __( 'Max suggestions', 'flapjack-search' ),
                            value: attributes.maxSuggestions,
                            onChange: function( val ) { setAttributes( { maxSuggestions: val } ); },
                            min: 1,
                            max: 20
                        })
                    )
                ),
                el( 'div', blockProps,
                    el( 'form', { role: 'search', className: 'flapjack-search-form' },
                        el( 'input', {
                            type: 'search',
                            placeholder: attributes.placeholder,
                            className: 'flapjack-search-input',
                            disabled: true
                        }),
                        attributes.showButton && el( 'button', {
                            type: 'button',
                            className: 'flapjack-search-button',
                            disabled: true
                        }, attributes.buttonText )
                    )
                )
            );
        }
    });
}(
    window.wp.blocks,
    window.wp.element,
    window.wp.blockEditor,
    window.wp.components,
    window.wp.i18n
) );
