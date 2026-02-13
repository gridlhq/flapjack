package com.flapjackhq.client.configuration.internal

import com.flapjackhq.client.configuration.AgentSegment
import com.flapjackhq.client.configuration.ClientOptions
import com.flapjackhq.client.configuration.CompressionType
import io.ktor.client.*

internal actual fun platformAgentSegment(): AgentSegment =
  AgentSegment("JVM", System.getProperty("java.version"))

internal actual fun HttpClientConfig<*>.platformConfig(options: ClientOptions) {
  if (options.compressionType == CompressionType.GZIP) {
    install(GzipCompression)
  }
}
