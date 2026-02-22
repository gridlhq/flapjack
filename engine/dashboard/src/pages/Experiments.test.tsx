import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { Experiments } from './Experiments';

vi.mock('@/hooks/useExperiments', () => ({
  useExperiments: vi.fn(),
  useStopExperiment: vi.fn(),
  useDeleteExperiment: vi.fn(),
  useCreateExperiment: vi.fn(),
}));

vi.mock('@/hooks/useIndexes', () => ({
  useIndexes: vi.fn(),
}));

import {
  useExperiments,
  useStopExperiment,
  useDeleteExperiment,
  useCreateExperiment,
} from '@/hooks/useExperiments';
import { useIndexes } from '@/hooks/useIndexes';
import type { Experiment } from '@/lib/types';

function makeExperiment(overrides: Partial<Experiment> = {}): Experiment {
  return {
    id: 'exp-1',
    name: 'Ranking test',
    indexName: 'products',
    status: 'running',
    trafficSplit: 0.5,
    primaryMetric: 'ctr',
    createdAt: Date.now(),
    startedAt: Date.now() - 86400000 * 3,
    minimumDays: 14,
    control: { name: 'control' },
    variant: { name: 'variant' },
    ...overrides,
  };
}

const mockStopMutate = vi.fn();
const mockDeleteMutate = vi.fn();
const mockCreateMutateAsync = vi.fn();

function setupMutationMocks() {
  vi.mocked(useStopExperiment).mockReturnValue({
    mutate: mockStopMutate,
    isPending: false,
  } as any);
  vi.mocked(useDeleteExperiment).mockReturnValue({
    mutate: mockDeleteMutate,
    isPending: false,
  } as any);
  vi.mocked(useCreateExperiment).mockReturnValue({
    mutateAsync: mockCreateMutateAsync,
    isPending: false,
  } as any);
  vi.mocked(useIndexes).mockReturnValue({
    data: [{ uid: 'products' }, { uid: 'products_variant' }],
    isLoading: false,
  } as any);
}

describe('Experiments', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setupMutationMocks();
  });

  it('shows empty state when no experiments', () => {
    vi.mocked(useExperiments).mockReturnValue({
      data: [],
      isLoading: false,
    } as any);

    render(
      <MemoryRouter>
        <Experiments />
      </MemoryRouter>
    );

    expect(screen.getByText('No experiments yet')).toBeInTheDocument();
  });

  it('opens create experiment dialog from header action', async () => {
    const user = userEvent.setup();
    vi.mocked(useExperiments).mockReturnValue({
      data: [],
      isLoading: false,
    } as any);

    render(
      <MemoryRouter>
        <Experiments />
      </MemoryRouter>
    );

    await user.click(screen.getByRole('button', { name: /create experiment/i }));

    expect(screen.getByTestId('create-experiment-dialog')).toBeInTheDocument();
  });

  it('shows experiment name, index, status, and traffic split', () => {
    vi.mocked(useExperiments).mockReturnValue({
      data: [makeExperiment()],
      isLoading: false,
    } as any);

    render(
      <MemoryRouter>
        <Experiments />
      </MemoryRouter>
    );

    expect(screen.getByText('Ranking test')).toBeInTheDocument();
    expect(screen.getByText('products')).toBeInTheDocument();
    expect(screen.getByText('running')).toBeInTheDocument();
    expect(screen.getByText('50%')).toBeInTheDocument();
  });

  it('running badge has green color', () => {
    vi.mocked(useExperiments).mockReturnValue({
      data: [makeExperiment({ id: 'exp-running', status: 'running' })],
      isLoading: false,
    } as any);

    render(
      <MemoryRouter>
        <Experiments />
      </MemoryRouter>
    );

    expect(screen.getByTestId('experiment-status-exp-running').className).toMatch(
      /bg-emerald-100/
    );
  });

  it('draft badge has gray color', () => {
    vi.mocked(useExperiments).mockReturnValue({
      data: [makeExperiment({ id: 'exp-draft', status: 'draft' })],
      isLoading: false,
    } as any);

    render(
      <MemoryRouter>
        <Experiments />
      </MemoryRouter>
    );

    expect(screen.getByTestId('experiment-status-exp-draft').className).toMatch(
      /bg-slate-100/
    );
  });

  it('stopped badge has orange color', () => {
    vi.mocked(useExperiments).mockReturnValue({
      data: [makeExperiment({ id: 'exp-stopped', status: 'stopped' })],
      isLoading: false,
    } as any);

    render(
      <MemoryRouter>
        <Experiments />
      </MemoryRouter>
    );

    expect(screen.getByTestId('experiment-status-exp-stopped').className).toMatch(
      /bg-orange-100/
    );
  });

  it('shows primary metric and started date columns', () => {
    vi.mocked(useExperiments).mockReturnValue({
      data: [makeExperiment({ primaryMetric: 'ctr', startedAt: 1740000000000 })],
      isLoading: false,
    } as any);

    render(
      <MemoryRouter>
        <Experiments />
      </MemoryRouter>
    );

    // Column headers
    expect(screen.getByText('Metric')).toBeInTheDocument();
    expect(screen.getByText('Started')).toBeInTheDocument();

    // Cell values
    const row = screen.getByTestId('experiment-row-exp-1');
    expect(within(row).getByText('CTR')).toBeInTheDocument();
  });

  it('renders conversion metric label for camelCase metric value', () => {
    vi.mocked(useExperiments).mockReturnValue({
      data: [makeExperiment({ primaryMetric: 'conversionRate' })],
      isLoading: false,
    } as any);

    render(
      <MemoryRouter>
        <Experiments />
      </MemoryRouter>
    );

    const row = screen.getByTestId('experiment-row-exp-1');
    expect(within(row).getByText('Conversion')).toBeInTheDocument();
  });

  it('shows dash for started date when experiment has no startedAt', () => {
    vi.mocked(useExperiments).mockReturnValue({
      data: [makeExperiment({ status: 'draft', startedAt: null })],
      isLoading: false,
    } as any);

    render(
      <MemoryRouter>
        <Experiments />
      </MemoryRouter>
    );

    const row = screen.getByTestId('experiment-row-exp-1');
    expect(within(row).getByTestId('experiment-started-exp-1')).toHaveTextContent('—');
  });

  it('stop button triggers confirmation then calls stop mutation', async () => {
    const user = userEvent.setup();
    vi.mocked(useExperiments).mockReturnValue({
      data: [makeExperiment({ id: 'exp-run', status: 'running' })],
      isLoading: false,
    } as any);

    render(
      <MemoryRouter>
        <Experiments />
      </MemoryRouter>
    );

    // Click stop button
    await user.click(screen.getByTestId('stop-experiment-exp-run'));

    // Confirmation dialog should appear
    expect(screen.getByText(/stop this experiment/i)).toBeInTheDocument();

    // Confirm
    await user.click(screen.getByRole('button', { name: /stop/i }));

    expect(mockStopMutate).toHaveBeenCalledWith('exp-run');
  });

  it('delete button shows confirmation then calls delete mutation', async () => {
    const user = userEvent.setup();
    vi.mocked(useExperiments).mockReturnValue({
      data: [makeExperiment({ id: 'exp-stopped', status: 'stopped' })],
      isLoading: false,
    } as any);

    render(
      <MemoryRouter>
        <Experiments />
      </MemoryRouter>
    );

    // Click delete button
    await user.click(screen.getByTestId('delete-experiment-exp-stopped'));

    // Confirmation dialog should appear
    expect(screen.getByText(/delete this experiment/i)).toBeInTheDocument();

    // Confirm
    await user.click(screen.getByRole('button', { name: /delete/i }));

    expect(mockDeleteMutate).toHaveBeenCalledWith('exp-stopped');
  });

  it('delete button disabled for running experiments', () => {
    vi.mocked(useExperiments).mockReturnValue({
      data: [makeExperiment({ id: 'exp-run', status: 'running' })],
      isLoading: false,
    } as any);

    render(
      <MemoryRouter>
        <Experiments />
      </MemoryRouter>
    );

    expect(screen.getByTestId('delete-experiment-exp-run')).toBeDisabled();
  });

  it('stop button hidden for non-running experiments', () => {
    vi.mocked(useExperiments).mockReturnValue({
      data: [makeExperiment({ id: 'exp-stopped', status: 'stopped' })],
      isLoading: false,
    } as any);

    render(
      <MemoryRouter>
        <Experiments />
      </MemoryRouter>
    );

    expect(screen.queryByTestId('stop-experiment-exp-stopped')).not.toBeInTheDocument();
  });
});
