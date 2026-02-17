/**
 * Flapjack Search — Admin settings page JavaScript.
 *
 * Handles Test Connection, Reindex, and Background Reindex AJAX buttons.
 */
jQuery( function( $ ) {
    'use strict';

    var config = window.flapjackAdminConfig || {};
    var i18n   = config.i18n || {};
    var progressTimer = null;

    // Test Connection button.
    $( '#flapjack-test-connection' ).on( 'click', function() {
        var $btn    = $( this );
        var $result = $( '#flapjack-test-result' );

        $btn.prop( 'disabled', true );
        $result.text( i18n.testing || 'Testing...' );

        $.post( window.ajaxurl, {
            action:   'flapjack_test_connection',
            _wpnonce: config.testNonce || ''
        }, function( response ) {
            $result.text( response.data.message )
                   .css( 'color', response.success ? 'green' : 'red' );
            $btn.prop( 'disabled', false );
        }).fail( function() {
            $result.text( 'Request failed.' ).css( 'color', 'red' );
            $btn.prop( 'disabled', false );
        });
    });

    // Reindex button (synchronous).
    $( '#flapjack-reindex' ).on( 'click', function() {
        var $btn    = $( this );
        var $result = $( '#flapjack-reindex-result' );

        $btn.prop( 'disabled', true );
        $result.text( i18n.reindexing || 'Reindexing...' );

        $.post( window.ajaxurl, {
            action:   'flapjack_reindex',
            _wpnonce: config.reindexNonce || ''
        }, function( response ) {
            $result.text( response.data.message )
                   .css( 'color', response.success ? 'green' : 'red' );
            $btn.prop( 'disabled', false );
        }).fail( function() {
            $result.text( 'Request failed.' ).css( 'color', 'red' );
            $btn.prop( 'disabled', false );
        });
    });

    // Background Reindex button.
    $( '#flapjack-reindex-background' ).on( 'click', function() {
        var $btn    = $( this );
        var $cancel = $( '#flapjack-reindex-cancel' );
        var $result = $( '#flapjack-reindex-bg-result' );

        $btn.prop( 'disabled', true );
        $result.text( i18n.starting || 'Starting background reindex...' );

        $.post( window.ajaxurl, {
            action:   'flapjack_reindex_background',
            _wpnonce: config.reindexBgNonce || ''
        }, function( response ) {
            if ( response.success ) {
                $cancel.show();
                $result.text( '' );
                showProgress( response.data );
                startProgressPolling();
            } else {
                $result.text( response.data.message ).css( 'color', 'red' );
                $btn.prop( 'disabled', false );
            }
        }).fail( function() {
            $result.text( 'Request failed.' ).css( 'color', 'red' );
            $btn.prop( 'disabled', false );
        });
    });

    // Cancel button.
    $( '#flapjack-reindex-cancel' ).on( 'click', function() {
        var $btn = $( this );
        $btn.prop( 'disabled', true );

        $.post( window.ajaxurl, {
            action:   'flapjack_reindex_cancel',
            _wpnonce: config.reindexCancelNonce || ''
        }, function() {
            stopProgressPolling();
            $btn.hide().prop( 'disabled', false );
            $( '#flapjack-reindex-background' ).prop( 'disabled', false );
            $( '#flapjack-reindex-bg-result' )
                .text( i18n.cancelled || 'Cancelled.' )
                .css( 'color', '#666' );
            $( '#flapjack-reindex-progress' ).hide();
        }).fail( function() {
            $btn.prop( 'disabled', false );
        });
    });

    function showProgress( data ) {
        var $wrap = $( '#flapjack-reindex-progress' );
        var $fill = $wrap.find( '.flapjack-progress-fill' );
        var $text = $wrap.find( '.flapjack-progress-text' );
        var total = data.total_posts || 0;
        var done  = data.processed || 0;
        var pct   = total > 0 ? Math.round( ( done / total ) * 100 ) : 0;

        $wrap.show();
        $fill.css( 'width', pct + '%' );

        if ( i18n.progressFmt ) {
            $text.text( i18n.progressFmt.replace( '%1$d', done ).replace( '%2$d', total ).replace( '%3$d', pct ) );
        } else {
            $text.text( done + ' / ' + total + ' (' + pct + '%)' );
        }
    }

    function startProgressPolling() {
        stopProgressPolling();
        progressTimer = setInterval( function() {
            $.post( window.ajaxurl, {
                action:   'flapjack_reindex_progress',
                _wpnonce: config.reindexProgressNonce || ''
            }, function( response ) {
                if ( ! response.success ) {
                    stopProgressPolling();
                    return;
                }

                var data = response.data;
                showProgress( data );

                if ( data.status === 'complete' ) {
                    stopProgressPolling();
                    $( '#flapjack-reindex-cancel' ).hide();
                    $( '#flapjack-reindex-background' ).prop( 'disabled', false );
                    $( '#flapjack-reindex-bg-result' )
                        .text( ( i18n.complete || 'Complete!' ) + ' ' + data.processed + ' posts indexed.' )
                        .css( 'color', 'green' );
                } else if ( data.status === 'failed' ) {
                    stopProgressPolling();
                    $( '#flapjack-reindex-cancel' ).hide();
                    $( '#flapjack-reindex-background' ).prop( 'disabled', false );
                    $( '#flapjack-reindex-bg-result' )
                        .text( ( i18n.failed || 'Failed.' ) + ' ' + ( data.error || '' ) )
                        .css( 'color', 'red' );
                } else if ( data.status === 'cancelled' ) {
                    stopProgressPolling();
                    $( '#flapjack-reindex-cancel' ).hide();
                    $( '#flapjack-reindex-background' ).prop( 'disabled', false );
                    $( '#flapjack-reindex-bg-result' )
                        .text( i18n.cancelled || 'Cancelled.' )
                        .css( 'color', '#666' );
                }
            });
        }, 2000 );
    }

    function stopProgressPolling() {
        if ( progressTimer ) {
            clearInterval( progressTimer );
            progressTimer = null;
        }
    }
});
