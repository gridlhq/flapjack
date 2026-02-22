import { useState } from 'react';
import { useParams, Link } from 'react-router-dom';
import { Card } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Skeleton } from '@/components/ui/skeleton';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog';
import { useExperimentResults, useExperiment, useConcludeExperiment } from '@/hooks/useExperiments';
import type { ArmResultsResponse, ExperimentResultsResponse } from '@/hooks/useExperiments';
import { useUpdateSettings } from '@/hooks/useSettings';
import type { Experiment } from '@/lib/types';

const METRIC_LABELS: Record<string, string> = {
  ctr: 'CTR',
  conversionRate: 'Conversion Rate',
  revenuePerSearch: 'Revenue / Search',
  zeroResultRate: 'Zero Result Rate',
  abandonmentRate: 'Abandonment Rate',
  conversion_rate: 'Conversion Rate',
  revenue_per_search: 'Revenue / Search',
  zero_result_rate: 'Zero Result Rate',
  abandonment_rate: 'Abandonment Rate',
};

function formatPct(value: number): string {
  return `${(value * 100).toFixed(1)}%`;
}

function formatNumber(value: number): string {
  return value.toLocaleString();
}

function formatCurrency(value: number): string {
  return `$${value.toFixed(2)}`;
}

function formatDate(value: string | null | undefined): string {
  if (!value) return '';
  return value.slice(0, 10);
}

function formatPrimaryMetricValue(metric: string, value: number): string {
  switch (metric) {
    case 'revenuePerSearch':
    case 'revenue_per_search':
      return formatCurrency(value);
    case 'ctr':
    case 'conversionRate':
    case 'conversion_rate':
    case 'zeroResultRate':
    case 'zero_result_rate':
    case 'abandonmentRate':
    case 'abandonment_rate':
      return formatPct(value);
    default:
      return formatNumber(value);
  }
}

function statusBadgeClass(status: string): string {
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

function getPrimaryMetricValue(arm: ArmResultsResponse, metric: string): number {
  switch (metric) {
    case 'ctr': return arm.ctr;
    case 'conversionRate': return arm.conversionRate;
    case 'conversion_rate': return arm.conversionRate;
    case 'revenuePerSearch': return arm.revenuePerSearch;
    case 'revenue_per_search': return arm.revenuePerSearch;
    case 'zeroResultRate': return arm.zeroResultRate;
    case 'zero_result_rate': return arm.zeroResultRate;
    case 'abandonmentRate': return arm.abandonmentRate;
    case 'abandonment_rate': return arm.abandonmentRate;
    default: return arm.ctr;
  }
}

function defaultReason(results: ExperimentResultsResponse): string {
  const sig = results.significance;
  const metricLabel = METRIC_LABELS[results.primaryMetric] || results.primaryMetric;
  if (sig?.significant && sig.winner) {
    return `Statistically significant: ${sig.winner} wins on ${metricLabel} with ${(sig.confidence * 100).toFixed(1)}% confidence.`;
  }
  if (sig && !sig.significant) {
    return `No statistically significant difference detected on ${metricLabel}.`;
  }
  return '';
}

// --- DeclareWinnerDialog ---

type WinnerChoice = 'control' | 'variant' | 'none';

function SettingsDiff({ experiment }: { experiment: Experiment | undefined }) {
  if (!experiment) return null;

  const variant = experiment.variant;
  const overrides = variant.queryOverrides;
  const variantIndex = variant.indexName;
  const hasOverrides = !!overrides && Object.keys(overrides).length > 0;

  if (!hasOverrides && !variantIndex) return null;

  return (
    <div data-testid="settings-diff" className="rounded-md border p-3 bg-muted/50">
      <p className="text-sm font-medium mb-2">Variant Configuration</p>
      {variantIndex && (
        <p className="text-sm text-muted-foreground">
          Mode B: routes to index <span className="font-mono font-semibold">{variantIndex}</span>
        </p>
      )}
      {hasOverrides && (
        <ul className="text-sm space-y-1">
          {Object.entries(overrides).map(([key, value]) => (
            <li key={key} className="font-mono text-muted-foreground">
              {key}: {JSON.stringify(value)}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function DeclareWinnerDialog({
  open,
  onOpenChange,
  results,
  experimentId,
  experiment,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  results: ExperimentResultsResponse;
  experimentId: string;
  experiment: Experiment | undefined;
}) {
  const conclude = useConcludeExperiment();
  const indexName = results.indexName;
  const updateSettings = useUpdateSettings(indexName);
  const promoteOverrides = experiment?.variant.queryOverrides;
  const canPromote =
    !!promoteOverrides && Object.keys(promoteOverrides).length > 0;

  const initialWinner: WinnerChoice = results.significance?.winner === 'control'
    ? 'control'
    : results.significance?.winner === 'variant'
    ? 'variant'
    : 'none';

  const [winner, setWinner] = useState<WinnerChoice>(initialWinner);
  const [reason, setReason] = useState(defaultReason(results));
  const [promoted, setPromoted] = useState(false);

  const isPending = conclude.isPending || updateSettings.isPending;

  const handleConfirm = async () => {
    const promotedApplied = promoted && canPromote;
    if (promotedApplied) {
      await updateSettings.mutateAsync(promoteOverrides as Record<string, unknown>);
    }
    await conclude.mutateAsync({
      id: experimentId,
      payload: {
        winner: winner === 'none' ? null : winner,
        reason,
        controlMetric: getPrimaryMetricValue(results.control, results.primaryMetric),
        variantMetric: getPrimaryMetricValue(results.variant, results.primaryMetric),
        confidence: results.significance?.confidence ?? 0,
        significant: results.significance?.significant ?? false,
        promoted: promotedApplied,
      },
    });
    onOpenChange(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent data-testid="declare-winner-dialog">
        <DialogHeader>
          <DialogTitle>Choose a Winner</DialogTitle>
          <DialogDescription>
            Conclude this experiment by declaring a winner or marking it as inconclusive.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          {/* Settings Diff */}
          <SettingsDiff experiment={experiment} />

          {/* Winner Selection */}
          <fieldset>
            <legend className="text-sm font-medium mb-2">Winner</legend>
            <div className="space-y-2">
              <label className="flex items-center gap-2 cursor-pointer">
                <input
                  type="radio"
                  name="winner"
                  value="control"
                  checked={winner === 'control'}
                  onChange={() => setWinner('control')}
                  aria-label="Control"
                />
                <span className="text-sm">Control</span>
              </label>
              <label className="flex items-center gap-2 cursor-pointer">
                <input
                  type="radio"
                  name="winner"
                  value="variant"
                  checked={winner === 'variant'}
                  onChange={() => setWinner('variant')}
                  aria-label="Variant"
                />
                <span className="text-sm">Variant</span>
              </label>
              <label className="flex items-center gap-2 cursor-pointer">
                <input
                  type="radio"
                  name="winner"
                  value="none"
                  checked={winner === 'none'}
                  onChange={() => setWinner('none')}
                  aria-label="No Winner"
                />
                <span className="text-sm">No Winner (inconclusive)</span>
              </label>
            </div>
          </fieldset>

          {/* Reason */}
          <div>
            <label htmlFor="conclude-reason" className="text-sm font-medium">
              Reason
            </label>
            <textarea
              id="conclude-reason"
              aria-label="Reason"
              className="mt-1 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring min-h-[80px]"
              value={reason}
              onChange={(e) => setReason(e.target.value)}
            />
          </div>

          {/* Promote checkbox (Mode A only) */}
          {canPromote && (
            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                checked={promoted}
                onChange={(e) => setPromoted(e.target.checked)}
                aria-label="Promote winner settings"
              />
              <span className="text-sm">Promote winner settings to the base index</span>
            </label>
          )}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)} disabled={isPending}>
            Cancel
          </Button>
          <Button onClick={handleConfirm} disabled={isPending}>
            {isPending ? 'Concluding...' : 'Confirm'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// --- ArmMetricsCard ---

function ArmMetricsCard({ arm, label, testId }: {
  arm: ArmResultsResponse;
  label: string;
  testId: string;
}) {
  return (
    <Card className="p-4 flex-1" data-testid={testId}>
      <h4 className="text-sm font-semibold mb-3 capitalize">{label}</h4>
      <div className="space-y-2 text-sm">
        <div className="flex justify-between">
          <span className="text-muted-foreground">CTR</span>
          <span>{formatPct(arm.ctr)}</span>
        </div>
        <div className="flex justify-between">
          <span className="text-muted-foreground">Conversion Rate</span>
          <span>{formatPct(arm.conversionRate)}</span>
        </div>
        <div className="flex justify-between">
          <span className="text-muted-foreground">Revenue / Search</span>
          <span>{formatCurrency(arm.revenuePerSearch)}</span>
        </div>
        <div className="flex justify-between">
          <span className="text-muted-foreground">Zero Result Rate</span>
          <span>{formatPct(arm.zeroResultRate)}</span>
        </div>
        <div className="flex justify-between">
          <span className="text-muted-foreground">Abandonment Rate</span>
          <span>{formatPct(arm.abandonmentRate)}</span>
        </div>
        <div className="border-t pt-2 mt-2">
          <div className="flex justify-between">
            <span className="text-muted-foreground">Searches</span>
            <span>{formatNumber(arm.searches)}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-muted-foreground">Users</span>
            <span>{formatNumber(arm.users)}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-muted-foreground">Clicks</span>
            <span>{formatNumber(arm.clicks)}</span>
          </div>
        </div>
      </div>
    </Card>
  );
}

// --- Main Page ---

export function ExperimentDetail() {
  const { experimentId } = useParams<{ experimentId: string }>();
  const { data: results, isLoading } = useExperimentResults(experimentId || '');
  const { data: experiment } = useExperiment(experimentId || '');
  const [showDeclareWinner, setShowDeclareWinner] = useState(false);
  const [showDaysGateConfirmation, setShowDaysGateConfirmation] = useState(false);

  if (isLoading || !results) {
    return (
      <div className="space-y-6">
        <Skeleton className="h-8 w-48" />
        <div className="text-sm text-muted-foreground">Loading...</div>
        <Card className="p-6 space-y-3">
          <Skeleton className="h-10 w-full" />
          <Skeleton className="h-10 w-full" />
        </Card>
      </div>
    );
  }

  const totalSearches = results.control.searches + results.variant.searches;
  const guardRailAlerts = results.guardRailAlerts ?? [];
  const outlierUsersExcluded = results.outlierUsersExcluded ?? 0;
  const noStableIdQueries = results.noStableIdQueries ?? 0;
  const controlMeanClickRank = results.control.meanClickRank ?? 0;
  const variantMeanClickRank = results.variant.meanClickRank ?? 0;
  const unstableIdFraction = totalSearches > 0
    ? noStableIdQueries / totalSearches
    : 0;

  const canDeclareWinner = results.gate.minimumNReached && results.status !== 'concluded';
  const needsDaysGateWarning = results.gate.minimumNReached && !results.gate.minimumDaysReached;
  const hasConclusion = results.status === 'concluded' && !!results.conclusion;
  const conclusionWinnerLabel =
    results.conclusion?.winner === 'control'
      ? 'Control'
      : results.conclusion?.winner === 'variant'
        ? 'Variant'
        : 'No winner (inconclusive)';
  const conclusionMetricLabel = METRIC_LABELS[results.primaryMetric] || results.primaryMetric;

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-start justify-between gap-4">
        <div>
          <div className="flex items-center gap-3">
            <Link
              to="/experiments"
              className="text-sm text-muted-foreground hover:underline"
              data-testid="experiment-detail-back-link"
            >
              &larr; Experiments
            </Link>
          </div>
          <h2 className="text-2xl font-bold mt-2" data-testid="experiment-detail-name">{results.name}</h2>
          <div className="flex items-center gap-3 mt-1">
            <Badge variant="outline" className={statusBadgeClass(results.status)} data-testid="experiment-detail-status">
              {results.status}
            </Badge>
            <span className="text-sm text-muted-foreground">
              <span data-testid="experiment-detail-index">{results.indexName}</span>
              {' '}
              &middot;
              {' '}
              <span data-testid="experiment-detail-primary-metric">
                {METRIC_LABELS[results.primaryMetric] || results.primaryMetric}
              </span>
            </span>
          </div>
        </div>
        {canDeclareWinner && (
          <Button
            data-testid="declare-winner-button"
            onClick={() => {
              if (needsDaysGateWarning) {
                setShowDaysGateConfirmation(true);
              } else {
                setShowDeclareWinner(true);
              }
            }}
          >
            Declare Winner
          </Button>
        )}
      </div>

      {/* Days Gate Confirmation (novelty warning) */}
      {showDaysGateConfirmation && (
        <Card className="p-4 border-amber-300 bg-amber-50" data-testid="days-gate-confirmation">
          <p className="text-sm font-medium text-amber-800 mb-3">
            The minimum duration has not been reached. Concluding early risks a novelty effect
            skewing results. Are you sure you want to proceed?
          </p>
          <div className="flex gap-2">
            <Button variant="outline" size="sm" onClick={() => setShowDaysGateConfirmation(false)}>
              Cancel
            </Button>
            <Button size="sm" onClick={() => {
              setShowDaysGateConfirmation(false);
              setShowDeclareWinner(true);
            }}>
              Proceed Anyway
            </Button>
          </div>
        </Card>
      )}

      {/* Declare Winner Dialog */}
      {canDeclareWinner && (
        <DeclareWinnerDialog
          open={showDeclareWinner}
          onOpenChange={setShowDeclareWinner}
          results={results}
          experimentId={experimentId || ''}
          experiment={experiment}
        />
      )}

      {/* Concluded Summary */}
      {hasConclusion && (
        <Card className="p-4 border-blue-200 bg-blue-50" data-testid="conclusion-card">
          <h3 className="text-sm font-semibold text-blue-900">Declared Winner</h3>
          <div className="mt-2 space-y-1 text-sm text-blue-900">
            <p>
              Winner: <span className="font-semibold">{conclusionWinnerLabel}</span>
            </p>
            <p>
              Confidence: {(results.conclusion!.confidence * 100).toFixed(1)}% confidence
            </p>
            <p>
              {conclusionMetricLabel}: control {formatPrimaryMetricValue(
                results.primaryMetric,
                results.conclusion!.controlMetric,
              )} vs variant {formatPrimaryMetricValue(
                results.primaryMetric,
                results.conclusion!.variantMetric,
              )}
            </p>
            <p>
              Promoted to base index: {results.conclusion!.promoted ? 'Yes' : 'No'}
            </p>
            {results.endedAt && (
              <p>
                Ended: {formatDate(results.endedAt)}
              </p>
            )}
            <p>{results.conclusion!.reason}</p>
          </div>
        </Card>
      )}

      {/* Minimum Days Warning */}
      {needsDaysGateWarning && (
        <Card className="p-4 border-amber-200 bg-amber-50" data-testid="minimum-days-warning">
          <p className="text-sm font-medium text-amber-800">
            Required sample size reached, but the minimum duration has not elapsed.
            Results may be influenced by novelty effects. Consider waiting before concluding.
          </p>
        </Card>
      )}

      {/* SRM Warning */}
      {results.sampleRatioMismatch && (
        <Card className="p-4 border-amber-300 bg-amber-50" data-testid="srm-banner">
          <p className="text-sm font-medium text-amber-800">
            Traffic split mismatch detected. Possible causes: bot traffic, cookie clearing,
            variant index errors. Results may be invalid. Investigate before concluding.
          </p>
        </Card>
      )}

      {/* Guard Rail Alerts */}
      {guardRailAlerts.length > 0 && (
        <Card className="p-4 border-amber-300 bg-amber-50" data-testid="guard-rail-banner">
          <h3 className="text-sm font-semibold text-amber-900">Guard Rail Alert</h3>
          <div className="mt-2 space-y-1 text-sm text-amber-900">
            {guardRailAlerts.map((alert, index) => (
              <p key={`${alert.metricName}-${index}`}>
                <span className="font-semibold">{METRIC_LABELS[alert.metricName] || alert.metricName}</span>
                {': '}
                {alert.dropPct.toFixed(1)}% regression
                {' '}
                (control {formatPrimaryMetricValue(alert.metricName, alert.controlValue)} vs variant {formatPrimaryMetricValue(alert.metricName, alert.variantValue)})
              </p>
            ))}
          </div>
        </Card>
      )}

      {/* Progress Bar */}
      {!results.gate.readyToRead && (
        <Card className="p-4" data-testid="progress-bar">
          <div className="flex items-center justify-between text-sm mb-2">
            <span className="font-medium">Data collection progress</span>
            <span>
              {formatNumber(results.gate.currentSearchesPerArm)} / {formatNumber(results.gate.requiredSearchesPerArm)} searches per arm ({results.gate.progressPct.toFixed(1)}%)
            </span>
          </div>
          <div className="w-full bg-slate-200 rounded-full h-2">
            <div
              className="bg-blue-600 h-2 rounded-full transition-all"
              style={{ width: `${Math.min(results.gate.progressPct, 100)}%` }}
            />
          </div>
          {results.gate.estimatedDaysRemaining != null && (
            <p className="text-xs text-muted-foreground mt-2">
              ~{results.gate.estimatedDaysRemaining.toFixed(1)} days remaining
            </p>
          )}
        </Card>
      )}

      {/* Bayesian Card (always visible) */}
      {results.bayesian && (
        <Card className="p-4" data-testid="bayesian-card">
          <h3 className="text-sm font-semibold mb-1">Bayesian Probability</h3>
          <p className="text-2xl font-bold">{Math.round(results.bayesian.probVariantBetter * 100)}% probability variant wins</p>
          <p className="text-xs text-muted-foreground mt-1">
            Valid to inspect at any time. Useful when frequentist significance may take weeks.
          </p>
        </Card>
      )}

      {/* Significance Section (available once sample size N reached) */}
      {results.gate.minimumNReached && results.significance && (
        <Card className="p-4" data-testid="significance-section">
          <div className="mb-2 flex items-center gap-2">
            <h3 className="text-sm font-semibold">Statistical Significance</h3>
            {results.cupedApplied && (
              <Badge
                variant="outline"
                className="border-emerald-300 bg-emerald-50 text-emerald-800"
                data-testid="cuped-badge"
              >
                CUPED
              </Badge>
            )}
          </div>
          <div className="flex items-center gap-4">
            <div className="flex-1">
              <div className="w-full bg-slate-200 rounded-full h-3">
                <div
                  className={`h-3 rounded-full transition-all ${
                    results.significance.confidence >= 0.95
                      ? 'bg-emerald-600'
                      : results.significance.confidence >= 0.90
                      ? 'bg-emerald-400'
                      : results.significance.confidence >= 0.50
                      ? 'bg-amber-400'
                      : 'bg-red-400'
                  }`}
                  style={{ width: `${Math.min(results.significance.confidence * 100, 100)}%` }}
                />
              </div>
            </div>
            <span className="text-lg font-bold whitespace-nowrap">
              {(results.significance.confidence * 100).toFixed(1)}% confidence
            </span>
          </div>
          {results.significance.significant && results.significance.winner && (
            <p className="text-sm mt-2">
              Winner: <span className="font-semibold capitalize">{results.significance.winner}</span>
              {' '}({(results.significance.relativeImprovement * 100).toFixed(1)}% improvement)
            </p>
          )}
        </Card>
      )}

      {/* Recommendation */}
      {results.recommendation && (
        <Card className="p-4 bg-blue-50 border-blue-200">
          <p className="text-sm text-blue-800">{results.recommendation}</p>
        </Card>
      )}

      {/* Metric Cards */}
      <div className="grid grid-cols-2 gap-4">
        <ArmMetricsCard arm={results.control} label="Control" testId="metric-card-control" />
        <ArmMetricsCard arm={results.variant} label="Variant" testId="metric-card-variant" />
      </div>

      {/* MeanClickRank Diagnostic */}
      <Card className="p-4" data-testid="mean-click-rank-card">
        <div className="flex items-center justify-between gap-4">
          <h3 className="text-sm font-semibold">Avg Click Position</h3>
          <span className="text-xs text-muted-foreground">&darr; Lower is better</span>
        </div>
        <div className="mt-3 grid grid-cols-2 gap-4">
          <div>
            <p className="text-xs text-muted-foreground">Control</p>
            <p className="text-lg font-semibold">{controlMeanClickRank.toFixed(2)}</p>
          </div>
          <div>
            <p className="text-xs text-muted-foreground">Variant</p>
            <p className="text-lg font-semibold">{variantMeanClickRank.toFixed(2)}</p>
          </div>
        </div>
      </Card>

      {/* Notices */}
      {outlierUsersExcluded > 0 && (
        <p className="text-xs text-muted-foreground">
          {outlierUsersExcluded} users excluded as outliers (bot-like traffic patterns).
        </p>
      )}

      {unstableIdFraction > 0.05 && noStableIdQueries > 0 && (
        <p className="text-xs text-muted-foreground">
          {formatNumber(noStableIdQueries)} queries ({(unstableIdFraction * 100).toFixed(1)}%) used unstable IDs and are excluded from arm statistics. Verify your userToken implementation.
        </p>
      )}
    </div>
  );
}
