package com.flapjackhq.exceptions;

/** Exception thrown in case of API failure such as 4XX, 5XX error. */
public class FlapjackApiException extends FlapjackRuntimeException {

  private static final long serialVersionUID = 1L;

  public int getStatusCode() {
    return statusCode;
  }

  private final int statusCode;

  public FlapjackApiException(String message, Throwable cause, int httpErrorCode) {
    super(message, cause);
    this.statusCode = httpErrorCode;
  }

  public FlapjackApiException(String message, int httpErrorCode) {
    super(message);
    this.statusCode = httpErrorCode;
  }

  public FlapjackApiException(Throwable cause, int httpErrorCode) {
    super(cause);
    this.statusCode = httpErrorCode;
  }

  @Override
  public String getMessage() {
    String message = super.getMessage();
    return "Status Code: " + getStatusCode() + (message != null ? " - " + message : "");
  }
}
