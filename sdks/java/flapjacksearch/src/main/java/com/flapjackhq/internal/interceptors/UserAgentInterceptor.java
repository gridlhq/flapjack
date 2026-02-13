package com.flapjackhq.internal.interceptors;

import com.flapjackhq.config.FlapjackAgent;
import java.io.IOException;
import javax.annotation.Nonnull;
import okhttp3.Interceptor;
import okhttp3.Response;

public final class UserAgentInterceptor implements Interceptor {

  private final FlapjackAgent agent;

  public UserAgentInterceptor(FlapjackAgent agent) {
    this.agent = agent;
  }

  @Nonnull
  @Override
  public Response intercept(Chain chain) throws IOException {
    okhttp3.Request originalRequest = chain.request();
    okhttp3.Request.Builder builder = originalRequest.newBuilder();
    builder.header("user-agent", agent.toString());
    okhttp3.Request newRequest = builder.build();
    return chain.proceed(newRequest);
  }
}
