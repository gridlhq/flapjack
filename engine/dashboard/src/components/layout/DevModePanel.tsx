import { useState } from 'react';
import { useDevMode, DevLogEntry } from '@/hooks/useDevMode';
import { Button } from '@/components/ui/button';
import { Copy, Trash2, ChevronDown, ChevronUp, Check } from 'lucide-react';

function formatTime(ts: number) {
  const d = new Date(ts);
  return d.toLocaleTimeString('en-US', { hour12: false }) + '.' + String(d.getMilliseconds()).padStart(3, '0');
}

const categoryColors: Record<string, string> = {
  facets: 'text-blue-600 dark:text-blue-400',
  search: 'text-green-600 dark:text-green-400',
  api: 'text-yellow-600 dark:text-yellow-400',
  error: 'text-red-600 dark:text-red-400',
};

function LogLine({ entry }: { entry: DevLogEntry }) {
  const [expanded, setExpanded] = useState(false);
  const colorClass = categoryColors[entry.category] || 'text-muted-foreground';

  return (
    <div className="font-mono text-xs leading-relaxed border-b border-border/50 py-0.5">
      <div className="flex items-start gap-2">
        <span className="text-muted-foreground shrink-0">{formatTime(entry.timestamp)}</span>
        <span className={`shrink-0 font-semibold uppercase ${colorClass}`}>[{entry.category}]</span>
        <span className="text-foreground flex-1">{entry.message}</span>
        {entry.data !== undefined && (
          <button
            onClick={() => setExpanded(!expanded)}
            className="text-muted-foreground hover:text-foreground shrink-0"
          >
            {expanded ? <ChevronUp className="h-3 w-3" /> : <ChevronDown className="h-3 w-3" />}
          </button>
        )}
      </div>
      {expanded && entry.data !== undefined && (
        <pre className="ml-20 mt-1 text-muted-foreground overflow-x-auto max-h-40 overflow-y-auto bg-muted/50 rounded p-1">
          {JSON.stringify(entry.data, null, 2)}
        </pre>
      )}
    </div>
  );
}

export function DevModePanel() {
  const { enabled, logs, clear } = useDevMode();
  const [collapsed, setCollapsed] = useState(false);
  const [copied, setCopied] = useState(false);

  if (!enabled) return null;

  const handleCopy = () => {
    const text = logs
      .slice()
      .reverse()
      .map((e) => {
        const time = formatTime(e.timestamp);
        const data = e.data !== undefined ? ' ' + JSON.stringify(e.data) : '';
        return `${time} [${e.category}] ${e.message}${data}`;
      })
      .join('\n');
    navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="border-b-2 border-orange-500/50 bg-orange-50/50 dark:bg-orange-950/20">
      <div className="flex items-center justify-between px-4 py-1.5">
        <div className="flex items-center gap-2">
          <span className="text-xs font-bold text-orange-600 dark:text-orange-400 uppercase tracking-wider">
            Dev Mode
          </span>
          <span className="text-xs text-muted-foreground">
            {logs.length} log{logs.length !== 1 ? 's' : ''}
          </span>
        </div>
        <div className="flex items-center gap-1">
          <Button variant="ghost" size="sm" className="h-6 text-xs" onClick={handleCopy} disabled={logs.length === 0}>
            {copied ? <Check className="h-3 w-3 mr-1" /> : <Copy className="h-3 w-3 mr-1" />}
            {copied ? 'Copied' : 'Copy All'}
          </Button>
          <Button variant="ghost" size="sm" className="h-6 text-xs" onClick={clear} disabled={logs.length === 0}>
            <Trash2 className="h-3 w-3 mr-1" /> Clear
          </Button>
          <Button variant="ghost" size="sm" className="h-6 text-xs" onClick={() => setCollapsed(!collapsed)}>
            {collapsed ? <ChevronDown className="h-3 w-3" /> : <ChevronUp className="h-3 w-3" />}
          </Button>
        </div>
      </div>
      {!collapsed && (
        <div className="max-h-60 overflow-y-auto px-4 pb-2">
          {logs.length === 0 ? (
            <p className="text-xs text-muted-foreground py-2">
              No debug logs yet. Interact with the dashboard to generate logs.
            </p>
          ) : (
            logs.map((entry) => <LogLine key={entry.id} entry={entry} />)
          )}
        </div>
      )}
    </div>
  );
}
