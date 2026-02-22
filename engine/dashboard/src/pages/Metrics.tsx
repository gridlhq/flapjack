import { memo, useMemo, useState } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Skeleton } from '@/components/ui/skeleton';
import { usePrometheusMetrics, getPerIndexMetrics, getSystemMetric } from '@/hooks/useMetrics';
import { useHealthDetail } from '@/hooks/useSystemStatus';
import {
  BarChart3,
  Search,
  PenLine,
  BookOpen,
  ArrowDownToLine,
  FileText,
  HardDrive,
  Database,
  XCircle,
  ArrowUpDown,
} from 'lucide-react';
import { formatBytes, formatUptime } from '@/lib/utils';

type SortKey = 'name' | 'documents_count' | 'storage_bytes' | 'search_requests_total' | 'write_operations_total' | 'read_requests_total' | 'bytes_in_total' | 'oplog_current_seq';
type SortDir = 'asc' | 'desc';

function OverviewTab() {
  const { data: metrics, isLoading: metricsLoading, isError: metricsError } = usePrometheusMetrics();
  const { data: health } = useHealthDetail();

  if (metricsLoading) {
    return (
      <div className="space-y-4">
        <Skeleton className="h-8 w-48" />
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
          {Array.from({ length: 7 }).map((_, i) => (
            <Card key={i}><CardContent className="pt-6"><Skeleton className="h-16" /></CardContent></Card>
          ))}
        </div>
      </div>
    );
  }

  if (metricsError || !metrics) {
    return (
      <Card>
        <CardContent className="pt-6">
          <div className="flex items-center gap-3 text-destructive">
            <XCircle className="h-5 w-5" />
            <div>
              <p className="font-medium">Failed to fetch metrics</p>
              <p className="text-sm text-muted-foreground">The /metrics endpoint may be unreachable.</p>
            </div>
          </div>
        </CardContent>
      </Card>
    );
  }

  // Aggregate per-index counters
  const perIndex = getPerIndexMetrics(metrics);
  let totalSearches = 0;
  let totalWrites = 0;
  let totalReads = 0;
  let totalBytesIn = 0;
  let totalDocs = 0;
  let totalStorage = 0;

  for (const record of perIndex.values()) {
    totalSearches += record.search_requests_total ?? 0;
    totalWrites += record.write_operations_total ?? 0;
    totalReads += record.read_requests_total ?? 0;
    totalBytesIn += record.bytes_in_total ?? 0;
    totalDocs += record.documents_count ?? 0;
    totalStorage += record.storage_bytes ?? 0;
  }

  const tenantsLoaded = getSystemMetric(metrics, 'flapjack_tenants_loaded') ?? 0;

  const requestCards = [
    { label: 'Total Searches', value: totalSearches.toLocaleString(), icon: Search, testId: 'metrics-total-searches', color: 'text-blue-600 dark:text-blue-400' },
    { label: 'Total Writes', value: totalWrites.toLocaleString(), icon: PenLine, testId: 'metrics-total-writes', color: 'text-green-600 dark:text-green-400' },
    { label: 'Total Reads', value: totalReads.toLocaleString(), icon: BookOpen, testId: 'metrics-total-reads', color: 'text-purple-600 dark:text-purple-400' },
    { label: 'Total Bytes In', value: formatBytes(totalBytesIn), icon: ArrowDownToLine, testId: 'metrics-total-bytes-in', color: 'text-orange-600 dark:text-orange-400' },
  ];

  const storageCards = [
    { label: 'Total Documents', value: totalDocs.toLocaleString(), icon: FileText, testId: 'metrics-total-docs', color: 'text-cyan-600 dark:text-cyan-400' },
    { label: 'Total Storage', value: formatBytes(totalStorage), icon: HardDrive, testId: 'metrics-total-storage', color: 'text-rose-600 dark:text-rose-400' },
    { label: 'Loaded Tenants', value: String(tenantsLoaded), icon: Database, testId: 'metrics-tenants', color: 'text-indigo-600 dark:text-indigo-400' },
  ];

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-3">
        <p className="text-sm text-muted-foreground">Auto-refreshes every 10 seconds</p>
        {health?.version && (
          <span
            className="inline-flex items-center rounded-full bg-muted px-2.5 py-0.5 text-xs font-medium"
            data-testid="metrics-version"
          >
            {health.version}
          </span>
        )}
        {health?.uptime_secs != null && (
          <span className="text-sm text-muted-foreground" data-testid="metrics-uptime">
            Uptime: {formatUptime(health.uptime_secs)}
          </span>
        )}
      </div>

      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        {requestCards.map((card) => (
          <Card key={card.testId} data-testid={card.testId}>
            <CardHeader className="flex flex-row items-center justify-between pb-2">
              <CardTitle className="text-sm font-medium text-muted-foreground">{card.label}</CardTitle>
              <card.icon className={`h-5 w-5 ${card.color}`} />
            </CardHeader>
            <CardContent>
              <p className="text-2xl font-bold" data-testid="stat-value">{card.value}</p>
            </CardContent>
          </Card>
        ))}
      </div>

      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {storageCards.map((card) => (
          <Card key={card.testId} data-testid={card.testId}>
            <CardHeader className="flex flex-row items-center justify-between pb-2">
              <CardTitle className="text-sm font-medium text-muted-foreground">{card.label}</CardTitle>
              <card.icon className={`h-5 w-5 ${card.color}`} />
            </CardHeader>
            <CardContent>
              <p className="text-2xl font-bold" data-testid="stat-value">{card.value}</p>
            </CardContent>
          </Card>
        ))}
      </div>
    </div>
  );
}

function PerIndexTab() {
  const { data: metrics, isLoading, isError } = usePrometheusMetrics();
  const [sortKey, setSortKey] = useState<SortKey>('name');
  const [sortDir, setSortDir] = useState<SortDir>('asc');

  const rows = useMemo(() => {
    if (!metrics) return [];
    const perIndex = getPerIndexMetrics(metrics);
    const entries = Array.from(perIndex.entries()).map(([name, m]) => ({
      name,
      documents_count: m.documents_count ?? 0,
      storage_bytes: m.storage_bytes ?? 0,
      search_requests_total: m.search_requests_total ?? 0,
      write_operations_total: m.write_operations_total ?? 0,
      read_requests_total: m.read_requests_total ?? 0,
      bytes_in_total: m.bytes_in_total ?? 0,
      oplog_current_seq: m.oplog_current_seq ?? 0,
    }));

    entries.sort((a, b) => {
      const aVal = a[sortKey];
      const bVal = b[sortKey];
      if (typeof aVal === 'string' && typeof bVal === 'string') {
        return sortDir === 'asc' ? aVal.localeCompare(bVal) : bVal.localeCompare(aVal);
      }
      return sortDir === 'asc'
        ? (aVal as number) - (bVal as number)
        : (bVal as number) - (aVal as number);
    });

    return entries;
  }, [metrics, sortKey, sortDir]);

  const toggleSort = (key: SortKey) => {
    if (sortKey === key) {
      setSortDir((d) => (d === 'asc' ? 'desc' : 'asc'));
    } else {
      setSortKey(key);
      setSortDir('asc');
    }
  };

  if (isLoading) {
    return (
      <div className="space-y-2">
        {Array.from({ length: 5 }).map((_, i) => (
          <Skeleton key={i} className="h-12 w-full" />
        ))}
      </div>
    );
  }

  if (isError) {
    return (
      <Card>
        <CardContent className="pt-6">
          <div className="flex items-center gap-3 text-destructive">
            <XCircle className="h-5 w-5" />
            <p className="font-medium">Failed to load per-index metrics.</p>
          </div>
        </CardContent>
      </Card>
    );
  }

  if (rows.length === 0) {
    return (
      <Card>
        <CardContent className="pt-6 text-center text-muted-foreground">
          No indexes found. Create an index to see per-index metrics.
        </CardContent>
      </Card>
    );
  }

  const columns: { key: SortKey; label: string; align: 'left' | 'right' }[] = [
    { key: 'name', label: 'Index Name', align: 'left' },
    { key: 'documents_count', label: 'Documents', align: 'right' },
    { key: 'storage_bytes', label: 'Storage', align: 'right' },
    { key: 'search_requests_total', label: 'Searches', align: 'right' },
    { key: 'write_operations_total', label: 'Writes', align: 'right' },
    { key: 'read_requests_total', label: 'Reads', align: 'right' },
    { key: 'bytes_in_total', label: 'Bytes In', align: 'right' },
    { key: 'oplog_current_seq', label: 'Oplog Seq', align: 'right' },
  ];

  return (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground">Auto-refreshes every 10 seconds</p>
      <Card>
        <CardContent className="pt-6">
          <div className="overflow-x-auto">
            <table className="w-full text-sm" data-testid="metrics-per-index-table">
              <thead>
                <tr className="border-b text-left text-muted-foreground">
                  {columns.map((col) => (
                    <th
                      key={col.key}
                      className={`pb-2 pr-4 font-medium cursor-pointer select-none hover:text-foreground transition-colors ${col.align === 'right' ? 'text-right' : ''}`}
                      onClick={() => toggleSort(col.key)}
                      aria-sort={sortKey === col.key ? (sortDir === 'asc' ? 'ascending' : 'descending') : 'none'}
                    >
                      <span className="inline-flex items-center gap-1">
                        {col.label}
                        <ArrowUpDown className={`h-3 w-3 ${sortKey === col.key ? 'opacity-100' : 'opacity-30'}`} />
                      </span>
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {rows.map((row) => (
                  <tr
                    key={row.name}
                    className="border-b last:border-0"
                    data-testid={`metrics-index-row-${row.name}`}
                  >
                    <td className="py-2 pr-4 font-medium" data-testid={`metrics-cell-${row.name}-name`}>{row.name}</td>
                    <td className="py-2 pr-4 text-right" data-testid={`metrics-cell-${row.name}-docs`}>{row.documents_count.toLocaleString()}</td>
                    <td className="py-2 pr-4 text-right" data-testid={`metrics-cell-${row.name}-storage`}>{formatBytes(row.storage_bytes)}</td>
                    <td className="py-2 pr-4 text-right" data-testid={`metrics-cell-${row.name}-searches`}>{row.search_requests_total.toLocaleString()}</td>
                    <td className="py-2 pr-4 text-right" data-testid={`metrics-cell-${row.name}-writes`}>{row.write_operations_total.toLocaleString()}</td>
                    <td className="py-2 pr-4 text-right" data-testid={`metrics-cell-${row.name}-reads`}>{row.read_requests_total.toLocaleString()}</td>
                    <td className="py-2 pr-4 text-right" data-testid={`metrics-cell-${row.name}-bytes-in`}>{formatBytes(row.bytes_in_total)}</td>
                    <td className="py-2 text-right" data-testid={`metrics-cell-${row.name}-oplog`}>{row.oplog_current_seq.toLocaleString()}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

export const Metrics = memo(function Metrics() {
  return (
    <div className="space-y-6">
      <div className="flex items-center gap-3">
        <BarChart3 className="h-6 w-6" />
        <h1 className="text-2xl font-bold">Metrics</h1>
      </div>

      <Tabs defaultValue="overview">
        <TabsList>
          <TabsTrigger value="overview">Overview</TabsTrigger>
          <TabsTrigger value="per-index">Per-Index</TabsTrigger>
        </TabsList>

        <TabsContent value="overview">
          <OverviewTab />
        </TabsContent>

        <TabsContent value="per-index">
          <PerIndexTab />
        </TabsContent>
      </Tabs>
    </div>
  );
});
