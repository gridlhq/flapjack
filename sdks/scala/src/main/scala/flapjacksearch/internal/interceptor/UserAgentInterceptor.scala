package flapjacksearch.internal.interceptor

import flapjacksearch.internal.FlapjackAgent
import okhttp3.{Interceptor, Request, Response}

/** Interceptor that adds the user agent to the request headers.
  *
  * @param agent
  *   user agent
  */
private[flapjacksearch] class UserAgentInterceptor(agent: FlapjackAgent) extends Interceptor {

  override def intercept(chain: Interceptor.Chain): Response = {
    val originalRequest: Request = chain.request()
    val newRequest: Request = originalRequest
      .newBuilder()
      .header("user-agent", agent.toString)
      .build()

    chain.proceed(newRequest)
  }
}
