import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { Rules } from './Rules'

vi.mock('@/hooks/useRules', () => ({
  useRules: vi.fn(),
  useSaveRule: () => ({ mutateAsync: vi.fn().mockResolvedValue({}), isPending: false }),
  useDeleteRule: () => ({ mutateAsync: vi.fn().mockResolvedValue({}), isPending: false }),
  useClearRules: () => ({ mutateAsync: vi.fn().mockResolvedValue({}), isPending: false }),
}))

// Monaco editor isn't available in jsdom — mock the lazy-loaded dialog internals
vi.mock('@monaco-editor/react', () => ({
  default: ({ value, onChange }: { value: string; onChange: (v: string) => void }) => (
    <textarea
      data-testid="monaco-editor"
      value={value}
      onChange={(e) => onChange(e.target.value)}
    />
  ),
}))

import { useRules } from '@/hooks/useRules'

function wrapper({ children }: { children: React.ReactNode }) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return (
    <QueryClientProvider client={qc}>
      <MemoryRouter initialEntries={['/index/products/rules']}>
        <Routes>
          <Route path="/index/:indexName/rules" element={children} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>
  )
}

function noIndexWrapper({ children }: { children: React.ReactNode }) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return (
    <QueryClientProvider client={qc}>
      <MemoryRouter initialEntries={['/rules']}>
        <Routes>
          <Route path="/rules" element={children} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>
  )
}

const RULE_ENABLED = {
  objectID: 'boost-apple',
  conditions: [{ pattern: 'apple', anchoring: 'contains' as const }],
  consequence: { promote: [{ objectID: 'prod-1', position: 0 }] },
  description: 'Boost Apple products',
  enabled: true,
}

const RULE_DISABLED = {
  objectID: 'hide-refurb',
  conditions: [],
  consequence: { hide: [{ objectID: 'prod-2' }, { objectID: 'prod-3' }] },
  description: '',
  enabled: false,
}

describe('Rules', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('shows no-index state when indexName is missing from route', () => {
    vi.mocked(useRules).mockReturnValue({ data: undefined, isLoading: false } as any)
    render(<Rules />, { wrapper: noIndexWrapper })
    expect(screen.getByText('No index selected')).toBeInTheDocument()
  })

  it('shows loading state while fetching', () => {
    vi.mocked(useRules).mockReturnValue({ data: undefined, isLoading: true } as any)
    render(<Rules />, { wrapper })
    // No rules list while loading
    expect(screen.queryByTestId('rules-list')).not.toBeInTheDocument()
    // No empty-state message either
    expect(screen.queryByText('No rules')).not.toBeInTheDocument()
  })

  it('shows empty state when there are no rules', () => {
    vi.mocked(useRules).mockReturnValue({ data: { hits: [], nbHits: 0 }, isLoading: false } as any)
    render(<Rules />, { wrapper })
    expect(screen.getByText('No rules')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /create a rule/i })).toBeInTheDocument()
  })

  it('renders rule cards when rules exist', () => {
    vi.mocked(useRules).mockReturnValue({
      data: { hits: [RULE_ENABLED, RULE_DISABLED], nbHits: 2 },
      isLoading: false,
    } as any)
    render(<Rules />, { wrapper })
    expect(screen.getByTestId('rules-list')).toBeInTheDocument()
    expect(screen.getAllByTestId('rule-card')).toHaveLength(2)
    expect(screen.getByText('boost-apple')).toBeInTheDocument()
    expect(screen.getByText('Boost Apple products')).toBeInTheDocument()
  })

  it('shows promote/hide badges on rule cards', () => {
    vi.mocked(useRules).mockReturnValue({
      data: { hits: [RULE_ENABLED, RULE_DISABLED], nbHits: 2 },
      isLoading: false,
    } as any)
    render(<Rules />, { wrapper })
    expect(screen.getByText('1 pinned')).toBeInTheDocument()
    expect(screen.getByText('2 hidden')).toBeInTheDocument()
  })

  it('shows rules count badge in header', () => {
    vi.mocked(useRules).mockReturnValue({
      data: { hits: [RULE_ENABLED], nbHits: 1 },
      isLoading: false,
    } as any)
    render(<Rules />, { wrapper })
    expect(screen.getByTestId('rules-count-badge')).toHaveTextContent('1')
  })

  it('opens rule editor dialog when Add Rule is clicked', async () => {
    const user = userEvent.setup()
    vi.mocked(useRules).mockReturnValue({ data: { hits: [], nbHits: 0 }, isLoading: false } as any)
    render(<Rules />, { wrapper })
    await user.click(screen.getByRole('button', { name: /add rule/i }))
    expect(screen.getByRole('dialog')).toBeInTheDocument()
    expect(screen.getByText('Create Rule')).toBeInTheDocument()
  })
})
