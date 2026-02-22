import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { CreateExperimentDialog } from '../CreateExperimentDialog';

vi.mock('@/hooks/useIndexes', () => ({
  useIndexes: vi.fn(),
}));

vi.mock('@/hooks/useExperiments', () => ({
  useCreateExperiment: vi.fn(),
}));

import { useIndexes } from '@/hooks/useIndexes';
import { useCreateExperiment } from '@/hooks/useExperiments';

const mockCreateMutateAsync = vi.fn();

function setupDefaultMocks() {
  vi.mocked(useIndexes).mockReturnValue({
    data: [
      { uid: 'products' },
      { uid: 'products_variant' },
    ],
    isLoading: false,
  } as any);

  vi.mocked(useCreateExperiment).mockReturnValue({
    mutateAsync: mockCreateMutateAsync,
    isPending: false,
  } as any);
}

async function completeStep1(user: ReturnType<typeof userEvent.setup>) {
  await user.type(screen.getByTestId('experiment-name-input'), 'Checkout ranking test');
  await user.selectOptions(screen.getByTestId('experiment-index-select'), 'products');
  await user.click(screen.getByRole('button', { name: /next/i }));
}

async function completeStep2ModeB(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByTestId('mode-b-option'));
  await user.selectOptions(screen.getByTestId('variant-index-select'), 'products_variant');
  await user.click(screen.getByRole('button', { name: /next/i }));
}

describe('CreateExperimentDialog', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setupDefaultMocks();
  });

  it('step 1: selecting CTR metric shows description text', async () => {
    const user = userEvent.setup();
    render(<CreateExperimentDialog open={true} onOpenChange={vi.fn()} />);

    await user.click(screen.getByTestId('metric-conversion-rate'));
    expect(screen.getByText(/tracks conversions per search/i)).toBeInTheDocument();

    await user.click(screen.getByTestId('metric-ctr'));
    expect(screen.getByText(/tracks click-through rate for result relevance/i)).toBeInTheDocument();
  });

  it('step 2: mode A shows query override fields', async () => {
    const user = userEvent.setup();
    render(<CreateExperimentDialog open={true} onOpenChange={vi.fn()} />);

    await completeStep1(user);

    await user.click(screen.getByTestId('mode-a-option'));

    expect(screen.getByTestId('override-enable-synonyms')).toBeInTheDocument();
    expect(screen.getByTestId('override-enable-rules')).toBeInTheDocument();
    expect(screen.getByTestId('override-filters-input')).toBeInTheDocument();
  });

  it('step 2: mode B shows index name dropdown', async () => {
    const user = userEvent.setup();
    render(<CreateExperimentDialog open={true} onOpenChange={vi.fn()} />);

    await completeStep1(user);
    await user.click(screen.getByTestId('mode-b-option'));

    expect(screen.getByTestId('variant-index-select')).toBeInTheDocument();
    expect(screen.queryByTestId('override-filters-input')).not.toBeInTheDocument();
  });

  it('step 3: sample size table shows 4 MDE rows', async () => {
    const user = userEvent.setup();
    render(<CreateExperimentDialog open={true} onOpenChange={vi.fn()} />);

    await completeStep1(user);
    await user.click(screen.getByRole('button', { name: /next/i }));

    expect(screen.getByText(/large gain \(10% relative\)/i)).toBeInTheDocument();
    expect(screen.getByText(/typical early-stage gain \(5%\)/i)).toBeInTheDocument();
    expect(screen.getByText(/small gain \(2%\)/i)).toBeInTheDocument();
    expect(screen.getByText(/mature product \(1%\)/i)).toBeInTheDocument();
  });

  it('step 3: shows warning when estimated runtime > 90 days', async () => {
    const user = userEvent.setup();
    render(<CreateExperimentDialog open={true} onOpenChange={vi.fn()} />);

    await completeStep1(user);
    await user.click(screen.getByRole('button', { name: /next/i }));

    fireEvent.change(screen.getByTestId('traffic-split-percent-input'), {
      target: { value: '10' },
    });

    expect(screen.getByTestId('runtime-warning')).toHaveTextContent(/more than 90 days/i);
  });

  it('step 3: 90% variant split still warns because control arm is bottleneck', async () => {
    const user = userEvent.setup();
    render(<CreateExperimentDialog open={true} onOpenChange={vi.fn()} />);

    await completeStep1(user);
    await user.click(screen.getByRole('button', { name: /next/i }));

    fireEvent.change(screen.getByTestId('traffic-split-percent-input'), {
      target: { value: '90' },
    });

    expect(screen.getByTestId('runtime-warning')).toHaveTextContent(/more than 90 days/i);
  });

  it('step 3: shows user_token stability warning', async () => {
    const user = userEvent.setup();
    render(<CreateExperimentDialog open={true} onOpenChange={vi.fn()} />);

    await completeStep1(user);
    await user.click(screen.getByRole('button', { name: /next/i }));

    expect(screen.getByTestId('user-token-warning')).toHaveTextContent(/stable usertoken/i);
  });

  it('step 4: review shows all selected settings', async () => {
    const user = userEvent.setup();
    render(<CreateExperimentDialog open={true} onOpenChange={vi.fn()} />);

    await completeStep1(user);
    await completeStep2ModeB(user);
    await user.click(screen.getByRole('button', { name: /next/i }));

    expect(screen.getByTestId('review-name')).toHaveTextContent('Checkout ranking test');
    expect(screen.getByTestId('review-index')).toHaveTextContent('products');
    expect(screen.getByTestId('review-mode')).toHaveTextContent(/mode b/i);
    expect(screen.getByTestId('review-variant-index')).toHaveTextContent('products_variant');
  });

  it('mode B blocks progress when variant index matches selected index', async () => {
    const user = userEvent.setup();
    render(<CreateExperimentDialog open={true} onOpenChange={vi.fn()} />);

    await completeStep1(user);
    await user.click(screen.getByTestId('mode-b-option'));
    await user.selectOptions(screen.getByTestId('variant-index-select'), 'products_variant');
    await user.click(screen.getByRole('button', { name: /back/i }));

    await user.selectOptions(screen.getByTestId('experiment-index-select'), 'products_variant');
    await user.click(screen.getByRole('button', { name: /next/i }));

    expect(screen.getByRole('button', { name: /next/i })).toBeDisabled();
  });

  it('launch button calls POST /2/abtests and closes dialog', async () => {
    const user = userEvent.setup();
    const onOpenChange = vi.fn();

    mockCreateMutateAsync.mockResolvedValue({ id: 'exp-new-1' });

    render(<CreateExperimentDialog open={true} onOpenChange={onOpenChange} />);

    await completeStep1(user);
    await completeStep2ModeB(user);
    await user.click(screen.getByRole('button', { name: /next/i }));
    await user.click(screen.getByRole('button', { name: /launch/i }));

    expect(mockCreateMutateAsync).toHaveBeenCalledWith(
      expect.objectContaining({
        name: 'Checkout ranking test',
        indexName: 'products',
        primaryMetric: 'ctr',
        trafficSplit: 0.5,
        control: { name: 'control' },
        variant: expect.objectContaining({
          name: 'variant',
          indexName: 'products_variant',
        }),
      })
    );
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });
});
