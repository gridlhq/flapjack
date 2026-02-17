import 'package:flapjack_client_core/src/config/agent_segment.dart';
import 'package:flapjack_client_core/src/transport/flapjack_agent.dart';
import 'package:dio/dio.dart' as dio;

/// [AgentSegment]s for unsupported platforms.
Iterable<AgentSegment> platformAgentSegments() => const [];

/// [FlapjackAgent] for unsupported platforms.
void platformFlapjackAgent(dio.RequestOptions options, String agent) {
  // NO-OP.
}
