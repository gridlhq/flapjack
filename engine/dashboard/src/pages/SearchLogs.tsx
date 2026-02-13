import { useState, useMemo } from 'react';
import { Search, Download, Trash2, Clock, CheckCircle2, XCircle, Loader2 } from 'lucide-react';
import { Card } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import { useApiLogger, type ApiLogEntry } from '@/hooks/useApiLogger';

function formatDuration(ms: number): string {
  if (ms < 1) return '<1ms';
  if (ms < 1000) return `${Math.round(ms)}ms`;
  return `${(ms / 1000).toFixed(2)}s`;
}

function getMethodColor(method: string): string {
  switch (method) {
    case 'GET': return 'text-blue-600 dark:text-blue-400';
    case 'POST': return 'text-green-600 dark:text-green-400';
    case 'PUT': return 'text-orange-600 dark:text-orange-400';
    case 'DELETE': return 'text-red-600 dark:text-red-400';
    default: return 'text-muted-foreground';
  }
}

function StatusIcon({ status }: { status: ApiLogEntry['status'] }) {
  switch (status) {
    case 'pending':
      return <Loader2 className="h-4 w-4 text-muted-foreground animate-spin" />;
    case 'success':
      return <CheckCircle2 className="h-4 w-4 text-green-500" />;
    case 'error':
      return <XCircle className="h-4 w-4 text-destructive" />;
  }
}

function extractSearchQuery(entry: ApiLogEntry): string | null {
  if (entry.url.includes('/query') && entry.body?.query !== undefined) {
    return entry.body.query;
  }
  return null;
}

function extractHitCount(entry: ApiLogEntry): number | null {
  if (entry.response?.nbHits !== undefined) {
    return entry.response.nbHits;
  }
  return null;
}

export function SearchLogs() {
  const { entries, clear, exportAsFile } = useApiLogger();
  const [filter, setFilter] = useState('');
  const [expandedId, setExpandedId] = useState<string | null>(null);

  const filtered = useMemo(() => {
    if (!filter) return entries;
    const lower = filter.toLowerCase();
    return entries.filter(
      (e) =>
        e.url.toLowerCase().includes(lower) ||
        e.method.toLowerCase().includes(lower) ||
        JSON.stringify(e.body || '').toLowerCase().includes(lower)
    );
  }, [entries, filter]);

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold">API Logs</h2>
          <p className="text-sm text-muted-foreground mt-1">
            Recent API calls with query, response time, and hit count
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Badge variant="secondary">{entries.length} requests</Badge>
          <Button variant="outline" size="sm" onClick={exportAsFile}>
            <Download className="h-4 w-4 mr-1" />
            Export
          </Button>
          <Button variant="outline" size="sm" onClick={clear}>
            <Trash2 className="h-4 w-4 mr-1" />
            Clear
          </Button>
        </div>
      </div>

      {/* Filter */}
      <div className="relative">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
        <Input
          placeholder="Filter by URL, method, or body..."
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          className="pl-9"
        />
      </div>

      {/* Log entries */}
      {filtered.length === 0 ? (
        <Card className="p-8 text-center">
          <h3 className="text-lg font-semibold mb-2">No API logs</h3>
          <p className="text-sm text-muted-foreground">
            API calls will appear here as you use the dashboard.
            Search, browse, and manage your indices to generate logs.
          </p>
        </Card>
      ) : (
        <div className="space-y-1" data-testid="logs-list">
          {/* Table header */}
          <div className="grid grid-cols-[80px_40px_1fr_100px_80px_80px] gap-2 px-4 py-2 text-xs font-medium text-muted-foreground border-b">
            <span>Time</span>
            <span></span>
            <span>Request</span>
            <span>Query</span>
            <span className="text-right">Hits</span>
            <span className="text-right">Duration</span>
          </div>
          {filtered.map((entry) => {
            const query = extractSearchQuery(entry);
            const hits = extractHitCount(entry);
            const isExpanded = expandedId === entry.id;
            return (
              <div key={entry.id}>
                <button
                  type="button"
                  className="w-full grid grid-cols-[80px_40px_1fr_100px_80px_80px] gap-2 px-4 py-2.5 text-sm hover:bg-accent/50 rounded-md transition-colors text-left"
                  onClick={() => setExpandedId(isExpanded ? null : entry.id)}
                >
                  <span className="text-xs text-muted-foreground flex items-center gap-1">
                    <Clock className="h-3 w-3" />
                    {new Date(entry.timestamp).toLocaleTimeString()}
                  </span>
                  <StatusIcon status={entry.status} />
                  <span className="flex items-center gap-2 min-w-0">
                    <span className={`font-mono font-bold text-xs ${getMethodColor(entry.method)}`}>
                      {entry.method}
                    </span>
                    <span className="font-mono truncate text-xs">{entry.url}</span>
                  </span>
                  <span className="text-xs text-muted-foreground truncate">
                    {query !== null ? `"${query}"` : ''}
                  </span>
                  <span className="text-right text-xs font-mono">
                    {hits !== null ? hits.toLocaleString() : ''}
                  </span>
                  <span className="text-right text-xs font-mono">
                    {entry.duration > 0 ? formatDuration(entry.duration) : ''}
                  </span>
                </button>

                {/* Expanded detail */}
                {isExpanded && (
                  <Card className="mx-4 mb-2 p-4">
                    <div className="grid grid-cols-2 gap-4">
                      {entry.body && (
                        <div>
                          <h4 className="text-xs font-semibold text-muted-foreground mb-1">Request Body</h4>
                          <pre className="text-xs font-mono bg-muted p-2 rounded-md overflow-auto max-h-48">
                            {JSON.stringify(entry.body, null, 2)}
                          </pre>
                        </div>
                      )}
                      {entry.response && (
                        <div>
                          <h4 className="text-xs font-semibold text-muted-foreground mb-1">Response</h4>
                          <pre className="text-xs font-mono bg-muted p-2 rounded-md overflow-auto max-h-48">
                            {JSON.stringify(entry.response, null, 2)}
                          </pre>
                        </div>
                      )}
                    </div>
                  </Card>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
