export interface PrometheusMetric {
  name: string;
  labels: Record<string, string>;
  value: number;
}

/**
 * Parse Prometheus text exposition format into structured metrics.
 * Handles: `metric_name value`, `metric_name{label="val"} value`,
 * HELP/TYPE comments (skipped), blank lines (skipped), malformed lines (skipped).
 */
export function parsePrometheusText(text: string): PrometheusMetric[] {
  const metrics: PrometheusMetric[] = [];

  for (const line of text.split('\n')) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith('#')) continue;

    const metric = parseLine(trimmed);
    if (metric) metrics.push(metric);
  }

  return metrics;
}

function parseLine(line: string): PrometheusMetric | null {
  // Format: metric_name{label="val",label2="val2"} value
  // or:     metric_name value
  const braceIdx = line.indexOf('{');

  if (braceIdx === -1) {
    // No labels: "metric_name value"
    const spaceIdx = line.lastIndexOf(' ');
    if (spaceIdx === -1) return null;

    const name = line.substring(0, spaceIdx);
    const value = Number(line.substring(spaceIdx + 1));
    if (!name || isNaN(value)) return null;

    return { name, labels: {}, value };
  }

  // Has labels: "metric_name{...} value"
  const closeBrace = line.indexOf('}');
  if (closeBrace === -1) return null;

  const name = line.substring(0, braceIdx);
  const labelsStr = line.substring(braceIdx + 1, closeBrace);
  const valueStr = line.substring(closeBrace + 1).trim();
  const value = Number(valueStr);

  if (!name || isNaN(value)) return null;

  const labels: Record<string, string> = {};
  if (labelsStr) {
    // Parse label="value" pairs, handling commas inside values
    const labelRegex = /(\w+)="([^"]*)"/g;
    let match;
    while ((match = labelRegex.exec(labelsStr)) !== null) {
      labels[match[1]] = match[2];
    }
  }

  return { name, labels, value };
}
