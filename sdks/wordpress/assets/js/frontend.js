/**
 * Flapjack Search — InstantSearch.js frontend integration.
 *
 * @package flapjack-search
 */

(function () {
    'use strict';

    var config = window.flapjackSearchConfig || {};

    if (!config.appId || !config.apiKey || !config.indexName) {
        return;
    }

    // Wait for DOM and dependencies.
    document.addEventListener('DOMContentLoaded', function () {
        if (typeof window.instantsearch === 'undefined') {
            console.warn('[Flapjack Search] InstantSearch.js not loaded.');
            return;
        }

        initInstantSearch(config);
        initAutocomplete(config);
    });

    /**
     * Initialize the full InstantSearch experience.
     */
    function initInstantSearch(config) {
        var searchBoxEl = document.getElementById('flapjack-searchbox');
        var hitsEl = document.getElementById('flapjack-hits');

        if (!searchBoxEl || !hitsEl) {
            return;
        }

        var searchClient = createSearchClient(config);
        var search = window.instantsearch({
            indexName: config.indexName,
            searchClient: searchClient,
        });

        var widgets = [
            window.instantsearch.widgets.searchBox({
                container: '#flapjack-searchbox',
                placeholder: 'Search...',
                showSubmit: false,
                showReset: true,
                cssClasses: {
                    root: 'flapjack-searchbox-root',
                },
            }),

            window.instantsearch.widgets.hits({
                container: '#flapjack-hits',
                templates: {
                    item: function (hit, bindEvent) {
                        var html =
                            '<article class="flapjack-hit">' +
                            '<h3 class="flapjack-hit-title">' +
                            '<a href="' + escapeHtml(hit.permalink || '#') + '">' +
                            window.instantsearch.highlight({
                                attribute: 'post_title',
                                hit: hit,
                            }) +
                            '</a>' +
                            '</h3>' +
                            '<p class="flapjack-hit-excerpt">' +
                            window.instantsearch.snippet({
                                attribute: 'post_content',
                                hit: hit,
                            }) +
                            '</p>';

                        // WooCommerce product-specific fields.
                        if (hit.post_type === 'product' && hit.price !== undefined) {
                            var wc = config.woocommerce || {};
                            var sym = wc.currencySymbol || '$';
                            html += '<div class="flapjack-hit-product-meta">';
                            html += '<span class="flapjack-hit-price">' + sym + parseFloat(hit.price).toFixed(2) + '</span>';
                            if (hit.on_sale) {
                                html += '<span class="flapjack-hit-badge flapjack-hit-sale">Sale</span>';
                            }
                            if (hit.average_rating > 0) {
                                html += '<span class="flapjack-hit-rating">' + renderStars(hit.average_rating) + '</span>';
                            }
                            if (!hit.in_stock) {
                                html += '<span class="flapjack-hit-badge flapjack-hit-out-of-stock">Out of Stock</span>';
                            }
                            html += '</div>';
                        }

                        html += '<span class="flapjack-hit-type">' +
                            escapeHtml(hit.post_type_label || hit.post_type || '') +
                            '</span>' +
                            '</article>';

                        return html;
                    },
                    empty: function () {
                        return '<div class="flapjack-no-results">No results found.</div>';
                    },
                },
            }),

            window.instantsearch.widgets.pagination({
                container: '#flapjack-pagination',
            }),
        ];

        // Add stats widget if container exists.
        if (document.getElementById('flapjack-stats')) {
            widgets.push(window.instantsearch.widgets.stats({
                container: '#flapjack-stats',
            }));
        }

        // Add WooCommerce facet widgets.
        if (config.woocommerce && config.woocommerce.enabled) {
            addWooCommerceFacets(widgets, config.woocommerce);
        }

        search.addWidgets(widgets);
        search.start();
    }

    /**
     * Initialize autocomplete on existing WordPress search forms.
     */
    function initAutocomplete(config) {
        var searchForms = document.querySelectorAll('form[role="search"], .search-form');

        searchForms.forEach(function (form) {
            var input = form.querySelector('input[name="s"], input[type="search"]');
            if (!input) return;

            var dropdown = createDropdown();
            input.parentNode.style.position = 'relative';
            input.parentNode.appendChild(dropdown);

            var debounceTimer;
            input.addEventListener('input', function () {
                clearTimeout(debounceTimer);
                var query = input.value.trim();

                if (query.length < 2) {
                    dropdown.style.display = 'none';
                    return;
                }

                debounceTimer = setTimeout(function () {
                    fetchResults(config, query, function (hits) {
                        renderDropdown(dropdown, hits);
                    });
                }, 200);
            });

            // Close on blur.
            input.addEventListener('blur', function () {
                setTimeout(function () {
                    dropdown.style.display = 'none';
                }, 200);
            });

            // Reopen on focus.
            input.addEventListener('focus', function () {
                if (dropdown.children.length > 0 && input.value.trim().length >= 2) {
                    dropdown.style.display = 'block';
                }
            });
        });
    }

    /**
     * Fetch search results from the REST API.
     */
    function fetchResults(config, query, callback) {
        var url =
            config.restUrl +
            'search?q=' +
            encodeURIComponent(query) +
            '&per_page=5';

        fetch(url, {
            headers: {
                'X-WP-Nonce': config.nonce,
            },
        })
            .then(function (response) {
                return response.json();
            })
            .then(function (data) {
                callback(data.hits || []);
            })
            .catch(function () {
                callback([]);
            });
    }

    /**
     * Create the autocomplete dropdown element.
     */
    function createDropdown() {
        var el = document.createElement('div');
        el.className = 'flapjack-autocomplete-dropdown';
        el.style.display = 'none';
        return el;
    }

    /**
     * Render hits into the dropdown.
     */
    function renderDropdown(dropdown, hits) {
        if (hits.length === 0) {
            dropdown.style.display = 'none';
            return;
        }

        dropdown.innerHTML = hits
            .map(function (hit) {
                return (
                    '<a class="flapjack-autocomplete-item" href="' +
                    escapeHtml(hit.permalink || '#') +
                    '">' +
                    '<strong>' +
                    escapeHtml(hit.post_title || '') +
                    '</strong>' +
                    '<span>' +
                    escapeHtml(hit.post_type_label || '') +
                    '</span>' +
                    '</a>'
                );
            })
            .join('');

        dropdown.style.display = 'block';
    }

    /**
     * Create a search client for InstantSearch.
     */
    function createSearchClient(config) {
        return {
            search: function (requests) {
                return fetch(config.restUrl + 'search?' + buildSearchParams(requests[0].params), {
                    headers: { 'X-WP-Nonce': config.nonce },
                })
                    .then(function (response) {
                        return response.json();
                    })
                    .then(function (data) {
                        return { results: [data] };
                    });
            },
        };
    }

    /**
     * Build query string from InstantSearch params.
     * Forwards facet/filter params so refinements work server-side.
     */
    function buildSearchParams(params) {
        var parts = ['q=' + encodeURIComponent(params.query || '')];
        if (params.hitsPerPage) parts.push('per_page=' + params.hitsPerPage);
        if (params.page) parts.push('page=' + params.page);
        if (params.facets) parts.push('facets=' + encodeURIComponent(JSON.stringify(params.facets)));
        if (params.facetFilters) parts.push('facetFilters=' + encodeURIComponent(JSON.stringify(params.facetFilters)));
        if (params.numericFilters) parts.push('numericFilters=' + encodeURIComponent(JSON.stringify(params.numericFilters)));
        if (params.tagFilters) parts.push('tagFilters=' + encodeURIComponent(JSON.stringify(params.tagFilters)));
        if (params.filters) parts.push('filters=' + encodeURIComponent(params.filters));
        return parts.join('&');
    }

    /**
     * Add WooCommerce facet widgets to the InstantSearch instance.
     */
    function addWooCommerceFacets(widgets, wcConfig) {
        var is = window.instantsearch.widgets;

        // Category refinement list.
        if (document.getElementById('flapjack-facet-categories')) {
            widgets.push(is.refinementList({
                container: '#flapjack-facet-categories',
                attribute: 'taxonomy_product_cat',
                limit: 10,
                showMore: true,
                showMoreLimit: 20,
                searchable: true,
                sortBy: ['count:desc', 'name:asc'],
                cssClasses: { root: 'flapjack-facet-panel' },
                templates: {
                    header: function () { return '<h4 class="flapjack-facet-title">Categories</h4>'; },
                },
            }));
        }

        // Price range slider.
        if (document.getElementById('flapjack-facet-price') && is.rangeSlider) {
            widgets.push(is.rangeSlider({
                container: '#flapjack-facet-price',
                attribute: 'price',
                precision: 0,
                cssClasses: { root: 'flapjack-facet-panel' },
                templates: {
                    header: function () { return '<h4 class="flapjack-facet-title">Price</h4>'; },
                },
            }));
        } else if (document.getElementById('flapjack-facet-price') && is.rangeInput) {
            // Fallback to range input if slider widget not available.
            widgets.push(is.rangeInput({
                container: '#flapjack-facet-price',
                attribute: 'price',
                precision: 0,
                cssClasses: { root: 'flapjack-facet-panel' },
                templates: {
                    header: function () { return '<h4 class="flapjack-facet-title">Price</h4>'; },
                },
            }));
        }

        // In Stock toggle.
        if (document.getElementById('flapjack-facet-stock') && is.toggleRefinement) {
            widgets.push(is.toggleRefinement({
                container: '#flapjack-facet-stock',
                attribute: 'in_stock',
                on: 1,
                cssClasses: { root: 'flapjack-facet-panel' },
                templates: {
                    labelText: function () { return 'In Stock Only'; },
                },
            }));
        }

        // On Sale toggle.
        if (document.getElementById('flapjack-facet-sale') && is.toggleRefinement) {
            widgets.push(is.toggleRefinement({
                container: '#flapjack-facet-sale',
                attribute: 'on_sale',
                on: 1,
                cssClasses: { root: 'flapjack-facet-panel' },
                templates: {
                    labelText: function () { return 'On Sale'; },
                },
            }));
        }

        // Rating filter.
        if (document.getElementById('flapjack-facet-rating') && is.ratingMenu) {
            widgets.push(is.ratingMenu({
                container: '#flapjack-facet-rating',
                attribute: 'average_rating',
                cssClasses: { root: 'flapjack-facet-panel' },
            }));
        }

        // Clear refinements button.
        if (document.getElementById('flapjack-clear-refinements') && is.clearRefinements) {
            widgets.push(is.clearRefinements({
                container: '#flapjack-clear-refinements',
                cssClasses: { root: 'flapjack-facet-panel' },
                templates: {
                    resetLabel: function () { return 'Clear all filters'; },
                },
            }));
        }
    }

    /**
     * Render star rating as HTML.
     */
    function renderStars(rating) {
        var full = Math.floor(rating);
        var half = rating - full >= 0.5 ? 1 : 0;
        var empty = 5 - full - half;
        var html = '';
        for (var i = 0; i < full; i++) html += '&#9733;';
        for (var j = 0; j < half; j++) html += '&#9734;';
        for (var k = 0; k < empty; k++) html += '&#9734;';
        return html;
    }

    /**
     * Escape HTML to prevent XSS.
     */
    function escapeHtml(str) {
        var div = document.createElement('div');
        div.appendChild(document.createTextNode(str));
        return div.innerHTML;
    }
})();
