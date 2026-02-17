package com.flapjackhq.exceptions;

/**
 * Exception thrown when an error occurs during the waitForTask strategy. For example: maximum
 * number of retry exceeded
 */
public class FlapjackRetriesExceededException extends FlapjackRuntimeException {

  private static final long serialVersionUID = 1L;

  public FlapjackRetriesExceededException(String message, Throwable cause) {
    super(message, cause);
  }

  public FlapjackRetriesExceededException(String message) {
    super(message);
  }

  public FlapjackRetriesExceededException(Throwable cause) {
    super(cause);
  }
}
