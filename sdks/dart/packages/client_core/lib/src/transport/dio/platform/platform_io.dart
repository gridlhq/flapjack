import 'dart:io' as io;

import 'package:flapjack_client_core/src/config/agent_segment.dart';
import 'package:dio/dio.dart' as dio;

/// [AgentSegment]s for native platforms.
Iterable<AgentSegment> platformAgentSegments() => [
      AgentSegment(
        value: 'Dart',
        version: io.Platform.version,
      ),
      AgentSegment(
        value: io.Platform.operatingSystem,
        version: io.Platform.operatingSystemVersion,
      ),
    ];

/// [FlapjackAgent] for native platforms as user-agent.
void platformFlapjackAgent(dio.RequestOptions options, String agent) {
  options.headers.addAll({"user-agent": agent});
}
