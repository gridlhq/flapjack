using System;
using System.Collections.Generic;
using Flapjack.Search.Http;
using Flapjack.Search.Models.Common;
using Flapjack.Search.Serializer;
using Flapjack.Search.Transport;
using Flapjack.Search.Utils;

namespace Flapjack.Search.Clients
{
  /// <summary>
  /// Flapjack's client configuration
  /// </summary>
  public abstract class FlapjackConfig
  {
    /// <summary>
    /// Create a new Flapjack's configuration for the given credentials
    /// </summary>
    /// <param name="appId">Your application ID</param>
    /// <param name="apiKey">Your API Key</param>
    /// <param name="clientName">The client name</param>
    /// <param name="clientVersion">The client version</param>
    protected FlapjackConfig(string appId, string apiKey, string clientName, string clientVersion)
    {
      AppId = appId;
      ApiKey = apiKey;
      UserAgent = new FlapjackUserAgent(clientName, clientVersion);
      DefaultHeaders = new Dictionary<string, string>
      {
        { Defaults.FlapjackApplicationHeader.ToLowerInvariant(), AppId },
        { Defaults.FlapjackApiKeyHeader.ToLowerInvariant(), ApiKey },
        { Defaults.UserAgentHeader.ToLowerInvariant(), "" },
        { Defaults.Connection.ToLowerInvariant(), Defaults.KeepAlive },
        { Defaults.AcceptHeader.ToLowerInvariant(), JsonConfig.JsonContentType },
      };
    }

    /// <summary>
    /// The application ID
    /// </summary>
    /// <returns></returns>
    public string AppId { get; }

    /// <summary>
    /// The admin API Key
    /// </summary>
    /// <returns></returns>
    public string ApiKey { get; set; }

    /// <summary>
    /// Configurations hosts
    /// </summary>
    public List<StatefulHost> CustomHosts { get; set; }

    /// <summary>
    /// Flapjack's default headers.
    /// Will be sent for every request
    /// </summary>
    public Dictionary<string, string> DefaultHeaders { get; set; }

    /// <summary>
    /// Set the read timeout for all requests
    /// </summary>
    public TimeSpan? ReadTimeout { get; set; }

    /// <summary>
    /// Set the read timeout for all requests
    /// </summary>
    public TimeSpan? WriteTimeout { get; set; }

    /// <summary>
    /// Set the connect timeout for all requests
    /// </summary>
    public TimeSpan? ConnectTimeout { get; set; }

    /// <summary>
    /// Compression for outgoing http requests  <see cref="CompressionType"/>
    /// </summary>
    public CompressionType Compression { get; set; }

    /// <summary>
    /// Configurations hosts
    /// </summary>
    protected internal List<StatefulHost> DefaultHosts { get; set; }

    /// <summary>
    /// The user-agent header
    /// </summary>
    public FlapjackUserAgent UserAgent { get; }

    /// <summary>
    /// Build the headers for the request
    /// </summary>
    /// <returns></returns>
    internal Dictionary<string, string> BuildHeaders()
    {
      DefaultHeaders[Defaults.UserAgentHeader.ToLowerInvariant()] = UserAgent.ToString();
      return DefaultHeaders;
    }

    /// <summary>
    /// Helper to switch the API key sent with each request
    /// </summary>
    /// <param name="apiKey">Your API Key</param>
    /// <returns></returns>
    public void SetClientApiKey(string apiKey)
    {
      ApiKey = apiKey;
      DefaultHeaders[Defaults.FlapjackApiKeyHeader.ToLowerInvariant()] = apiKey;
    }
  }
}
