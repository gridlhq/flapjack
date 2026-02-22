import { useEffect, useMemo, useState } from 'react';
import { useIndexes } from '@/hooks/useIndexes';
import { useCreateExperiment } from '@/hooks/useExperiments';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';

type PrimaryMetric =
  | 'ctr'
  | 'conversionRate'
  | 'revenuePerSearch'
  | 'zeroResultRate'
  | 'abandonmentRate';

type WizardMode = 'modeA' | 'modeB';

interface CreateExperimentDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

const METRIC_OPTIONS: Array<{
  value: PrimaryMetric;
  label: string;
  description: string;
  testId: string;
}> = [
  {
    value: 'ctr',
    label: 'CTR',
    description: 'Tracks click-through rate for result relevance.',
    testId: 'metric-ctr',
  },
  {
    value: 'conversionRate',
    label: 'Conversion rate',
    description: 'Tracks conversions per search.',
    testId: 'metric-conversion-rate',
  },
  {
    value: 'revenuePerSearch',
    label: 'Revenue per search',
    description: 'Tracks average revenue generated per search.',
    testId: 'metric-revenue-per-search',
  },
  {
    value: 'zeroResultRate',
    label: 'Zero result rate',
    description: 'Tracks how often queries return no results.',
    testId: 'metric-zero-result-rate',
  },
  {
    value: 'abandonmentRate',
    label: 'Abandonment rate',
    description: 'Tracks no-click behavior when results are shown.',
    testId: 'metric-abandonment-rate',
  },
];

const SAMPLE_SIZE_ROWS = [
  { label: 'Large gain (10% relative)', baseDays: 13 },
  { label: 'Typical early-stage gain (5%)', baseDays: 25 },
  { label: 'Small gain (2%)', baseDays: 165 },
  { label: 'Mature product (1%)', baseDays: 833 },
];

function estimateRuntimeDays(baseDaysAt50Pct: number, trafficSplitPercent: number): number {
  const safePercent = Math.max(1, Math.min(99, trafficSplitPercent));
  const bottleneckArmPercent = Math.min(safePercent, 100 - safePercent);
  const splitFactor = 50 / bottleneckArmPercent;
  return Math.round(baseDaysAt50Pct * splitFactor);
}

export function CreateExperimentDialog({ open, onOpenChange }: CreateExperimentDialogProps) {
  const { data: indexes } = useIndexes();
  const createExperiment = useCreateExperiment();

  const [step, setStep] = useState(1);
  const [name, setName] = useState('');
  const [indexName, setIndexName] = useState('');
  const [primaryMetric, setPrimaryMetric] = useState<PrimaryMetric>('ctr');
  const [mode, setMode] = useState<WizardMode>('modeA');
  const [variantIndexName, setVariantIndexName] = useState('');
  const [enableSynonyms, setEnableSynonyms] = useState(false);
  const [enableRules, setEnableRules] = useState(false);
  const [filters, setFilters] = useState('');
  const [trafficSplitPercent, setTrafficSplitPercent] = useState(50);
  const [minimumDays, setMinimumDays] = useState(14);

  useEffect(() => {
    if (!open) return;
    setStep(1);
    setName('');
    setIndexName('');
    setPrimaryMetric('ctr');
    setMode('modeA');
    setVariantIndexName('');
    setEnableSynonyms(false);
    setEnableRules(false);
    setFilters('');
    setTrafficSplitPercent(50);
    setMinimumDays(14);
  }, [open]);

  const selectedMetric = useMemo(
    () => METRIC_OPTIONS.find((metric) => metric.value === primaryMetric),
    [primaryMetric]
  );

  const estimatedRows = useMemo(
    () =>
      SAMPLE_SIZE_ROWS.map((row) => ({
        ...row,
        estimatedDays: estimateRuntimeDays(row.baseDays, trafficSplitPercent),
      })),
    [trafficSplitPercent]
  );

  const typicalRuntimeDays = estimatedRows[1]?.estimatedDays ?? 0;
  const showRuntimeWarning = typicalRuntimeDays > 90;
  const showRuntimeDanger = typicalRuntimeDays > 365;

  function canProceedStep1() {
    return name.trim().length > 0 && indexName.length > 0;
  }

  function canProceedStep2() {
    if (mode === 'modeA') return true;
    return variantIndexName.length > 0 && variantIndexName !== indexName;
  }

  function canProceed() {
    if (step === 1) return canProceedStep1();
    if (step === 2) return canProceedStep2();
    return true;
  }

  function buildVariantPayload() {
    if (mode === 'modeB') {
      return {
        name: 'variant',
        indexName: variantIndexName,
      };
    }

    const queryOverrides: Record<string, unknown> = {
      enableSynonyms,
      enableRules,
    };
    if (filters.trim()) {
      queryOverrides.filters = filters.trim();
    }

    return {
      name: 'variant',
      queryOverrides,
    };
  }

  async function handleLaunch() {
    if (mode === 'modeB' && (!variantIndexName || variantIndexName === indexName)) {
      return;
    }

    const payload = {
      name: name.trim(),
      indexName,
      trafficSplit: trafficSplitPercent / 100,
      control: { name: 'control' },
      variant: buildVariantPayload(),
      primaryMetric,
      minimumDays,
    };

    try {
      await createExperiment.mutateAsync(payload);
      onOpenChange(false);
    } catch {
      // Mutation hook handles toast + error state.
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl" data-testid="create-experiment-dialog">
        <DialogHeader>
          <DialogTitle>Create Experiment</DialogTitle>
          <DialogDescription>
            Step {step} of 4
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          {step === 1 && (
            <div className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="experiment-name">Experiment name</Label>
                <Input
                  id="experiment-name"
                  data-testid="experiment-name-input"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="Checkout ranking test"
                />
              </div>

              <div className="space-y-2">
                <Label htmlFor="experiment-index">Index</Label>
                <select
                  id="experiment-index"
                  className="h-10 w-full rounded-md border px-3 text-sm bg-background"
                  data-testid="experiment-index-select"
                  value={indexName}
                  onChange={(e) => setIndexName(e.target.value)}
                >
                  <option value="">Select index</option>
                  {(indexes || []).map((index) => (
                    <option key={index.uid} value={index.uid}>
                      {index.uid}
                    </option>
                  ))}
                </select>
              </div>

              <div className="space-y-2">
                <Label>Primary metric</Label>
                <div className="grid grid-cols-1 gap-2">
                  {METRIC_OPTIONS.map((metric) => (
                    <label
                      key={metric.value}
                      className="flex items-center gap-2 rounded-md border p-2"
                    >
                      <input
                        type="radio"
                        name="primaryMetric"
                        data-testid={metric.testId}
                        checked={primaryMetric === metric.value}
                        onChange={() => setPrimaryMetric(metric.value)}
                      />
                      <span>{metric.label}</span>
                    </label>
                  ))}
                </div>
                <p className="text-sm text-muted-foreground" data-testid="metric-description">
                  {selectedMetric?.description}
                </p>
              </div>
            </div>
          )}

          {step === 2 && (
            <div className="space-y-4">
              <div className="space-y-2">
                <Label>Experiment mode</Label>
                <div className="grid grid-cols-1 gap-2 md:grid-cols-2">
                  <label className="flex items-center gap-2 rounded-md border p-2">
                    <input
                      type="radio"
                      name="mode"
                      data-testid="mode-a-option"
                      checked={mode === 'modeA'}
                      onChange={() => setMode('modeA')}
                    />
                    <span>Mode A (query overrides)</span>
                  </label>
                  <label className="flex items-center gap-2 rounded-md border p-2">
                    <input
                      type="radio"
                      name="mode"
                      data-testid="mode-b-option"
                      checked={mode === 'modeB'}
                      onChange={() => setMode('modeB')}
                    />
                    <span>Mode B (variant index)</span>
                  </label>
                </div>
              </div>

              {mode === 'modeA' ? (
                <div className="space-y-3 rounded-md border p-3">
                  <Label className="text-sm">Variant query overrides</Label>
                  <label className="flex items-center gap-2">
                    <input
                      type="checkbox"
                      data-testid="override-enable-synonyms"
                      checked={enableSynonyms}
                      onChange={(e) => setEnableSynonyms(e.target.checked)}
                    />
                    <span>Enable synonyms</span>
                  </label>
                  <label className="flex items-center gap-2">
                    <input
                      type="checkbox"
                      data-testid="override-enable-rules"
                      checked={enableRules}
                      onChange={(e) => setEnableRules(e.target.checked)}
                    />
                    <span>Enable rules</span>
                  </label>
                  <div className="space-y-1">
                    <Label htmlFor="override-filters">Filters</Label>
                    <Input
                      id="override-filters"
                      data-testid="override-filters-input"
                      value={filters}
                      onChange={(e) => setFilters(e.target.value)}
                      placeholder="brand:Nike"
                    />
                  </div>
                </div>
              ) : (
                <div className="space-y-1 rounded-md border p-3">
                  <Label htmlFor="variant-index">Variant index</Label>
                  <select
                    id="variant-index"
                    className="h-10 w-full rounded-md border px-3 text-sm bg-background"
                    data-testid="variant-index-select"
                    value={variantIndexName}
                    onChange={(e) => setVariantIndexName(e.target.value)}
                  >
                    <option value="">Select variant index</option>
                    {(indexes || [])
                      .filter((index) => index.uid !== indexName)
                      .map((index) => (
                        <option key={index.uid} value={index.uid}>
                          {index.uid}
                        </option>
                      ))}
                  </select>
                  {variantIndexName === indexName && variantIndexName.length > 0 && (
                    <p className="text-xs text-red-700">
                      Variant index must be different from the selected index.
                    </p>
                  )}
                </div>
              )}
            </div>
          )}

          {step === 3 && (
            <div className="space-y-4">
              <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
                <div className="space-y-1">
                  <Label htmlFor="traffic-split">Variant traffic split (%)</Label>
                  <Input
                    id="traffic-split"
                    type="number"
                    min={1}
                    max={99}
                    data-testid="traffic-split-percent-input"
                    value={trafficSplitPercent}
                    onChange={(e) =>
                      setTrafficSplitPercent(
                        Math.max(1, Math.min(99, Number(e.target.value) || 1))
                      )
                    }
                  />
                </div>

                <div className="space-y-1">
                  <Label htmlFor="minimum-days">Minimum runtime (days)</Label>
                  <Input
                    id="minimum-days"
                    type="number"
                    min={1}
                    data-testid="minimum-days-input"
                    value={minimumDays}
                    onChange={(e) => setMinimumDays(Math.max(1, Number(e.target.value) || 1))}
                  />
                </div>
              </div>

              <div className="rounded-md border p-3">
                <p className="mb-2 text-sm font-medium">
                  Runtime estimate (at 2,400 searches/day)
                </p>
                <table className="w-full text-sm">
                  <tbody>
                    {estimatedRows.map((row) => (
                      <tr key={row.label} className="border-b last:border-0">
                        <td className="py-2">{row.label}</td>
                        <td className="py-2 text-right">~{row.estimatedDays} days</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>

              {showRuntimeWarning && (
                <div
                  className={`rounded-md border p-3 text-sm ${
                    showRuntimeDanger
                      ? 'border-red-200 bg-red-50 text-red-900'
                      : 'border-amber-200 bg-amber-50 text-amber-900'
                  }`}
                  data-testid="runtime-warning"
                >
                  {showRuntimeDanger
                    ? 'Estimated runtime is more than 365 days. Consider a larger MDE or more traffic.'
                    : 'Estimated runtime is more than 90 days for a 5% gain. Consider increasing traffic split.'}
                </div>
              )}

              <div
                className="rounded-md border border-sky-200 bg-sky-50 p-3 text-sm text-sky-900"
                data-testid="user-token-warning"
              >
                Valid results require a stable userToken. Pass an authenticated user ID or
                server-side UUID, not a browser cookie.
              </div>
            </div>
          )}

          {step === 4 && (
            <div className="space-y-2 text-sm">
              <div>
                <span className="font-medium">Name: </span>
                <span data-testid="review-name">{name}</span>
              </div>
              <div>
                <span className="font-medium">Index: </span>
                <span data-testid="review-index">{indexName}</span>
              </div>
              <div>
                <span className="font-medium">Mode: </span>
                <span data-testid="review-mode">{mode === 'modeA' ? 'Mode A' : 'Mode B'}</span>
              </div>
              {mode === 'modeB' && (
                <div>
                  <span className="font-medium">Variant index: </span>
                  <span data-testid="review-variant-index">{variantIndexName}</span>
                </div>
              )}
              <div>
                <span className="font-medium">Metric: </span>
                <span>{selectedMetric?.label}</span>
              </div>
              <div>
                <span className="font-medium">Traffic split: </span>
                <span>{trafficSplitPercent}%</span>
              </div>
            </div>
          )}
        </div>

        <DialogFooter className="flex items-center justify-between">
          <Button
            variant="outline"
            onClick={() => {
              if (step > 1) {
                setStep((s) => s - 1);
              } else {
                onOpenChange(false);
              }
            }}
            disabled={createExperiment.isPending}
          >
            {step > 1 ? 'Back' : 'Cancel'}
          </Button>

          {step < 4 ? (
            <Button onClick={() => setStep((s) => s + 1)} disabled={!canProceed()}>
              Next
            </Button>
          ) : (
            <Button onClick={handleLaunch} disabled={createExperiment.isPending}>
              Launch
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
