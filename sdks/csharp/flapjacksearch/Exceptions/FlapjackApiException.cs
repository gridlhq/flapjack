using System;

namespace Flapjack.Search.Exceptions;

/// <summary>
/// Exception sent by Flapjack's API
/// </summary>
public class FlapjackApiException : Exception
{
  /// <summary>
  /// Http error code
  /// </summary>
  public int HttpErrorCode { get; set; }

  /// <summary>
  /// Create a new FlapjackAPIException
  /// </summary>
  /// <param name="message"></param>
  /// <param name="httpErrorCode"></param>
  public FlapjackApiException(string message, int httpErrorCode)
    : base(message)
  {
    HttpErrorCode = httpErrorCode;
  }
}
