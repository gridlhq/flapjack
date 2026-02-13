<?php

namespace Flapjack\FlapjackSearch\RetryStrategy;

interface ApiWrapperInterface
{
    public function sendRequest(
        $method,
        $path,
        $data = [],
        $requestOptions = [],
        $useReadTransporter = false
    );

    public function send($method, $path, $requestOptions = [], $hosts = null);
}
