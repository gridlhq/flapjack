import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { ExperimentDetail } from './ExperimentDetail';

const mockMutateAsync = vi.fn();
const mockPromoteMutateAsync = vi.fn();

vi.mock('@/hooks/useExperiments', () => ({
  useExperiments: vi.fn(),
  useStopExperiment: vi.fn(),
  useDeleteExperiment: vi.fn(),
  useCreateExperiment: vi.fn(),
  useExperimentResults: vi.fn(),
  useExperiment: vi.fn(),
  useConcludeExperiment: vi.fn(() => ({
    mutateAsync: mockMutateAsync,
    isPending: false,
  })),
}));

vi.mock('@/hooks/useSettings', () => ({
  useUpdateSettings: vi.fn(() => ({
    mutateAsync: mockPromoteMutateAsync,
    isPending: false,
  })),
}));

import { useExperimentResults, useExperiment } from '@/hooks/useExperiments';

function renderWithRoute(experimentId: string) {
  return render(
    <MemoryRouter initialEntries={[`/experiments/${experimentId}`]}>
      <Routes>
        <Route path="/experiments/:experimentId" element={<ExperimentDetail />} />
      </Routes>
    </MemoryRouter>
  );
}

function mockExperiment(overrides: Record<string, unknown> = {}) {
  const base = {
    id: 'exp-1',
    name: 'Ranking test',
    indexName: 'products',
    status: 'running',
    trafficSplit: 0.5,
    control: { name: 'control' },
    variant: { name: 'variant' },
    primaryMetric: 'ctr',
    createdAt: 1706745600000,
    startedAt: 1706745600000,
    minimumDays: 14,
    ...overrides,
  };

  vi.mocked(useExperiment).mockReturnValue({
    data: base,
    isLoading: false,
    error: null,
  } as any);
}

function mockResults(overrides: Record<string, unknown> = {}) {
  const base = {
    experimentID: 'exp-1',
    name: 'Ranking test',
    status: 'running',
    indexName: 'products',
    startDate: '2026-02-01T00:00:00Z',
    trafficSplit: 0.5,
    primaryMetric: 'ctr',
    gate: {
      minimumNReached: false,
      minimumDaysReached: false,
      readyToRead: false,
      requiredSearchesPerArm: 60000,
      currentSearchesPerArm: 41200,
      progressPct: 68.7,
      estimatedDaysRemaining: 12.3,
    },
    control: {
      name: 'control',
      searches: 41200,
      users: 8500,
      clicks: 5068,
      conversions: 1854,
      revenue: 45200.0,
      ctr: 0.123,
      conversionRate: 0.045,
      revenuePerSearch: 1.10,
      zeroResultRate: 0.032,
      abandonmentRate: 0.15,
      meanClickRank: 3.5,
    },
    variant: {
      name: 'variant',
      searches: 41200,
      users: 8400,
      clicks: 5397,
      conversions: 2142,
      revenue: 49800.0,
      ctr: 0.131,
      conversionRate: 0.052,
      revenuePerSearch: 1.21,
      zeroResultRate: 0.028,
      abandonmentRate: 0.12,
      meanClickRank: 2.1,
    },
    significance: null,
    bayesian: { probVariantBetter: 0.78 },
    sampleRatioMismatch: false,
    cupedApplied: false,
    guardRailAlerts: [],
    outlierUsersExcluded: 0,
    noStableIdQueries: 0,
    recommendation: null,
    ...overrides,
  };

  vi.mocked(useExperimentResults).mockReturnValue({
    data: base,
    isLoading: false,
    error: null,
  } as any);
}

describe('ExperimentDetail', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Default experiment mock — tests that need specific variant config call mockExperiment()
    vi.mocked(useExperiment).mockReturnValue({
      data: {
        id: 'exp-1',
        name: 'Ranking test',
        indexName: 'products',
        status: 'running',
        trafficSplit: 0.5,
        control: { name: 'control' },
        variant: { name: 'variant' },
        primaryMetric: 'ctr',
        createdAt: 1706745600000,
        startedAt: 1706745600000,
        minimumDays: 14,
      },
      isLoading: false,
      error: null,
    } as any);
  });

  it('shows progress bar with correct fraction when under required N', () => {
    mockResults();
    renderWithRoute('exp-1');

    const progressBar = screen.getByTestId('progress-bar');
    expect(progressBar).toHaveTextContent(/41,200/);
    expect(progressBar).toHaveTextContent(/60,000/);
    expect(progressBar).toHaveTextContent(/68\.7%/);
    // Significance must be absent (gate locked)
    expect(screen.queryByTestId('significance-section')).not.toBeInTheDocument();
  });

  it('hides declare winner button when gate not ready', () => {
    mockResults();
    renderWithRoute('exp-1');

    const btn = screen.queryByRole('button', { name: /declare winner/i });
    expect(btn).toBeNull();
  });

  it('shows significance section when gate is open', () => {
    mockResults({
      gate: {
        minimumNReached: true,
        minimumDaysReached: true,
        readyToRead: true,
        requiredSearchesPerArm: 60000,
        currentSearchesPerArm: 62000,
        progressPct: 100,
        estimatedDaysRemaining: null,
      },
      significance: {
        zScore: 2.15,
        pValue: 0.016,
        confidence: 0.984,
        significant: true,
        relativeImprovement: 0.065,
        winner: 'variant',
      },
    });
    renderWithRoute('exp-1');

    expect(screen.getByTestId('significance-section')).toBeInTheDocument();
    expect(screen.getByText(/98\.4%/)).toBeInTheDocument();
  });

  it('always shows bayesian probability even when gate locked', () => {
    mockResults();
    renderWithRoute('exp-1');

    expect(screen.getByTestId('bayesian-card')).toBeInTheDocument();
    expect(screen.getByText(/78%/)).toBeInTheDocument();
  });

  it('shows SRM warning banner when srm detected', () => {
    mockResults({ sampleRatioMismatch: true });
    renderWithRoute('exp-1');

    expect(screen.getByTestId('srm-banner')).toBeInTheDocument();
    expect(screen.getByText(/traffic split mismatch/i)).toBeInTheDocument();
  });

  it('does not show SRM banner when srm is false', () => {
    mockResults({ sampleRatioMismatch: false });
    renderWithRoute('exp-1');

    expect(screen.queryByTestId('srm-banner')).not.toBeInTheDocument();
  });

  it('shows guard rail alert banner when guardRailAlerts is non-empty', () => {
    mockResults({
      guardRailAlerts: [
        {
          metricName: 'ctr',
          controlValue: 0.12,
          variantValue: 0.09,
          dropPct: 25.0,
        },
      ],
    });
    renderWithRoute('exp-1');

    const banner = screen.getByTestId('guard-rail-banner');
    expect(banner).toBeInTheDocument();
    expect(banner).toHaveTextContent(/guard rail alert/i);
    expect(banner).toHaveTextContent(/ctr/i);
    expect(banner).toHaveTextContent(/25\.0%/);
  });

  it('does not show guard rail banner when guardRailAlerts is empty', () => {
    mockResults({ guardRailAlerts: [] });
    renderWithRoute('exp-1');

    expect(screen.queryByTestId('guard-rail-banner')).not.toBeInTheDocument();
  });

  it('renders safely when guardRailAlerts is missing', () => {
    mockResults({ guardRailAlerts: undefined });
    renderWithRoute('exp-1');

    expect(screen.getByTestId('experiment-detail-name')).toHaveTextContent('Ranking test');
    expect(screen.queryByTestId('guard-rail-banner')).not.toBeInTheDocument();
  });

  it('shows CUPED badge when cupedApplied is true', () => {
    mockResults({
      gate: {
        minimumNReached: true,
        minimumDaysReached: true,
        readyToRead: true,
        requiredSearchesPerArm: 60000,
        currentSearchesPerArm: 62000,
        progressPct: 100,
        estimatedDaysRemaining: null,
      },
      significance: {
        zScore: 2.15,
        pValue: 0.016,
        confidence: 0.984,
        significant: true,
        relativeImprovement: 0.065,
        winner: 'variant',
      },
      cupedApplied: true,
    });
    renderWithRoute('exp-1');

    expect(screen.getByTestId('cuped-badge')).toBeInTheDocument();
    expect(screen.getByTestId('cuped-badge')).toHaveTextContent('CUPED');
  });

  it('does not show CUPED badge when cupedApplied is false', () => {
    mockResults({
      gate: {
        minimumNReached: true,
        minimumDaysReached: true,
        readyToRead: true,
        requiredSearchesPerArm: 60000,
        currentSearchesPerArm: 62000,
        progressPct: 100,
        estimatedDaysRemaining: null,
      },
      significance: {
        zScore: 2.15,
        pValue: 0.016,
        confidence: 0.984,
        significant: true,
        relativeImprovement: 0.065,
        winner: 'variant',
      },
      cupedApplied: false,
    });
    renderWithRoute('exp-1');

    expect(screen.queryByTestId('cuped-badge')).not.toBeInTheDocument();
  });

  it('shows interleaving preference card when interleaving data present', () => {
    mockResults({
      interleaving: {
        deltaAB: -0.214,
        winsControl: 12,
        winsVariant: 20,
        ties: 3,
        pValue: 0.021,
        significant: true,
        totalQueries: 35,
        dataQualityOk: true,
      },
    });
    renderWithRoute('exp-1');

    const card = screen.getByTestId('interleaving-card');
    expect(card).toBeInTheDocument();
    expect(card).toHaveTextContent(/interleaving preference/i);
    expect(card).toHaveTextContent(/-0\.214/);
    expect(card).toHaveTextContent(/control wins/i);
    expect(card).toHaveTextContent(/variant wins/i);
    expect(card).toHaveTextContent(/ties/i);
    expect(screen.getByText(/^Significant$/i)).toBeInTheDocument();
    expect(card).not.toHaveTextContent(/not significant/i);
  });

  it('does not show interleaving card for standard experiments', () => {
    mockResults({ interleaving: null });
    renderWithRoute('exp-1');

    expect(screen.queryByTestId('interleaving-card')).not.toBeInTheDocument();
  });

  it('shows data quality warning when first-team distribution is skewed', () => {
    mockResults({
      interleaving: {
        deltaAB: 0.140,
        winsControl: 18,
        winsVariant: 14,
        ties: 2,
        pValue: 0.13,
        significant: false,
        totalQueries: 34,
        dataQualityOk: false,
      },
    });
    renderWithRoute('exp-1');

    const card = screen.getByTestId('interleaving-card');
    expect(card).toBeInTheDocument();
    expect(screen.getByText(/^Not significant$/i)).toBeInTheDocument();
    expect(screen.getByTestId('interleaving-data-quality-warning')).toBeInTheDocument();
  });

  it('interleaving card shows correct preference direction', () => {
    mockResults({
      interleaving: {
        deltaAB: 0.250,
        winsControl: 25,
        winsVariant: 10,
        ties: 5,
        pValue: 0.004,
        significant: true,
        totalQueries: 40,
        dataQualityOk: true,
      },
    });
    const firstRender = renderWithRoute('exp-1');
    expect(screen.getByTestId('interleaving-card')).toHaveTextContent(/control preferred/i);
    firstRender.unmount();

    mockResults({
      interleaving: {
        deltaAB: -0.250,
        winsControl: 10,
        winsVariant: 25,
        ties: 5,
        pValue: 0.004,
        significant: true,
        totalQueries: 40,
        dataQualityOk: true,
      },
    });
    renderWithRoute('exp-1');
    expect(screen.getByTestId('interleaving-card')).toHaveTextContent(/variant preferred/i);
  });

  it('shows outlier exclusion notice when outlierUsersExcluded > 0', () => {
    mockResults({ outlierUsersExcluded: 12 });
    renderWithRoute('exp-1');

    expect(screen.getByText(/12 users excluded/i)).toBeInTheDocument();
  });

  it('shows unstable ID notice when fraction exceeds 5%', () => {
    mockResults({
      noStableIdQueries: 7000,
      control: {
        name: 'control',
        searches: 41200,
        users: 8500,
        clicks: 5068,
        conversions: 1854,
        revenue: 45200.0,
        ctr: 0.123,
        conversionRate: 0.045,
        revenuePerSearch: 1.10,
        zeroResultRate: 0.032,
        abandonmentRate: 0.15,
        meanClickRank: 3.5,
      },
      variant: {
        name: 'variant',
        searches: 41200,
        users: 8400,
        clicks: 5397,
        conversions: 2142,
        revenue: 49800.0,
        ctr: 0.131,
        conversionRate: 0.052,
        revenuePerSearch: 1.21,
        zeroResultRate: 0.028,
        abandonmentRate: 0.12,
        meanClickRank: 2.1,
      },
    });
    renderWithRoute('exp-1');

    expect(screen.getByText(/unstable IDs/i)).toBeInTheDocument();
  });

  it('metric cards show correct values for control and variant', () => {
    mockResults();
    renderWithRoute('exp-1');

    const controlCard = screen.getByTestId('metric-card-control');
    const variantCard = screen.getByTestId('metric-card-variant');

    expect(controlCard).toHaveTextContent('12.3%');
    expect(variantCard).toHaveTextContent('13.1%');
    expect(controlCard).toHaveTextContent('4.5%');
    expect(variantCard).toHaveTextContent('5.2%');
  });

  it('shows mean click rank diagnostic card for each arm', () => {
    mockResults({
      control: {
        name: 'control',
        searches: 41200,
        users: 8500,
        clicks: 5068,
        conversions: 1854,
        revenue: 45200.0,
        ctr: 0.123,
        conversionRate: 0.045,
        revenuePerSearch: 1.10,
        zeroResultRate: 0.032,
        abandonmentRate: 0.15,
        meanClickRank: 3.5,
      },
      variant: {
        name: 'variant',
        searches: 41200,
        users: 8400,
        clicks: 5397,
        conversions: 2142,
        revenue: 49800.0,
        ctr: 0.131,
        conversionRate: 0.052,
        revenuePerSearch: 1.21,
        zeroResultRate: 0.028,
        abandonmentRate: 0.12,
        meanClickRank: 2.1,
      },
    });
    renderWithRoute('exp-1');

    const card = screen.getByTestId('mean-click-rank-card');
    expect(card).toBeInTheDocument();
    expect(card).toHaveTextContent(/avg click position/i);
    expect(card).toHaveTextContent(/lower is better/i);
    expect(card).toHaveTextContent('3.50');
    expect(card).toHaveTextContent('2.10');
  });

  it('shows declare winner button when gate is ready', () => {
    mockResults({
      gate: {
        minimumNReached: true,
        minimumDaysReached: true,
        readyToRead: true,
        requiredSearchesPerArm: 60000,
        currentSearchesPerArm: 62000,
        progressPct: 100,
        estimatedDaysRemaining: null,
      },
      significance: {
        zScore: 2.15,
        pValue: 0.016,
        confidence: 0.984,
        significant: true,
        relativeImprovement: 0.065,
        winner: 'variant',
      },
    });
    renderWithRoute('exp-1');

    expect(screen.getByRole('button', { name: /declare winner/i })).toBeInTheDocument();
  });

  it('shows experiment name and status', () => {
    mockResults();
    renderWithRoute('exp-1');

    expect(screen.getByText('Ranking test')).toBeInTheDocument();
    expect(screen.getByText('running')).toBeInTheDocument();
  });

  it('shows estimated days remaining when available', () => {
    mockResults();
    renderWithRoute('exp-1');

    const progressBar = screen.getByTestId('progress-bar');
    expect(progressBar).toHaveTextContent(/12\.3/);
    expect(progressBar).toHaveTextContent(/days remaining/i);
  });

  it('shows loading state', () => {
    vi.mocked(useExperimentResults).mockReturnValue({
      data: undefined,
      isLoading: true,
      error: null,
    } as any);
    renderWithRoute('exp-1');

    expect(screen.getByText(/loading/i)).toBeInTheDocument();
  });

  it('shows recommendation text when available', () => {
    mockResults({
      gate: {
        minimumNReached: true,
        minimumDaysReached: true,
        readyToRead: true,
        requiredSearchesPerArm: 60000,
        currentSearchesPerArm: 62000,
        progressPct: 100,
        estimatedDaysRemaining: null,
      },
      significance: {
        zScore: 2.15,
        pValue: 0.016,
        confidence: 0.984,
        significant: true,
        relativeImprovement: 0.065,
        winner: 'variant',
      },
      recommendation: 'Statistically significant result: variant arm wins on CTR.',
    });
    renderWithRoute('exp-1');

    expect(screen.getByText(/variant arm wins on CTR/)).toBeInTheDocument();
  });

  it('shows conclusion summary card for concluded experiments', () => {
    mockResults({
      status: 'concluded',
      endedAt: '2026-02-22T18:30:00Z',
      conclusion: {
        winner: 'variant',
        reason: 'Variant outperformed control on CTR with sufficient confidence.',
        controlMetric: 0.123,
        variantMetric: 0.131,
        confidence: 0.984,
        significant: true,
        promoted: true,
      },
    });

    renderWithRoute('exp-1');

    const card = screen.getByTestId('conclusion-card');
    expect(card).toHaveTextContent(/declared winner/i);
    expect(card).toHaveTextContent(/variant/i);
    expect(card).toHaveTextContent(/98\.4% confidence/i);
    expect(card).toHaveTextContent(/promoted to base index/i);
    expect(card).toHaveTextContent(/2026-02-22/i);
    expect(card).toHaveTextContent(/outperformed control/i);
  });

  it('shows concluded revenue metric as currency values', () => {
    mockResults({
      status: 'concluded',
      primaryMetric: 'revenuePerSearch',
      endedAt: '2026-02-22T18:30:00Z',
      conclusion: {
        winner: 'variant',
        reason: 'Variant produced higher revenue per search.',
        controlMetric: 1.1,
        variantMetric: 1.21,
        confidence: 0.973,
        significant: true,
        promoted: false,
      },
    });

    renderWithRoute('exp-1');

    const card = screen.getByTestId('conclusion-card');
    expect(card).toHaveTextContent(/revenue \/ search/i);
    expect(card).toHaveTextContent(/\$1\.10/);
    expect(card).toHaveTextContent(/\$1\.21/);
  });

  // --- Declare Winner Dialog tests ---

  function gateReadyOverrides(sigOverrides: Record<string, unknown> = {}) {
    return {
      gate: {
        minimumNReached: true,
        minimumDaysReached: true,
        readyToRead: true,
        requiredSearchesPerArm: 60000,
        currentSearchesPerArm: 62000,
        progressPct: 100,
        estimatedDaysRemaining: null,
      },
      significance: {
        zScore: 2.15,
        pValue: 0.016,
        confidence: 0.984,
        significant: true,
        relativeImprovement: 0.065,
        winner: 'variant',
        ...sigOverrides,
      },
    };
  }

  it('opens declare winner dialog on button click', async () => {
    const user = userEvent.setup();
    mockResults(gateReadyOverrides());
    renderWithRoute('exp-1');

    await user.click(screen.getByRole('button', { name: /declare winner/i }));

    expect(screen.getByTestId('declare-winner-dialog')).toBeInTheDocument();
    expect(screen.getByText(/choose a winner/i)).toBeInTheDocument();
  });

  it('dialog pre-selects winner from significance data', async () => {
    const user = userEvent.setup();
    mockResults(gateReadyOverrides({ winner: 'variant' }));
    renderWithRoute('exp-1');

    await user.click(screen.getByRole('button', { name: /declare winner/i }));

    const variantRadio = screen.getByRole('radio', { name: /variant/i });
    expect(variantRadio).toBeChecked();
  });

  it('dialog allows selecting control as winner', async () => {
    const user = userEvent.setup();
    mockResults(gateReadyOverrides({ winner: 'variant' }));
    renderWithRoute('exp-1');

    await user.click(screen.getByRole('button', { name: /declare winner/i }));
    await user.click(screen.getByRole('radio', { name: /control/i }));

    expect(screen.getByRole('radio', { name: /control/i })).toBeChecked();
    expect(screen.getByRole('radio', { name: /variant/i })).not.toBeChecked();
  });

  it('dialog allows selecting no winner (inconclusive)', async () => {
    const user = userEvent.setup();
    mockResults(gateReadyOverrides({ winner: 'variant' }));
    renderWithRoute('exp-1');

    await user.click(screen.getByRole('button', { name: /declare winner/i }));
    await user.click(screen.getByRole('radio', { name: /no winner/i }));

    expect(screen.getByRole('radio', { name: /no winner/i })).toBeChecked();
  });

  it('dialog has editable reason field', async () => {
    const user = userEvent.setup();
    mockResults(gateReadyOverrides());
    renderWithRoute('exp-1');

    await user.click(screen.getByRole('button', { name: /declare winner/i }));

    const textarea = screen.getByRole('textbox', { name: /reason/i });
    expect(textarea).toBeInTheDocument();
    await user.clear(textarea);
    await user.type(textarea, 'Custom reason text');
    expect(textarea).toHaveValue('Custom reason text');
  });

  it('dialog shows promote checkbox', async () => {
    const user = userEvent.setup();
    mockResults(gateReadyOverrides());
    mockExperiment({
      variant: {
        name: 'variant',
        queryOverrides: {
          enableSynonyms: false,
        },
      },
    });
    renderWithRoute('exp-1');

    await user.click(screen.getByRole('button', { name: /declare winner/i }));

    expect(screen.getByRole('checkbox', { name: /promote/i })).toBeInTheDocument();
  });

  it('confirm calls conclude with correct payload for variant winner', async () => {
    const user = userEvent.setup();
    mockMutateAsync.mockResolvedValue({});
    mockResults(gateReadyOverrides({ winner: 'variant' }));
    renderWithRoute('exp-1');

    await user.click(screen.getByRole('button', { name: /declare winner/i }));

    // Default pre-selected: variant. Click confirm.
    const dialog = screen.getByTestId('declare-winner-dialog');
    await user.click(within(dialog).getByRole('button', { name: /confirm/i }));

    expect(mockMutateAsync).toHaveBeenCalledWith({
      id: 'exp-1',
      payload: {
        winner: 'variant',
        reason: expect.any(String),
        controlMetric: 0.123,
        variantMetric: 0.131,
        confidence: 0.984,
        significant: true,
        promoted: false,
      },
    });
  });

  it('confirm maps snake_case primary metric values in conclude payload', async () => {
    const user = userEvent.setup();
    mockMutateAsync.mockResolvedValue({});
    mockResults({
      ...gateReadyOverrides({ winner: 'variant' }),
      primaryMetric: 'zero_result_rate',
      control: {
        name: 'control',
        searches: 41200,
        users: 8500,
        clicks: 5068,
        conversions: 1854,
        revenue: 45200.0,
        ctr: 0.123,
        conversionRate: 0.045,
        revenuePerSearch: 1.10,
        zeroResultRate: 0.032,
        abandonmentRate: 0.15,
        meanClickRank: 3.5,
      },
      variant: {
        name: 'variant',
        searches: 41200,
        users: 8400,
        clicks: 5397,
        conversions: 2142,
        revenue: 49800.0,
        ctr: 0.131,
        conversionRate: 0.052,
        revenuePerSearch: 1.21,
        zeroResultRate: 0.028,
        abandonmentRate: 0.12,
        meanClickRank: 2.1,
      },
    });
    renderWithRoute('exp-1');

    await user.click(screen.getByRole('button', { name: /declare winner/i }));
    const dialog = screen.getByTestId('declare-winner-dialog');
    await user.click(within(dialog).getByRole('button', { name: /confirm/i }));

    expect(mockMutateAsync).toHaveBeenCalledWith({
      id: 'exp-1',
      payload: expect.objectContaining({
        controlMetric: 0.032,
        variantMetric: 0.028,
      }),
    });
  });

  it('confirm calls conclude with null winner for inconclusive', async () => {
    const user = userEvent.setup();
    mockMutateAsync.mockResolvedValue({});
    mockResults(gateReadyOverrides({ winner: null, significant: false, confidence: 0.62 }));
    renderWithRoute('exp-1');

    await user.click(screen.getByRole('button', { name: /declare winner/i }));
    // Pre-selected should be "No Winner" when significance.winner is null
    const dialog = screen.getByTestId('declare-winner-dialog');
    await user.click(within(dialog).getByRole('button', { name: /confirm/i }));

    expect(mockMutateAsync).toHaveBeenCalledWith({
      id: 'exp-1',
      payload: expect.objectContaining({
        winner: null,
        confidence: 0.62,
        significant: false,
      }),
    });
  });

  it('confirm with promote checked sets promoted true', async () => {
    const user = userEvent.setup();
    mockMutateAsync.mockResolvedValue({});
    mockResults(gateReadyOverrides());
    mockExperiment({
      variant: {
        name: 'variant',
        queryOverrides: {
          enableSynonyms: false,
        },
      },
    });
    renderWithRoute('exp-1');

    await user.click(screen.getByRole('button', { name: /declare winner/i }));
    await user.click(screen.getByRole('checkbox', { name: /promote/i }));

    const dialog = screen.getByTestId('declare-winner-dialog');
    await user.click(within(dialog).getByRole('button', { name: /confirm/i }));

    expect(mockMutateAsync).toHaveBeenCalledWith({
      id: 'exp-1',
      payload: expect.objectContaining({
        promoted: true,
      }),
    });
  });

  it('hides declare winner button for concluded experiments', () => {
    mockResults({
      ...gateReadyOverrides(),
      status: 'concluded',
    });
    renderWithRoute('exp-1');

    expect(screen.queryByRole('button', { name: /declare winner/i })).not.toBeInTheDocument();
  });

  it('shows declare winner button for stopped experiments when gate is ready', () => {
    mockResults({
      ...gateReadyOverrides(),
      status: 'stopped',
    });
    renderWithRoute('exp-1');

    // Stopped experiments can still be concluded — backend accepts Running or Stopped
    expect(screen.getByRole('button', { name: /declare winner/i })).toBeInTheDocument();
  });

  // --- 14-day soft gate tests ---

  function nReachedDaysNotReached(sigOverrides: Record<string, unknown> = {}) {
    return {
      gate: {
        minimumNReached: true,
        minimumDaysReached: false,
        readyToRead: false,
        requiredSearchesPerArm: 60000,
        currentSearchesPerArm: 62000,
        progressPct: 100,
        estimatedDaysRemaining: 11.0,
      },
      significance: {
        zScore: 2.15,
        pValue: 0.016,
        confidence: 0.984,
        significant: true,
        relativeImprovement: 0.065,
        winner: 'variant',
        ...sigOverrides,
      },
    };
  }

  it('shows declare winner button when N reached but days not reached (soft gate)', () => {
    mockResults(nReachedDaysNotReached());
    renderWithRoute('exp-1');

    expect(screen.getByRole('button', { name: /declare winner/i })).toBeInTheDocument();
  });

  it('shows significance section when N reached but days not reached', () => {
    mockResults(nReachedDaysNotReached());
    renderWithRoute('exp-1');

    expect(screen.getByTestId('significance-section')).toBeInTheDocument();
    expect(screen.getByText(/98\.4%/)).toBeInTheDocument();
  });

  it('shows minimum days warning when N reached but days not reached', () => {
    mockResults(nReachedDaysNotReached());
    renderWithRoute('exp-1');

    expect(screen.getByTestId('minimum-days-warning')).toBeInTheDocument();
    expect(screen.getByText(/minimum duration/i)).toBeInTheDocument();
  });

  it('clicking declare winner shows novelty warning confirmation when days not reached', async () => {
    const user = userEvent.setup();
    mockResults(nReachedDaysNotReached());
    renderWithRoute('exp-1');

    await user.click(screen.getByRole('button', { name: /declare winner/i }));

    // Should show a novelty warning confirmation before the declare winner dialog
    const confirmation = screen.getByTestId('days-gate-confirmation');
    expect(confirmation).toBeInTheDocument();
    expect(within(confirmation).getByText(/novelty effect/i)).toBeInTheDocument();
  });

  it('confirming novelty warning opens declare winner dialog', async () => {
    const user = userEvent.setup();
    mockResults(nReachedDaysNotReached());
    renderWithRoute('exp-1');

    await user.click(screen.getByRole('button', { name: /declare winner/i }));

    // Confirm the warning
    const warning = screen.getByTestId('days-gate-confirmation');
    await user.click(within(warning).getByRole('button', { name: /proceed anyway/i }));

    // Now the declare winner dialog should be shown
    expect(screen.getByTestId('declare-winner-dialog')).toBeInTheDocument();
  });

  it('does not show minimum days warning when both gates are met', () => {
    mockResults(gateReadyOverrides());
    renderWithRoute('exp-1');

    expect(screen.queryByTestId('minimum-days-warning')).not.toBeInTheDocument();
  });

  // --- C4: Settings diff + promote flow tests ---

  it('dialog shows settings diff for Mode A variant overrides', async () => {
    const user = userEvent.setup();
    mockResults(gateReadyOverrides());
    mockExperiment({
      variant: {
        name: 'variant',
        queryOverrides: {
          enableSynonyms: false,
          typoTolerance: false,
        },
      },
    });
    renderWithRoute('exp-1');

    await user.click(screen.getByRole('button', { name: /declare winner/i }));

    const dialog = screen.getByTestId('declare-winner-dialog');
    expect(within(dialog).getByText(/enableSynonyms/)).toBeInTheDocument();
    expect(within(dialog).getByText(/typoTolerance/)).toBeInTheDocument();
  });

  it('dialog shows variant index for Mode B experiment', async () => {
    const user = userEvent.setup();
    mockResults(gateReadyOverrides());
    mockExperiment({
      variant: {
        name: 'variant',
        indexName: 'products_v2',
      },
    });
    renderWithRoute('exp-1');

    await user.click(screen.getByRole('button', { name: /declare winner/i }));

    const dialog = screen.getByTestId('declare-winner-dialog');
    expect(within(dialog).getByText(/products_v2/)).toBeInTheDocument();
  });

  it('mode b experiment does not offer promote checkbox', async () => {
    const user = userEvent.setup();
    mockResults(gateReadyOverrides());
    mockExperiment({
      variant: {
        name: 'variant',
        indexName: 'products_v2',
      },
    });
    renderWithRoute('exp-1');

    await user.click(screen.getByRole('button', { name: /declare winner/i }));

    const dialog = screen.getByTestId('declare-winner-dialog');
    expect(within(dialog).queryByRole('checkbox', { name: /promote/i })).not.toBeInTheDocument();
  });

  it('empty queryOverrides object does not offer promote checkbox', async () => {
    const user = userEvent.setup();
    mockResults(gateReadyOverrides());
    mockExperiment({
      variant: {
        name: 'variant',
        queryOverrides: {},
      },
    });
    renderWithRoute('exp-1');

    await user.click(screen.getByRole('button', { name: /declare winner/i }));

    const dialog = screen.getByTestId('declare-winner-dialog');
    expect(within(dialog).queryByRole('checkbox', { name: /promote/i })).not.toBeInTheDocument();
  });

  it('empty queryOverrides conclude sends promoted false', async () => {
    const user = userEvent.setup();
    mockMutateAsync.mockResolvedValue({});
    mockResults(gateReadyOverrides());
    mockExperiment({
      variant: {
        name: 'variant',
        queryOverrides: {},
      },
    });
    renderWithRoute('exp-1');

    await user.click(screen.getByRole('button', { name: /declare winner/i }));

    const dialog = screen.getByTestId('declare-winner-dialog');
    await user.click(within(dialog).getByRole('button', { name: /confirm/i }));

    expect(mockMutateAsync).toHaveBeenCalledWith(
      expect.objectContaining({
        payload: expect.objectContaining({ promoted: false }),
      })
    );
    expect(mockPromoteMutateAsync).not.toHaveBeenCalled();
  });

  it('promote calls settings API with variant overrides then concludes', async () => {
    const user = userEvent.setup();
    mockMutateAsync.mockResolvedValue({});
    mockPromoteMutateAsync.mockResolvedValue({});
    mockResults(gateReadyOverrides());
    mockExperiment({
      variant: {
        name: 'variant',
        queryOverrides: {
          enableSynonyms: false,
        },
      },
    });
    renderWithRoute('exp-1');

    await user.click(screen.getByRole('button', { name: /declare winner/i }));
    await user.click(screen.getByRole('checkbox', { name: /promote/i }));

    const dialog = screen.getByTestId('declare-winner-dialog');
    await user.click(within(dialog).getByRole('button', { name: /confirm/i }));

    // Settings API should be called with the variant overrides
    expect(mockPromoteMutateAsync).toHaveBeenCalledWith(
      expect.objectContaining({ enableSynonyms: false })
    );
    // Conclude should also be called
    expect(mockMutateAsync).toHaveBeenCalledWith(
      expect.objectContaining({
        payload: expect.objectContaining({ promoted: true }),
      })
    );
  });

  it('conclude without promote does not call settings API', async () => {
    const user = userEvent.setup();
    mockMutateAsync.mockResolvedValue({});
    mockResults(gateReadyOverrides());
    mockExperiment({
      variant: {
        name: 'variant',
        queryOverrides: {
          enableSynonyms: false,
        },
      },
    });
    renderWithRoute('exp-1');

    await user.click(screen.getByRole('button', { name: /declare winner/i }));
    // Do NOT check the promote checkbox

    const dialog = screen.getByTestId('declare-winner-dialog');
    await user.click(within(dialog).getByRole('button', { name: /confirm/i }));

    // Settings API should NOT be called
    expect(mockPromoteMutateAsync).not.toHaveBeenCalled();
    // Conclude should be called with promoted: false
    expect(mockMutateAsync).toHaveBeenCalledWith(
      expect.objectContaining({
        payload: expect.objectContaining({ promoted: false }),
      })
    );
  });

  it('dialog shows no diff section when variant has no overrides', async () => {
    const user = userEvent.setup();
    mockResults(gateReadyOverrides());
    mockExperiment({
      variant: { name: 'variant' },
    });
    renderWithRoute('exp-1');

    await user.click(screen.getByRole('button', { name: /declare winner/i }));

    const dialog = screen.getByTestId('declare-winner-dialog');
    expect(within(dialog).queryByTestId('settings-diff')).not.toBeInTheDocument();
  });
});
