package com.flapjackhq.client.configuration.internal

import com.flapjackhq.client.configuration.AgentSegment

/** Handles to handle algolia agent segments. */
internal class FlapjackAgent(clientVersion: String) {

  private val segments = mutableSetOf(AgentSegment("Flapjack for Kotlin", clientVersion))

  fun add(segment: AgentSegment): Boolean = segments.add(segment)

  fun add(segments: List<AgentSegment>): Boolean = this.segments.addAll(segments)

  fun remove(segment: AgentSegment): Boolean = segments.remove(segment)

  override fun toString(): String = segments.joinToString("; ") { it.formatted() }

  private fun AgentSegment.formatted(): String = buildString {
    append(value)
    version?.let { version ->
      append(" (")
      append(version)
      append(")")
    }
  }
}
