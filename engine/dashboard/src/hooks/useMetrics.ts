import { useQuery } from '@tanstack/react-query';
import { parsePrometheusText, type PrometheusMetric } from '@/lib/prometheusParser';

export function usePrometheusMetrics() {
  return useQuery<PrometheusMetric[]>({
    queryKey: ['prometheus-metrics'],
    queryFn: async () => {
      // Fetch directly from the backend — /metrics can't go through the Vite proxy
      // because the dashboard page route is also /metrics (SPA path conflict).
      const res = await fetch(`${__BACKEND_URL__}/metrics`);
      if (!res.ok) throw new Error(`Metrics fetch failed: ${res.status}`);
      const text = await res.text();
      return parsePrometheusText(text);
    },
    refetchInterval: 10000,
    staleTime: 5000,
  });
}

/**
 * Group metrics by index label into a map of index name → metric short names → values.
 * Strips the `flapjack_` prefix for readability.
 */
export function getPerIndexMetrics(
  metrics: PrometheusMetric[]
): Map<string, Record<string, number>> {
  const result = new Map<string, Record<string, number>>();

  for (const m of metrics) {
    const indexName = m.labels.index;
    if (!indexName) continue;

    if (!result.has(indexName)) {
      result.set(indexName, {});
    }
    const shortName = m.name.replace(/^flapjack_/, '');
    result.get(indexName)![shortName] = m.value;
  }

  return result;
}

/**
 * Get a single system-wide metric value by name.
 * Returns undefined if not found.
 */
export function getSystemMetric(
  metrics: PrometheusMetric[],
  name: string
): number | undefined {
  return metrics.find((m) => m.name === name && Object.keys(m.labels).length === 0)?.value;
}
