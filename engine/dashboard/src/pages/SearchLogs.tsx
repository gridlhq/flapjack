import { useState, useMemo, useCallback } from 'react';
import { Search, Download, Trash2, Clock, CheckCircle2, XCircle, Loader2, Copy, Check, Terminal, List } from 'lucide-react';
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

function entryToCurl(entry: ApiLogEntry): string {
  const fullUrl = entry.url.startsWith('http') ? entry.url : `${__BACKEND_URL__}${entry.url}`;
  const headers = Object.entries(entry.headers || {})
    .filter(([k]) => k !== 'x-request-id')
    .map(([k, v]) => `  -H "${k}: ${v}"`)
    .join(' \\\n');
  const body = entry.body ? ` \\\n  -d '${JSON.stringify(entry.body)}'` : '';
  return `curl -X ${entry.method} "${fullUrl}" \\\n${headers}${body}`;
}

function entryToEndpoint(entry: ApiLogEntry): string {
  return `${entry.method} ${entry.url}`;
}

function CopyButton({ text, title }: { text: string; title?: string }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = useCallback(async (e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Fallback for non-secure contexts
      const ta = document.createElement('textarea');
      ta.value = text;
      document.body.appendChild(ta);
      ta.select();
      document.execCommand('copy');
      document.body.removeChild(ta);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  }, [text]);

  return (
    <button
      type="button"
      onClick={handleCopy}
      title={title || 'Copy to clipboard'}
      className="p-1 rounded hover:bg-accent transition-colors"
    >
      {copied ? (
        <Check className="h-3.5 w-3.5 text-green-500" />
      ) : (
        <Copy className="h-3.5 w-3.5 text-muted-foreground" />
      )}
    </button>
  );
}

export function SearchLogs() {
  const { entries, clear, exportAsFile } = useApiLogger();
  const [filter, setFilter] = useState('');
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [viewMode, setViewMode] = useState<'endpoint' | 'curl'>('endpoint');

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

          {/* View mode toggle */}
          <div className="flex rounded-md border border-input">
            <button
              onClick={() => setViewMode('endpoint')}
              className={`px-2.5 py-1.5 text-xs font-medium transition-colors rounded-l-md flex items-center gap-1 ${
                viewMode === 'endpoint'
                  ? 'bg-primary text-primary-foreground'
                  : 'text-muted-foreground hover:bg-accent'
              }`}
              title="Show endpoint format"
            >
              <List className="h-3 w-3" />
              Endpoint
            </button>
            <button
              onClick={() => setViewMode('curl')}
              className={`px-2.5 py-1.5 text-xs font-medium transition-colors rounded-r-md flex items-center gap-1 ${
                viewMode === 'curl'
                  ? 'bg-primary text-primary-foreground'
                  : 'text-muted-foreground hover:bg-accent'
              }`}
              title="Show curl commands"
            >
              <Terminal className="h-3 w-3" />
              Curl
            </button>
          </div>

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
            Search, browse, and manage your indexes to generate logs.
          </p>
        </Card>
      ) : viewMode === 'curl' ? (
        /* Curl view mode */
        <div className="space-y-2" data-testid="logs-list">
          {filtered.map((entry) => {
            const curl = entryToCurl(entry);
            return (
              <Card key={entry.id} className="p-3">
                <div className="flex items-start justify-between gap-2">
                  <div className="flex items-center gap-2 shrink-0">
                    <StatusIcon status={entry.status} />
                    <span className="text-xs text-muted-foreground">
                      {new Date(entry.timestamp).toLocaleTimeString()}
                    </span>
                    {entry.duration > 0 && (
                      <Badge variant="secondary" className="text-xs">
                        {formatDuration(entry.duration)}
                      </Badge>
                    )}
                  </div>
                  <CopyButton text={curl} title="Copy curl command" />
                </div>
                <pre className="text-xs font-mono mt-2 whitespace-pre-wrap break-all bg-muted p-2 rounded-md">
                  {curl}
                </pre>
              </Card>
            );
          })}
        </div>
      ) : (
        /* Endpoint view mode */
        <div className="space-y-1" data-testid="logs-list">
          {/* Table header */}
          <div className="grid grid-cols-[80px_40px_1fr_100px_80px_80px_32px] gap-2 px-4 py-2 text-xs font-medium text-muted-foreground border-b">
            <span>Time</span>
            <span></span>
            <span>Request</span>
            <span>Query</span>
            <span className="text-right">Hits</span>
            <span className="text-right">Duration</span>
            <span></span>
          </div>
          {filtered.map((entry) => {
            const query = extractSearchQuery(entry);
            const hits = extractHitCount(entry);
            const isExpanded = expandedId === entry.id;
            const copyText = entryToEndpoint(entry);
            return (
              <div key={entry.id}>
                <div className="grid grid-cols-[80px_40px_1fr_100px_80px_80px_32px] gap-2 px-4 py-2.5 text-sm hover:bg-accent/50 rounded-md transition-colors">
                  <button
                    type="button"
                    className="contents text-left"
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
                  <CopyButton text={copyText} title="Copy endpoint" />
                </div>

                {/* Expanded detail */}
                {isExpanded && (
                  <Card className="mx-4 mb-2 p-4">
                    {/* Summary line */}
                    <div className="flex items-center gap-3 mb-3 text-xs text-muted-foreground">
                      <span>Status: <span className="font-medium text-foreground">{entry.status}</span></span>
                      {entry.duration > 0 && (
                        <span>Duration: <span className="font-medium text-foreground">{formatDuration(entry.duration)}</span></span>
                      )}
                      {entry.response?.processingTimeMs !== undefined && (
                        <span>Server: <span className="font-medium text-foreground">{entry.response.processingTimeMs}ms</span></span>
                      )}
                    </div>

                    {/* Curl command */}
                    <div className="mb-3">
                      <div className="flex items-center justify-between mb-1">
                        <h4 className="text-xs font-semibold text-muted-foreground">Curl Command</h4>
                        <CopyButton text={entryToCurl(entry)} title="Copy curl command" />
                      </div>
                      <pre className="text-xs font-mono bg-muted p-2 rounded-md overflow-auto max-h-32 whitespace-pre-wrap break-all">
                        {entryToCurl(entry)}
                      </pre>
                    </div>

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
                      {!entry.body && !entry.response && (
                        <div className="col-span-2 text-xs text-muted-foreground py-4 text-center">
                          {entry.status === 'pending' ? 'Waiting for response...' : 'No request/response data available'}
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
