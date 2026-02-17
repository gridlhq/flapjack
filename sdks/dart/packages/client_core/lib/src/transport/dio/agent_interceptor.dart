import 'package:flapjack_client_core/src/transport/flapjack_agent.dart';
import 'package:flapjack_client_core/src/transport/dio/platform/platform.dart';
import 'package:dio/dio.dart';

/// Interceptor that attaches the Flapjack agent to outgoing requests.
///
/// This interceptor modifies the query parameters of each request to include the
/// formatted representation of the Flapjack agent.
class AgentInterceptor extends Interceptor {
  /// The Flapjack agent to be attached to outgoing requests.
  final FlapjackAgent agent;

  /// Constructs an [AgentInterceptor] with the provided Flapjack agent.
  AgentInterceptor({required this.agent});

  @override
  void onRequest(RequestOptions options, RequestInterceptorHandler handler) {
    Platform.flapjackAgent(options, agent.formatted());
    super.onRequest(options, handler);
  }
}
