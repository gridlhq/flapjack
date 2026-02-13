package com.flapjackhq.exceptions;

/** Exception thrown when an error occurs during the Serialization/Deserialization process */
public class FlapjackRuntimeException extends RuntimeException {

  private static final long serialVersionUID = 1L;

  public FlapjackRuntimeException(String message, Throwable cause) {
    super(message, cause);
  }

  public FlapjackRuntimeException(String message) {
    super(message);
  }

  public FlapjackRuntimeException(Throwable cause) {
    super(cause);
  }
}
