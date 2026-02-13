using System;

namespace Flapjack.Search.Exceptions;

/// <summary>
/// Exception thrown when an error occurs in the Flapjack client.
/// </summary>
public class FlapjackException : Exception
{
  /// <summary>
  /// Create a new Flapjack exception.
  /// </summary>
  /// <param name="message">The exception details.</param>
  public FlapjackException(string message)
    : base(message) { }

  /// <summary>
  /// Create a new Flapjack exception, with an inner exception.
  /// </summary>
  /// <param name="message"></param>
  /// <param name="inner"></param>
  public FlapjackException(string message, Exception inner)
    : base(message, inner) { }
}
