import { useState, useCallback } from 'react';
import { Plus, Trash2, RefreshCw, ChevronDown, ChevronRight } from 'lucide-react';
import {
  useQsConfigs,
  useQsBuildStatus,
  useQsLogs,
  useDeleteQsConfig,
  useTriggerQsBuild,
} from '@/hooks/useQuerySuggestions';
import { CreateQsConfigDialog } from '@/components/query-suggestions/CreateQsConfigDialog';
import { Card } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Skeleton } from '@/components/ui/skeleton';
import type { QsConfig } from '@/lib/types';

function formatDate(iso: string | null): string {
  if (!iso) return 'Never';
  const d = new Date(iso);
  return d.toLocaleString();
}

// ── Per-config row ────────────────────────────────────────────────────────────

interface ConfigRowProps {
  config: QsConfig;
  onDelete: (name: string) => void;
  onRebuild: (name: string) => void;
  isDeleting: boolean;
  isRebuilding: boolean;
}

function ConfigRow({ config, onDelete, onRebuild, isDeleting, isRebuilding }: ConfigRowProps) {
  const { data: status } = useQsBuildStatus(config.indexName);
  const { data: logs } = useQsLogs(config.indexName);
  const [showLogs, setShowLogs] = useState(false);

  return (
    <Card className="p-5 space-y-3" data-testid="qs-config-card">
      {/* Header row */}
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <h3 className="font-semibold truncate" data-testid="qs-config-name">
            {config.indexName}
          </h3>
          <p className="text-sm text-muted-foreground">
            Source:{' '}
            {config.sourceIndices.map((s) => s.indexName).join(', ')}
          </p>
        </div>

        <div className="flex items-center gap-2 shrink-0">
          {/* Running badge */}
          {status?.isRunning && (
            <Badge variant="secondary" className="animate-pulse" data-testid="badge-running">
              Building…
            </Badge>
          )}

          {/* Rebuild button */}
          <Button
            variant="outline"
            size="sm"
            onClick={() => onRebuild(config.indexName)}
            disabled={isRebuilding || status?.isRunning}
            data-testid="rebuild-btn"
            aria-label="Rebuild suggestions index"
          >
            <RefreshCw className="h-3.5 w-3.5 mr-1" />
            Rebuild
          </Button>

          {/* Delete button */}
          <Button
            variant="ghost"
            size="sm"
            onClick={() => onDelete(config.indexName)}
            disabled={isDeleting}
            data-testid="delete-config-btn"
            aria-label={`Delete config ${config.indexName}`}
          >
            <Trash2 className="h-4 w-4 text-destructive" />
          </Button>
        </div>
      </div>

      {/* Build status */}
      <div className="grid grid-cols-2 md:grid-cols-3 gap-3 text-sm">
        <div>
          <div className="text-muted-foreground text-xs">Last Built</div>
          <div className="font-medium">{formatDate(status?.lastBuiltAt ?? null)}</div>
        </div>
        <div>
          <div className="text-muted-foreground text-xs">Last Successful Build</div>
          <div className="font-medium">{formatDate(status?.lastSuccessfulBuiltAt ?? null)}</div>
        </div>
        <div>
          <div className="text-muted-foreground text-xs">Min Hits</div>
          <div className="font-medium">{config.sourceIndices[0]?.minHits ?? 5}</div>
        </div>
      </div>

      {/* Exclude list */}
      {config.exclude && config.exclude.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          <span className="text-xs text-muted-foreground self-center">Excluded:</span>
          {config.exclude.map((word) => (
            <Badge key={word} variant="outline" className="text-xs">
              {word}
            </Badge>
          ))}
        </div>
      )}

      {/* Build logs (collapsible) */}
      {logs && logs.length > 0 && (
        <div>
          <button
            className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
            onClick={() => setShowLogs((s) => !s)}
            aria-expanded={showLogs}
            aria-controls={`logs-${config.indexName}`}
          >
            {showLogs ? (
              <ChevronDown className="h-3 w-3" />
            ) : (
              <ChevronRight className="h-3 w-3" />
            )}
            Build logs ({logs.length})
          </button>
          {showLogs && (
            <div
              id={`logs-${config.indexName}`}
              className="mt-2 rounded-md bg-muted p-3 text-xs font-mono space-y-0.5 max-h-40 overflow-y-auto"
              data-testid="build-logs"
            >
              {logs.map((entry, i) => (
                <div key={i} className="flex gap-2">
                  <span className="text-muted-foreground shrink-0">
                    {new Date(entry.timestamp).toLocaleTimeString()}
                  </span>
                  <span
                    className={
                      entry.level === 'error'
                        ? 'text-destructive'
                        : entry.level === 'warn'
                          ? 'text-amber-500'
                          : ''
                    }
                  >
                    [{entry.level.toUpperCase()}] {entry.message}
                  </span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </Card>
  );
}

// ── Main page ─────────────────────────────────────────────────────────────────

export function QuerySuggestions() {
  const { data: configs, isLoading } = useQsConfigs();
  const deleteConfig = useDeleteQsConfig();
  const triggerBuild = useTriggerQsBuild();
  const [showCreateDialog, setShowCreateDialog] = useState(false);

  const handleDelete = useCallback(
    (indexName: string) => {
      const confirmed = confirm(
        `Delete config "${indexName}"? The suggestions index will be preserved.`
      );
      if (!confirmed) return;
      deleteConfig.mutate(indexName);
    },
    [deleteConfig]
  );

  const handleRebuild = useCallback(
    (indexName: string) => {
      triggerBuild.mutate(indexName);
    },
    [triggerBuild]
  );

  if (isLoading) {
    return (
      <div className="space-y-6">
        <div className="flex items-center justify-between">
          <div className="space-y-2">
            <Skeleton className="h-8 w-48" />
            <Skeleton className="h-4 w-72" />
          </div>
          <Skeleton className="h-10 w-36 rounded-md" />
        </div>
        {[1, 2].map((i) => (
          <Card key={i} className="p-5 space-y-3">
            <div className="flex items-start justify-between">
              <div className="space-y-2">
                <Skeleton className="h-5 w-40" />
                <Skeleton className="h-4 w-32" />
              </div>
              <Skeleton className="h-8 w-24 rounded-md" />
            </div>
            <div className="grid grid-cols-3 gap-3">
              <Skeleton className="h-10" />
              <Skeleton className="h-10" />
              <Skeleton className="h-10" />
            </div>
          </Card>
        ))}
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Page header */}
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold">Query Suggestions</h2>
          <p className="text-sm text-muted-foreground mt-1">
            Build autocomplete suggestion indexes from real search analytics.
          </p>
        </div>
        <Button onClick={() => setShowCreateDialog(true)}>
          <Plus className="h-4 w-4 mr-1" />
          Create Config
        </Button>
      </div>

      {/* Empty state */}
      {!configs || configs.length === 0 ? (
        <Card className="p-8 text-center">
          <h3 className="text-lg font-semibold mb-2">No Query Suggestions configs</h3>
          <p className="text-sm text-muted-foreground mb-4">
            Create a config to start building suggestion indexes from search analytics.
          </p>
          <Button onClick={() => setShowCreateDialog(true)}>
            <Plus className="h-4 w-4 mr-1" />
            Create Your First Config
          </Button>
        </Card>
      ) : (
        <div className="space-y-4" data-testid="qs-configs-list">
          {configs.map((config) => (
            <ConfigRow
              key={config.indexName}
              config={config}
              onDelete={handleDelete}
              onRebuild={handleRebuild}
              isDeleting={deleteConfig.isPending}
              isRebuilding={triggerBuild.isPending}
            />
          ))}
        </div>
      )}

      <CreateQsConfigDialog
        open={showCreateDialog}
        onOpenChange={setShowCreateDialog}
      />
    </div>
  );
}
