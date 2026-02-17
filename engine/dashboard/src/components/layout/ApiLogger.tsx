import { useApiLogger } from '@/hooks/useApiLogger';
import { Button } from '@/components/ui/button';
import { ChevronDown, ChevronUp, Download, Trash2 } from 'lucide-react';
import { formatDuration } from '@/lib/utils';

export function ApiLogger() {
  const { entries, isExpanded, toggleExpanded, clear, exportAsFile } = useApiLogger();
  const lastEntry = entries[0];

  return (
    <div
      className={`border-t border-border bg-background transition-all duration-200 ${
        isExpanded ? 'h-[40vh]' : 'h-[50px]'
      }`}
    >
      {/* Collapsed state */}
      <div className="flex h-[50px] items-center justify-between px-4 border-b border-border">
        <div className="flex items-center gap-4">
          <Button variant="ghost" size="sm" onClick={toggleExpanded}>
            {isExpanded ? <ChevronDown className="h-4 w-4" /> : <ChevronUp className="h-4 w-4" />}
            <span className="ml-2">📋 API Log ({entries.length})</span>
          </Button>
          {lastEntry && !isExpanded && (
            <span className="text-sm text-muted-foreground truncate max-w-md">
              Last: {lastEntry.method} {lastEntry.url} - {formatDuration(lastEntry.duration)}
            </span>
          )}
        </div>
        <div className="flex gap-2">
          <Button variant="ghost" size="sm" onClick={exportAsFile} disabled={entries.length === 0}>
            <Download className="h-4 w-4 mr-1" /> Export
          </Button>
          <Button variant="ghost" size="sm" onClick={clear} disabled={entries.length === 0}>
            <Trash2 className="h-4 w-4 mr-1" /> Clear
          </Button>
        </div>
      </div>

      {/* Expanded state */}
      {isExpanded && (
        <div className="h-[calc(40vh-50px)] overflow-y-auto px-4 py-2">
          {entries.length === 0 ? (
            <p className="text-sm text-muted-foreground text-center py-8">
              No API requests yet. API calls will appear here.
            </p>
          ) : (
            <div className="space-y-2">
              {entries.map((entry) => (
                <div
                  key={entry.id}
                  className="p-3 rounded-md border border-border bg-card text-sm"
                >
                  <div className="flex items-center justify-between">
                    <span className="font-medium">
                      <span className={entry.status === 'success' ? 'text-green-600' : entry.status === 'error' ? 'text-red-600' : 'text-yellow-600'}>
                        {entry.status === 'success' ? '✓' : entry.status === 'error' ? '✗' : '⏳'}
                      </span>
                      {' '}
                      {entry.method} {entry.url}
                    </span>
                    <span className="text-muted-foreground">{formatDuration(entry.duration)}</span>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
