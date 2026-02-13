import 'dart:core';

import 'package:flapjack_client_core/src/config/agent_segment.dart';

/// Handles Flapjack agent segments.
///
/// An instance of this class maintains a set of [AgentSegment]s, and provides
/// methods to add, remove, and format these segments.
final class FlapjackAgent {
  final Set<AgentSegment> _segments = {};

  /// Constructs an [FlapjackAgent] with the provided [clientVersion].
  FlapjackAgent(String clientVersion) {
    _segments.add(
      AgentSegment(value: "Flapjack for Dart", version: clientVersion),
    );
  }

  /// Adds a new [segment] to the agent segments.
  bool add(AgentSegment segment) => _segments.add(segment);

  /// Adds all [segments] to the agent segments.
  void addAll(Iterable<AgentSegment> segments) => _segments.addAll(segments);

  /// Removes [segment] from the agent segments.
  bool remove(AgentSegment segment) => _segments.remove(segment);

  /// Formats the agent segments into a semicolon-separated string.
  String formatted() => _segments.map((it) => it.formatted()).join("; ");

  @override
  String toString() {
    return 'FlapjackAgent{segments: $_segments}';
  }
}

/// Provides a formatted string representation for [AgentSegment].
extension on AgentSegment {
  String formatted() {
    StringBuffer sb = StringBuffer();
    sb.write(value);
    if (version != null) {
      sb.write(" (");
      sb.write(version);
      sb.write(")");
    }
    return sb.toString();
  }
}
