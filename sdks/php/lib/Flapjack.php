<?php

namespace Flapjack\FlapjackSearch;

use Flapjack\FlapjackSearch\Cache\NullCacheDriver;
use Flapjack\FlapjackSearch\Http\CurlHttpClient;
use Flapjack\FlapjackSearch\Http\GuzzleHttpClient;
use Flapjack\FlapjackSearch\Http\HttpClientInterface;
use Flapjack\FlapjackSearch\Log\DebugLogger;
use GuzzleHttp\Client;
use GuzzleHttp\ClientInterface;
use Psr\Log\LoggerInterface;
use Psr\SimpleCache\CacheInterface;

final class Flapjack
{
    public const VERSION = '0.1.0-beta.1';

    /**
     * Holds an instance of the simple cache repository (PSR-16).
     *
     * @var null|CacheInterface
     */
    private static $cache;

    /**
     * Holds an instance of the logger (PSR-3).
     *
     * @var null|LoggerInterface
     */
    private static $logger;

    /**
     * @var HttpClientInterface
     */
    private static $httpClient;

    public static function isCacheEnabled()
    {
        if (null === self::$cache) {
            return false;
        }

        return !self::getCache() instanceof NullCacheDriver;
    }

    /**
     * Gets the cache instance.
     *
     * @return CacheInterface
     */
    public static function getCache()
    {
        if (null === self::$cache) {
            self::setCache(new NullCacheDriver());
        }

        return self::$cache;
    }

    /**
     * Sets the cache instance.
     */
    public static function setCache(CacheInterface $cache)
    {
        self::$cache = $cache;
    }

    /**
     * Gets the logger instance.
     *
     * @return LoggerInterface
     */
    public static function getLogger()
    {
        if (null === self::$logger) {
            self::setLogger(new DebugLogger());
        }

        return self::$logger;
    }

    /**
     * Sets the logger instance.
     */
    public static function setLogger(LoggerInterface $logger)
    {
        self::$logger = $logger;
    }

    public static function getHttpClient()
    {
        $guzzleVersion = null;
        if (interface_exists('\GuzzleHttp\ClientInterface')) {
            if (defined('\GuzzleHttp\ClientInterface::VERSION')) {
                $guzzleVersion = (int) mb_substr(
                    Client::VERSION,
                    0,
                    1
                );
            } else {
                $guzzleVersion = ClientInterface::MAJOR_VERSION;
            }
        }

        if (null === self::$httpClient) {
            if (class_exists('\GuzzleHttp\Client') && 6 <= $guzzleVersion) {
                self::setHttpClient(
                    new GuzzleHttpClient()
                );
            } else {
                self::setHttpClient(
                    new CurlHttpClient()
                );
            }
        }

        return self::$httpClient;
    }

    public static function setHttpClient(HttpClientInterface $httpClient)
    {
        self::$httpClient = $httpClient;
    }

    public static function resetHttpClient()
    {
        self::$httpClient = null;
    }
}
