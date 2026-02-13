<?php

namespace Flapjack\FlapjackSearch\Support;

use Flapjack\FlapjackSearch\Flapjack;
use GuzzleHttp\ClientInterface;

final class FlapjackAgent
{
    private static $value;

    private static $customSegments = [];

    public static function get($clientName)
    {
        if (!isset(self::$value[$clientName])) {
            self::$value[$clientName] = self::getComputedValue($clientName);
        }

        return self::$value[$clientName];
    }

    public static function addFlapjackAgent($clientName, $segment, $version)
    {
        self::$value[$clientName] = null;
        self::$customSegments[trim($segment, ' ')] = trim($version, ' ');
    }

    private static function getComputedValue($clientName)
    {
        $ua = [];
        $segments = array_merge(
            self::getDefaultSegments($clientName),
            self::$customSegments
        );

        foreach ($segments as $segment => $version) {
            $ua[] = $segment.' ('.$version.')';
        }

        return implode('; ', $ua);
    }

    private static function getDefaultSegments($clientName)
    {
        $segments = [];

        $segments['Flapjack for PHP'] = Flapjack::VERSION;
        $segments[$clientName] = Flapjack::VERSION;
        $segments['PHP'] = rtrim(
            str_replace(PHP_EXTRA_VERSION, '', PHP_VERSION),
            '-'
        );
        if (defined('HHVM_VERSION')) {
            $segments['HHVM'] = HHVM_VERSION;
        }
        if (interface_exists('\GuzzleHttp\ClientInterface')) {
            if (defined('\GuzzleHttp\ClientInterface::VERSION')) {
                $segments['Guzzle'] = ClientInterface::VERSION;
            } else {
                $segments['Guzzle']
                    = ClientInterface::MAJOR_VERSION;
            }
        }

        return $segments;
    }
}
