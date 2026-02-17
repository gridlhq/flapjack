package com.flapjackhq.config;

import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;
import javax.annotation.Nonnull;

public final class FlapjackAgent {

  private final Set<String> segments;

  private String finalValue;

  public FlapjackAgent(String clientVersion) {
    this.finalValue = String.format("Flapjack for Java (%s)", clientVersion);
    this.segments = new LinkedHashSet<>();
    this.addSegment(new Segment("JVM", System.getProperty("java.version")));
  }

  public FlapjackAgent addSegment(@Nonnull Segment seg) {
    String segment = seg.toString();
    if (!segments.contains(segment)) {
      segments.add(segment);
      finalValue += segment;
    }
    return this;
  }

  public FlapjackAgent addSegments(@Nonnull List<Segment> segments) {
    for (Segment segment : segments) {
      addSegment(segment);
    }
    return this;
  }

  public FlapjackAgent removeSegment(@Nonnull Segment seg) {
    segments.remove(seg.toString());
    return this;
  }

  @Override
  public String toString() {
    return finalValue;
  }

  public static class Segment {

    private final String value;
    private final String version;

    public Segment(String value) {
      this(value, null);
    }

    public Segment(String value, String version) {
      this.value = value;
      this.version = version;
    }

    @Override
    public String toString() {
      StringBuilder sb = new StringBuilder();
      sb.append("; ").append(value);
      if (version != null) {
        sb.append(" (").append(version).append(")");
      }
      return sb.toString();
    }
  }
}
