package com.flapjackhq.client.exception.internal

import com.flapjackhq.client.exception.AlgoliaApiException
import com.flapjackhq.client.exception.AlgoliaClientException
import com.flapjackhq.client.exception.AlgoliaRuntimeException
import io.ktor.client.plugins.*

/** Coerce a Throwable to a [AlgoliaClientException]. */
internal fun Throwable.asClientException(): AlgoliaClientException =
  AlgoliaClientException(message = message, cause = this)

/** Coerce a [ResponseException] to a [AlgoliaRuntimeException]. */
internal fun ResponseException.asApiException(): AlgoliaApiException =
  AlgoliaApiException(message = message, cause = this, httpErrorCode = response.status.value)
