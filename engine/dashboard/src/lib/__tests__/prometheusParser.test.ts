import { describe, it, expect } from 'vitest';
import { parsePrometheusText } from '../prometheusParser';

describe('parsePrometheusText', () => {
  it('returns empty array for empty input', () => {
    expect(parsePrometheusText('')).toEqual([]);
  });

  it('parses gauge line without labels', () => {
    const result = parsePrometheusText('flapjack_active_writers 3');
    expect(result).toEqual([
      { name: 'flapjack_active_writers', labels: {}, value: 3 },
    ]);
  });

  it('parses gauge with single label', () => {
    const result = parsePrometheusText('flapjack_storage_bytes{index="foo"} 1024');
    expect(result).toEqual([
      { name: 'flapjack_storage_bytes', labels: { index: 'foo' }, value: 1024 },
    ]);
  });

  it('parses gauge with multiple labels', () => {
    const result = parsePrometheusText('flapjack_peer_status{index="foo",peer_id="bar"} 1');
    expect(result).toEqual([
      { name: 'flapjack_peer_status', labels: { index: 'foo', peer_id: 'bar' }, value: 1 },
    ]);
  });

  it('skips HELP comment lines', () => {
    const input = '# HELP flapjack_active_writers Number of active writers\nflapjack_active_writers 5';
    const result = parsePrometheusText(input);
    expect(result).toEqual([
      { name: 'flapjack_active_writers', labels: {}, value: 5 },
    ]);
  });

  it('skips TYPE comment lines', () => {
    const input = '# TYPE flapjack_active_writers gauge\nflapjack_active_writers 5';
    const result = parsePrometheusText(input);
    expect(result).toEqual([
      { name: 'flapjack_active_writers', labels: {}, value: 5 },
    ]);
  });

  it('skips blank lines', () => {
    const input = 'flapjack_a 1\n\n\nflapjack_b 2';
    const result = parsePrometheusText(input);
    expect(result).toHaveLength(2);
    expect(result[0].name).toBe('flapjack_a');
    expect(result[1].name).toBe('flapjack_b');
  });

  it('skips malformed lines gracefully', () => {
    const input = 'flapjack_a 1\nthis is garbage\nflapjack_b 2';
    const result = parsePrometheusText(input);
    expect(result).toHaveLength(2);
  });

  it('parses real sample from /metrics endpoint', () => {
    const sample = `# HELP flapjack_active_writers Number of active index writers
# TYPE flapjack_active_writers gauge
flapjack_active_writers 0
# HELP flapjack_max_concurrent_writers Maximum concurrent writers allowed
# TYPE flapjack_max_concurrent_writers gauge
flapjack_max_concurrent_writers 4
# HELP flapjack_memory_heap_bytes Heap allocated bytes
# TYPE flapjack_memory_heap_bytes gauge
flapjack_memory_heap_bytes 536870912
# HELP flapjack_storage_bytes Per-tenant disk storage in bytes
# TYPE flapjack_storage_bytes gauge
flapjack_storage_bytes{index="products"} 204800
flapjack_storage_bytes{index="users"} 102400
# HELP flapjack_search_requests_total Total search requests per index
# TYPE flapjack_search_requests_total gauge
flapjack_search_requests_total{index="products"} 42
flapjack_search_requests_total{index="users"} 10
# HELP flapjack_documents_count Number of documents per tenant index
# TYPE flapjack_documents_count gauge
flapjack_documents_count{index="products"} 100
flapjack_documents_count{index="users"} 50
# HELP flapjack_tenants_loaded Number of loaded tenant indexes
# TYPE flapjack_tenants_loaded gauge
flapjack_tenants_loaded 2`;

    const result = parsePrometheusText(sample);

    // System-wide gauges
    const activeWriters = result.find((m) => m.name === 'flapjack_active_writers');
    expect(activeWriters).toEqual({ name: 'flapjack_active_writers', labels: {}, value: 0 });

    const maxWriters = result.find((m) => m.name === 'flapjack_max_concurrent_writers');
    expect(maxWriters?.value).toBe(4);

    // Per-index gauges
    const productsStorage = result.find(
      (m) => m.name === 'flapjack_storage_bytes' && m.labels.index === 'products'
    );
    expect(productsStorage?.value).toBe(204800);

    const productsSearches = result.find(
      (m) => m.name === 'flapjack_search_requests_total' && m.labels.index === 'products'
    );
    expect(productsSearches?.value).toBe(42);

    const tenantsLoaded = result.find((m) => m.name === 'flapjack_tenants_loaded');
    expect(tenantsLoaded?.value).toBe(2);

    // active_writers + max_writers + heap + 2×storage + 2×search + 2×docs + tenants = 10
    expect(result.length).toBe(10);
  });

  it('handles floating point values', () => {
    const result = parsePrometheusText('flapjack_memory_heap_bytes 1.5e+09');
    expect(result[0].value).toBe(1.5e9);
  });
});
