/**
 * Flapjack Search Block — Frontend View Script
 *
 * Provides autocomplete functionality for Flapjack Search blocks.
 * Reads configuration from flapjackSearchConfig (wp_localize_script)
 * and fetches results from the REST API.
 */
( function() {
    'use strict';

    var config = window.flapjackSearchConfig || {};
    if ( ! config.restUrl ) {
        return;
    }

    var blocks = document.querySelectorAll( '.wp-block-flapjack-search[data-flapjack-autocomplete="true"]' );
    if ( ! blocks.length ) {
        return;
    }

    var debounceTimer = null;

    function debounce( fn, delay ) {
        return function() {
            var context = this;
            var args = arguments;
            clearTimeout( debounceTimer );
            debounceTimer = setTimeout( function() {
                fn.apply( context, args );
            }, delay );
        };
    }

    function escapeHtml( str ) {
        var div = document.createElement( 'div' );
        div.appendChild( document.createTextNode( str ) );
        return div.innerHTML;
    }

    /**
     * Sanitize highlight HTML — only allow <mark> tags for highlighting.
     * Strip all other HTML to prevent XSS from API responses.
     */
    function sanitizeHighlight( html ) {
        // Temporarily replace <mark> and </mark> with placeholders.
        var safe = html.replace( /<mark>/gi, '\x00MARK_OPEN\x00' )
                       .replace( /<\/mark>/gi, '\x00MARK_CLOSE\x00' );
        // Escape everything else.
        safe = escapeHtml( safe );
        // Restore <mark> tags.
        safe = safe.replace( /\x00MARK_OPEN\x00/g, '<mark>' )
                   .replace( /\x00MARK_CLOSE\x00/g, '</mark>' );
        return safe;
    }

    function initBlock( block ) {
        var input    = block.querySelector( '.flapjack-search-input' );
        var dropdown = block.querySelector( '.flapjack-autocomplete-dropdown' );
        var maxItems = parseInt( block.getAttribute( 'data-flapjack-max-suggestions' ) || '5', 10 );
        var activeIndex = -1;

        if ( ! input || ! dropdown ) {
            return;
        }

        var fetchResults = debounce( function( query ) {
            if ( query.length < 2 ) {
                dropdown.style.display = 'none';
                dropdown.innerHTML = '';
                return;
            }

            var url = config.restUrl + 'search?q=' + encodeURIComponent( query ) + '&per_page=' + maxItems;
            fetch( url, {
                headers: { 'X-WP-Nonce': config.nonce || '' }
            })
            .then( function( response ) { return response.json(); } )
            .then( function( data ) {
                var hits = data.hits || [];
                if ( ! hits.length ) {
                    dropdown.style.display = 'none';
                    dropdown.innerHTML = '';
                    return;
                }
                var html = '';
                hits.forEach( function( hit ) {
                    var title   = hit._highlightResult && hit._highlightResult.post_title
                                  ? sanitizeHighlight( hit._highlightResult.post_title.value )
                                  : escapeHtml( hit.post_title || '' );
                    var excerpt = hit.post_excerpt ? escapeHtml( hit.post_excerpt ).substring( 0, 100 ) : '';
                    var type    = hit.post_type_label || hit.post_type || '';
                    var link    = hit.permalink || '#';

                    html += '<a href="' + escapeHtml( link ) + '" class="flapjack-autocomplete-item">';
                    html += '<div class="title">' + title;
                    if ( type ) {
                        html += '<span class="type-badge">' + escapeHtml( type ) + '</span>';
                    }
                    html += '</div>';
                    if ( excerpt ) {
                        html += '<div class="excerpt">' + excerpt + '</div>';
                    }
                    html += '</a>';
                });
                dropdown.innerHTML = html;
                dropdown.style.display = 'block';
                activeIndex = -1;
            })
            .catch( function() {
                dropdown.style.display = 'none';
            });
        }, 200 );

        input.addEventListener( 'input', function() {
            fetchResults( input.value.trim() );
        });

        input.addEventListener( 'keydown', function( e ) {
            var items = dropdown.querySelectorAll( '.flapjack-autocomplete-item' );
            if ( ! items.length ) return;

            if ( e.key === 'ArrowDown' ) {
                e.preventDefault();
                activeIndex = Math.min( activeIndex + 1, items.length - 1 );
                updateActive( items );
            } else if ( e.key === 'ArrowUp' ) {
                e.preventDefault();
                activeIndex = Math.max( activeIndex - 1, -1 );
                updateActive( items );
            } else if ( e.key === 'Enter' && activeIndex >= 0 ) {
                e.preventDefault();
                items[ activeIndex ].click();
            } else if ( e.key === 'Escape' ) {
                dropdown.style.display = 'none';
                activeIndex = -1;
            }
        });

        function updateActive( items ) {
            items.forEach( function( item, i ) {
                item.classList.toggle( 'active', i === activeIndex );
            });
        }

        // Close dropdown on outside click.
        document.addEventListener( 'click', function( e ) {
            if ( ! block.contains( e.target ) ) {
                dropdown.style.display = 'none';
                activeIndex = -1;
            }
        });
    }

    // Initialize all Flapjack search blocks on the page.
    blocks.forEach( initBlock );
})();
