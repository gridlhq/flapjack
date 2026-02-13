<?php

namespace Flapjack\FlapjackSearch\Exceptions;

final class UnreachableException extends AlgoliaException
{
    public function __construct($message = '', $code = 0, $previous = null)
    {
        if (!$message) {
            $message
                = 'Impossible to connect, please check your Flapjack Application Id. If the error persists, please visit our help center https://alg.li/support-unreachable-hosts or reach out to the Flapjack Support team: https://alg.li/support';
        }

        parent::__construct($message, $code, $previous);
    }
}
