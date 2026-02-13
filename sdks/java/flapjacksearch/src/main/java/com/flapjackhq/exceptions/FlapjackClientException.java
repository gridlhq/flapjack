package com.flapjackhq.exceptions;

/** Exception thrown when an error occurs during API requests. */
public class FlapjackClientException extends FlapjackRuntimeException {

  public FlapjackClientException(String message, Throwable cause) {
    super(message, cause);
  }

  public FlapjackClientException(String message) {
    super(message);
  }

  public FlapjackClientException(Throwable cause) {
    super(cause);
  }
}
