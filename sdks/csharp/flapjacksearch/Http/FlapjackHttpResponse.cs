using System.IO;

namespace Flapjack.Search.Http;

/// <summary>
/// Response from Flapjack's API
/// </summary>
public class FlapjackHttpResponse
{
  /// <summary>
  /// Http response code
  /// </summary>
  public int HttpStatusCode { get; set; }

  /// <summary>
  /// Stream Response body
  /// </summary>
  public Stream Body { get; set; }

  /// <summary>
  /// TimeOut
  /// </summary>
  public bool IsTimedOut { get; set; }

  /// <summary>
  /// Network connectivity, DNS failure, server certificate validation.
  /// </summary>
  public bool IsNetworkError { get; set; }

  /// <summary>
  /// Http Error message
  /// </summary>
  public string Error { get; set; }
}
