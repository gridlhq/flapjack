import { useState } from 'react';
import { Link } from 'react-router-dom';
import { Card } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Skeleton } from '@/components/ui/skeleton';
import { ConfirmDialog } from '@/components/ui/confirm-dialog';
import { useExperiments, useStopExperiment, useDeleteExperiment } from '@/hooks/useExperiments';
import type { Experiment, ExperimentStatus } from '@/lib/types';
import { CreateExperimentDialog } from '@/components/experiments/CreateExperimentDialog';

const METRIC_LABELS: Record<string, string> = {
  ctr: 'CTR',
  conversionRate: 'Conversion',
  revenuePerSearch: 'Revenue / Search',
  zeroResultRate: 'Zero Result Rate',
  abandonmentRate: 'Abandonment Rate',
  conversion_rate: 'Conversion',
  revenue_per_search: 'Revenue / Search',
  zero_result_rate: 'Zero Result Rate',
  abandonment_rate: 'Abandonment Rate',
};

function formatTrafficSplit(value: number): string {
  return `${Math.round(value * 100)}%`;
}

function formatMetric(metric: string): string {
  return METRIC_LABELS[metric] || metric;
}

function formatDate(ts: number | null | undefined): string {
  if (!ts) return '—';
  return new Date(ts).toLocaleDateString();
}

function statusBadgeClass(status: ExperimentStatus): string {
  switch (status) {
    case 'running':
      return 'bg-emerald-100 text-emerald-800 border-emerald-200 animate-pulse';
    case 'draft':
      return 'bg-slate-100 text-slate-700 border-slate-300';
    case 'stopped':
      return 'bg-orange-100 text-orange-800 border-orange-200';
    case 'concluded':
      return 'bg-blue-100 text-blue-800 border-blue-200';
    default:
      return 'bg-slate-100 text-slate-700 border-slate-300';
  }
}

interface ExperimentRowProps {
  experiment: Experiment;
  onStop: (id: string) => void;
  onDelete: (id: string) => void;
}

function ExperimentRow({ experiment, onStop, onDelete }: ExperimentRowProps) {
  const isRunning = experiment.status === 'running';

  return (
    <tr className="border-b last:border-0" data-testid={`experiment-row-${experiment.id}`}>
      <td className="py-3 pr-3 font-medium">
        <Link to={`/experiments/${experiment.id}`} className="hover:underline text-blue-600">
          {experiment.name}
        </Link>
      </td>
      <td className="py-3 pr-3 text-muted-foreground">{experiment.indexName}</td>
      <td className="py-3 pr-3">
        <Badge
          variant="outline"
          className={statusBadgeClass(experiment.status)}
          data-testid={`experiment-status-${experiment.id}`}
        >
          {experiment.status}
        </Badge>
      </td>
      <td className="py-3 pr-3">{formatMetric(experiment.primaryMetric)}</td>
      <td className="py-3 pr-3 text-right">{formatTrafficSplit(experiment.trafficSplit)}</td>
      <td className="py-3 pr-3" data-testid={`experiment-started-${experiment.id}`}>
        {formatDate(experiment.startedAt)}
      </td>
      <td className="py-3 pr-3 text-right">
        <div className="flex items-center justify-end gap-2">
          {isRunning && (
            <Button
              variant="outline"
              size="sm"
              data-testid={`stop-experiment-${experiment.id}`}
              onClick={() => onStop(experiment.id)}
            >
              Stop
            </Button>
          )}
          <Button
            variant="ghost"
            size="sm"
            data-testid={`delete-experiment-${experiment.id}`}
            disabled={isRunning}
            onClick={() => onDelete(experiment.id)}
          >
            Delete
          </Button>
        </div>
      </td>
    </tr>
  );
}

export function Experiments() {
  const { data: experiments, isLoading } = useExperiments();
  const stopMutation = useStopExperiment();
  const deleteMutation = useDeleteExperiment();
  const [isCreateDialogOpen, setIsCreateDialogOpen] = useState(false);

  const [confirmAction, setConfirmAction] = useState<{
    type: 'stop' | 'delete';
    id: string;
  } | null>(null);

  function handleStopClick(id: string) {
    setConfirmAction({ type: 'stop', id });
  }

  function handleDeleteClick(id: string) {
    setConfirmAction({ type: 'delete', id });
  }

  function handleConfirm() {
    if (!confirmAction) return;
    if (confirmAction.type === 'stop') {
      stopMutation.mutate(confirmAction.id);
    } else {
      deleteMutation.mutate(confirmAction.id);
    }
    setConfirmAction(null);
  }

  if (isLoading) {
    return (
      <div className="space-y-6">
        <div className="space-y-2">
          <Skeleton className="h-8 w-48" />
          <Skeleton className="h-4 w-80" />
        </div>
        <Card className="p-6 space-y-3">
          <Skeleton className="h-10 w-full" />
          <Skeleton className="h-10 w-full" />
          <Skeleton className="h-10 w-full" />
        </Card>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h2 className="text-2xl font-bold" data-testid="experiments-heading">Experiments</h2>
          <p className="text-sm text-muted-foreground mt-1">
            Compare search strategies and safely roll out winners.
          </p>
        </div>
        <Button onClick={() => setIsCreateDialogOpen(true)}>
          Create Experiment
        </Button>
      </div>

      {!experiments || experiments.length === 0 ? (
        <Card className="p-8 text-center" data-testid="experiments-empty-state">
          <h3 className="text-lg font-semibold mb-2">No experiments yet</h3>
          <p className="text-sm text-muted-foreground">
            Create an experiment to compare control and variant performance.
          </p>
        </Card>
      ) : (
        <Card className="p-6">
          <div className="overflow-x-auto">
            <table className="w-full text-sm" data-testid="experiments-table">
              <thead>
                <tr className="border-b text-left text-muted-foreground">
                  <th className="pb-3 pr-3 font-medium">Name</th>
                  <th className="pb-3 pr-3 font-medium">Index</th>
                  <th className="pb-3 pr-3 font-medium">Status</th>
                  <th className="pb-3 pr-3 font-medium">Metric</th>
                  <th className="pb-3 pr-3 font-medium text-right">Traffic split</th>
                  <th className="pb-3 pr-3 font-medium">Started</th>
                  <th className="pb-3 pr-3 font-medium text-right">Actions</th>
                </tr>
              </thead>
              <tbody>
                {experiments.map((experiment) => (
                  <ExperimentRow
                    key={experiment.id}
                    experiment={experiment}
                    onStop={handleStopClick}
                    onDelete={handleDeleteClick}
                  />
                ))}
              </tbody>
            </table>
          </div>
        </Card>
      )}

      <ConfirmDialog
        open={confirmAction?.type === 'stop'}
        onOpenChange={(open) => { if (!open) setConfirmAction(null); }}
        title="Stop experiment"
        description="Are you sure you want to stop this experiment? This action cannot be undone."
        confirmLabel="Stop"
        variant="destructive"
        onConfirm={handleConfirm}
        isPending={stopMutation.isPending}
      />

      <ConfirmDialog
        open={confirmAction?.type === 'delete'}
        onOpenChange={(open) => { if (!open) setConfirmAction(null); }}
        title="Delete experiment"
        description="Are you sure you want to delete this experiment? All data will be permanently removed."
        confirmLabel="Delete"
        variant="destructive"
        onConfirm={handleConfirm}
        isPending={deleteMutation.isPending}
      />

      <CreateExperimentDialog
        open={isCreateDialogOpen}
        onOpenChange={setIsCreateDialogOpen}
      />
    </div>
  );
}
