package com.flapjackhq.exceptions;

/** Exception thrown in case of API failure such as 4XX, 5XX error. */
public class FlapjackResponseException extends FlapjackRuntimeException {

  private static final long serialVersionUID = 1L;

  public int getHttpErrorCode() {
    return httpErrorCode;
  }

  private final int httpErrorCode;

  public FlapjackResponseException(String message, Throwable cause, int httpErrorCode) {
    super(message, cause);
    this.httpErrorCode = httpErrorCode;
  }

  public FlapjackResponseException(String message, int httpErrorCode) {
    super(message);
    this.httpErrorCode = httpErrorCode;
  }

  public FlapjackResponseException(Throwable cause, int httpErrorCode) {
    super(cause);
    this.httpErrorCode = httpErrorCode;
  }
}
