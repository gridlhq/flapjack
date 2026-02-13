package flapjacksearch.internal

import flapjacksearch.config.AgentSegment

import scala.collection.mutable

/** Handles Flapjack agent segments.
  *
  * An instance of this class maintains a set of [AgentSegment]s, and provides methods to add, remove, and format these
  * segments.
  *
  * @param clientVersion
  *   client version
  */
class FlapjackAgent(clientVersion: String) {
  private val segs = mutable.LinkedHashSet[AgentSegment](
    AgentSegment("Flapjack for Scala", clientVersion),
    AgentSegment("JVM", System.getProperty("java.version"))
  )

  /** Adds a new segment to the agent segments. */
  def addSegment(seg: AgentSegment): FlapjackAgent = {
    if (!segs.contains(seg)) {
      segs += seg
    }
    this
  }

  /** Adds all segments to the agent segments */
  def addSegments(segments: Seq[AgentSegment]): FlapjackAgent = {
    segs.addAll(segments)
    this
  }

  override def toString: String = {
    segs.mkString("; ")
  }
}

object FlapjackAgent {

  /** Creates a new FlapjackAgent instance with the given client version. */
  def apply(clientVersion: String): FlapjackAgent =
    new FlapjackAgent(clientVersion)
}
