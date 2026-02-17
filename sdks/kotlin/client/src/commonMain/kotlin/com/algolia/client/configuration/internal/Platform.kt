package com.flapjackhq.client.configuration.internal

import com.flapjackhq.client.configuration.AgentSegment
import com.flapjackhq.client.configuration.ClientOptions
import io.ktor.client.*

/** Get platform specific algolia agent segment. */
internal expect fun platformAgentSegment(): AgentSegment

/** Platform specific http client configuration */
internal expect fun HttpClientConfig<*>.platformConfig(options: ClientOptions)
