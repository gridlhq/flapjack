package com.flapjackhq.utils;

import com.flapjackhq.exceptions.*;
import java.util.function.IntUnaryOperator;
import java.util.function.Predicate;
import java.util.function.Supplier;

public class TaskUtils {

  private TaskUtils() {
    // Empty.
  }

  public static final int DEFAULT_MAX_RETRIES = 50;
  public static final IntUnaryOperator DEFAULT_TIMEOUT = (int retries) -> Math.min(retries * 200, 5000);

  public static <T> T retryUntil(Supplier<T> func, Predicate<T> validate, int maxRetries, IntUnaryOperator timeout)
    throws FlapjackRuntimeException {
    if (timeout == null) {
      timeout = DEFAULT_TIMEOUT;
    }
    if (maxRetries == 0) {
      maxRetries = DEFAULT_MAX_RETRIES;
    }
    int retryCount = 0;
    while (retryCount < maxRetries) {
      T resp = func.get();
      if (validate.test(resp)) {
        return resp;
      }
      try {
        Thread.sleep(timeout.applyAsInt(retryCount));
      } catch (InterruptedException ignored) {
        // Restore interrupted state...
        Thread.currentThread().interrupt();
      }

      retryCount++;
    }
    throw new FlapjackRetriesExceededException("The maximum number of retries exceeded. (" + (retryCount + 1) + "/" + maxRetries + ")");
  }
}
