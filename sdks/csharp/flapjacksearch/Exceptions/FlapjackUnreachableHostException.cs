using System;

namespace Flapjack.Search.Exceptions;

/// <summary>
/// Exception thrown when an host in unreachable
/// </summary>
public class FlapjackUnreachableHostException : Exception
{
  /// <summary>
  /// Create a new FlapjackUnreachableHostException.
  /// </summary>
  /// <param name="message">The exception details.</param>
  public FlapjackUnreachableHostException(string message)
    : base(message) { }
}
