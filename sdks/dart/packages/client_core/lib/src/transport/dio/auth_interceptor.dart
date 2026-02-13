import 'package:dio/dio.dart';

/// Interceptor that attaches the application id and API key to outgoing requests.
///
/// This interceptor modifies the headers of each request to include the
/// application id and API key for Flapjack authentication.
class AuthInterceptor extends Interceptor {
  /// The application id used for Flapjack authentication.
  final String appId;

  /// The API key used for Flapjack authentication.
  String apiKey;

  /// Constructs an [AuthInterceptor] with the provided application id and API key.
  AuthInterceptor({
    required this.appId,
    required this.apiKey,
  });

  @override
  void onRequest(RequestOptions options, RequestInterceptorHandler handler) {
    options.headers.putIfAbsent("x-algolia-application-id", () => appId);
    options.headers.putIfAbsent("x-algolia-api-key", () => apiKey);
    super.onRequest(options, handler);
  }
}
