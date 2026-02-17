package com.flapjackhq.client.transport.internal

import com.flapjackhq.client.BuildConfig
import com.flapjackhq.client.configuration.AgentSegment
import com.flapjackhq.client.configuration.ClientOptions
import com.flapjackhq.client.configuration.Host
import com.flapjackhq.client.configuration.internal.FlapjackAgent
import com.flapjackhq.client.configuration.internal.algoliaHttpClient
import com.flapjackhq.client.configuration.internal.platformAgentSegment
import com.flapjackhq.client.transport.RequestConfig
import com.flapjackhq.client.transport.RequestOptions
import com.flapjackhq.client.transport.Requester
import io.ktor.util.reflect.*
import kotlin.time.Duration

/**
 * Executes a network request with the specified configuration and options, then returns the result
 * as the specified type.
 *
 * This is a suspending function, which means it can be used with coroutines for asynchronous
 * execution.
 *
 * @param T The type of the result expected from the request. This should match the returnType
 *   parameter.
 * @param requestConfig The configuration for the network request, including the URL, method,
 *   headers, and body.
 * @param requestOptions Optional settings for the request execution, such as timeouts or cache
 *   policies. Default value is null.
 */
internal suspend inline fun <reified T> Requester.execute(
  requestConfig: RequestConfig,
  requestOptions: RequestOptions? = null,
): T = execute(requestConfig, requestOptions, typeInfo<T>())

/** Creates a [Requester] instance. */
internal fun requesterOf(
  clientName: String,
  appId: String,
  apiKey: String,
  connectTimeout: Duration,
  readTimeout: Duration,
  writeTimeout: Duration,
  options: ClientOptions,
  defaultHosts: () -> List<Host>,
) =
  options.requester
    ?: KtorRequester(
      httpClient =
        algoliaHttpClient(
          appId = appId,
          apiKey = apiKey,
          options = options,
          agent =
            FlapjackAgent(BuildConfig.VERSION).apply {
              add(platformAgentSegment())
              add(AgentSegment(clientName, BuildConfig.VERSION))
            },
        ),
      connectTimeout = options.connectTimeout ?: connectTimeout,
      readTimeout = options.readTimeout ?: readTimeout,
      writeTimeout = options.writeTimeout ?: writeTimeout,
      hosts = options.hosts ?: defaultHosts(),
    )
