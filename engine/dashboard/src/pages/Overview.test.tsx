import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { Overview } from './Overview'

// Mock API module
vi.mock('@/lib/api', () => ({
  default: {
    post: vi.fn(),
    get: vi.fn(),
    delete: vi.fn(),
  },
}))

// Mock hooks
vi.mock('@/hooks/useIndexes', () => ({
  useIndexes: vi.fn(),
  useDeleteIndex: () => ({ mutate: vi.fn(), isPending: false }),
  useCreateIndex: () => ({ mutate: vi.fn(), isPending: false, isSuccess: false, reset: vi.fn() }),
}))

vi.mock('@/hooks/useHealth', () => ({
  useHealth: () => ({ data: { status: 'ok' }, isLoading: false, error: null }),
}))

vi.mock('@/hooks/useAnalytics', () => ({
  useAnalyticsOverview: vi.fn(),
  defaultRange: () => ({ startDate: '2026-02-09', endDate: '2026-02-16' }),
}))

vi.mock('@/hooks/useSnapshots', () => ({
  useExportIndex: () => ({ mutate: vi.fn(), isPending: false }),
  useImportIndex: () => ({ mutate: vi.fn(), isPending: false }),
}))

// Mock recharts to avoid rendering issues in jsdom
vi.mock('recharts', () => ({
  AreaChart: ({ children }: any) => <div data-testid="mock-chart">{children}</div>,
  Area: () => null,
  ResponsiveContainer: ({ children }: any) => <div>{children}</div>,
  XAxis: () => null,
  YAxis: () => null,
  CartesianGrid: () => null,
  Tooltip: () => null,
}))

import api from '@/lib/api'
import { useIndexes } from '@/hooks/useIndexes'
import { useAnalyticsOverview } from '@/hooks/useAnalytics'

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false, gcTime: 0, staleTime: 0 },
      mutations: { retry: false },
    },
  })

  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>{children}</MemoryRouter>
    </QueryClientProvider>
  )
}

describe('Overview Cleanup Button', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('shows cleanup button when analytics card is visible', () => {
    vi.mocked(useIndexes).mockReturnValue({
      data: [{ uid: 'products', entries: 100, dataSize: 1024 }],
      isLoading: false,
      error: null,
    } as any)
    vi.mocked(useAnalyticsOverview).mockReturnValue({
      data: { totalSearches: 50, uniqueUsers: 10, noResultRate: 0.05, dates: [], indices: [] },
      isLoading: false,
    } as any)

    render(<Overview />, { wrapper: createWrapper() })

    expect(screen.getByTestId('overview-cleanup-btn')).toBeInTheDocument()
  })

  it('does not show cleanup button when analytics card is hidden', () => {
    vi.mocked(useIndexes).mockReturnValue({
      data: [{ uid: 'products', entries: 100, dataSize: 1024 }],
      isLoading: false,
      error: null,
    } as any)
    vi.mocked(useAnalyticsOverview).mockReturnValue({
      data: { totalSearches: 0, uniqueUsers: 0, noResultRate: null, dates: [], indices: [] },
      isLoading: false,
    } as any)

    render(<Overview />, { wrapper: createWrapper() })

    expect(screen.queryByTestId('overview-cleanup-btn')).not.toBeInTheDocument()
  })

  it('opens confirmation dialog when cleanup button is clicked', async () => {
    const user = userEvent.setup()
    vi.mocked(useIndexes).mockReturnValue({
      data: [{ uid: 'products', entries: 100, dataSize: 1024 }],
      isLoading: false,
      error: null,
    } as any)
    vi.mocked(useAnalyticsOverview).mockReturnValue({
      data: { totalSearches: 50, uniqueUsers: 10, noResultRate: 0.05, dates: [], indices: [] },
      isLoading: false,
    } as any)

    render(<Overview />, { wrapper: createWrapper() })

    await user.click(screen.getByTestId('overview-cleanup-btn'))

    expect(screen.getByText('Cleanup Analytics')).toBeInTheDocument()
    expect(
      screen.getByText(
        'This will remove analytics data for indexes that no longer exist. Analytics for your active indexes will not be affected.'
      )
    ).toBeInTheDocument()
  })

  it('calls POST /2/analytics/cleanup when confirmed', async () => {
    const user = userEvent.setup()
    vi.mocked(useIndexes).mockReturnValue({
      data: [{ uid: 'products', entries: 100, dataSize: 1024 }],
      isLoading: false,
      error: null,
    } as any)
    vi.mocked(useAnalyticsOverview).mockReturnValue({
      data: { totalSearches: 50, uniqueUsers: 10, noResultRate: 0.05, dates: [], indices: [] },
      isLoading: false,
    } as any)
    vi.mocked(api.post).mockResolvedValueOnce({
      data: { status: 'ok', removedIndices: ['old-index'], removedCount: 1 },
    })

    render(<Overview />, { wrapper: createWrapper() })

    // Open dialog
    await user.click(screen.getByTestId('overview-cleanup-btn'))

    // Confirm
    const confirmButton = screen.getByRole('button', { name: /cleanup/i })
    await user.click(confirmButton)

    await waitFor(() => {
      expect(api.post).toHaveBeenCalledWith('/2/analytics/cleanup')
    })
  })

  it('shows success message after cleanup', async () => {
    const user = userEvent.setup()
    vi.mocked(useIndexes).mockReturnValue({
      data: [{ uid: 'products', entries: 100, dataSize: 1024 }],
      isLoading: false,
      error: null,
    } as any)
    vi.mocked(useAnalyticsOverview).mockReturnValue({
      data: { totalSearches: 50, uniqueUsers: 10, noResultRate: 0.05, dates: [], indices: [] },
      isLoading: false,
    } as any)
    vi.mocked(api.post).mockResolvedValueOnce({
      data: { status: 'ok', removedIndices: [], removedCount: 0 },
    })

    render(<Overview />, { wrapper: createWrapper() })

    await user.click(screen.getByTestId('overview-cleanup-btn'))

    const confirmButton = screen.getByRole('button', { name: /cleanup/i })
    await user.click(confirmButton)

    await waitFor(() => {
      expect(screen.getByText('Cleaned up')).toBeInTheDocument()
    })
  })

  it('handles API error gracefully', async () => {
    const user = userEvent.setup()
    vi.mocked(useIndexes).mockReturnValue({
      data: [{ uid: 'products', entries: 100, dataSize: 1024 }],
      isLoading: false,
      error: null,
    } as any)
    vi.mocked(useAnalyticsOverview).mockReturnValue({
      data: { totalSearches: 50, uniqueUsers: 10, noResultRate: 0.05, dates: [], indices: [] },
      isLoading: false,
    } as any)
    vi.mocked(api.post).mockRejectedValueOnce(new Error('Server error'))

    render(<Overview />, { wrapper: createWrapper() })

    await user.click(screen.getByTestId('overview-cleanup-btn'))

    const confirmButton = screen.getByRole('button', { name: /cleanup/i })
    await user.click(confirmButton)

    // After error, the dialog should close and button should still be available
    await waitFor(() => {
      expect(screen.getByTestId('overview-cleanup-btn')).toBeInTheDocument()
    })
  })
})
