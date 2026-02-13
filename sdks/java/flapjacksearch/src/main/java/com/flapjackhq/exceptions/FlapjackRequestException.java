package com.flapjackhq.exceptions;

/** Represents a retryable exception (4XX). */
public final class FlapjackRequestException extends FlapjackApiException {

  public FlapjackRequestException(String message, Throwable cause, int httpErrorCode) {
    super(message, cause, httpErrorCode);
  }

  public FlapjackRequestException(String message, int httpErrorCode) {
    super(message, httpErrorCode);
  }

  public FlapjackRequestException(Throwable cause, int httpErrorCode) {
    super(cause, httpErrorCode);
  }
}
