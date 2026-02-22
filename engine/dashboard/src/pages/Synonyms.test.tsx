import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { Synonyms } from './Synonyms'

vi.mock('@/hooks/useSynonyms', () => ({
  useSynonyms: vi.fn(),
  useSaveSynonym: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useDeleteSynonym: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useClearSynonyms: () => ({ mutateAsync: vi.fn(), isPending: false }),
}))

import { useSynonyms } from '@/hooks/useSynonyms'

function wrapper({ children }: { children: React.ReactNode }) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return (
    <QueryClientProvider client={qc}>
      <MemoryRouter initialEntries={['/index/products/synonyms']}>
        <Routes>
          <Route path="/index/:indexName/synonyms" element={children} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>
  )
}

const MULTI_WAY_SYN = {
  type: 'synonym' as const,
  objectID: 'syn-1',
  synonyms: ['laptop', 'notebook', 'computer'],
}

const ONE_WAY_SYN = {
  type: 'onewaysynonym' as const,
  objectID: 'syn-2',
  input: 'phone',
  synonyms: ['smartphone', 'mobile'],
}

describe('Synonyms', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('shows loading skeleton while fetching', () => {
    vi.mocked(useSynonyms).mockReturnValue({ data: undefined, isLoading: true } as any)
    render(<Synonyms />, { wrapper })
    expect(screen.queryByTestId('synonyms-list')).not.toBeInTheDocument()
  })

  it('shows empty state when no synonyms exist', () => {
    vi.mocked(useSynonyms).mockReturnValue({
      data: { hits: [], nbHits: 0 },
      isLoading: false,
    } as any)
    render(<Synonyms />, { wrapper })
    expect(screen.getByText(/no synonyms/i)).toBeInTheDocument()
  })

  it('renders synonym rows when synonyms exist', () => {
    vi.mocked(useSynonyms).mockReturnValue({
      data: { hits: [MULTI_WAY_SYN], nbHits: 1 },
      isLoading: false,
    } as any)
    render(<Synonyms />, { wrapper })
    expect(screen.getByText(/laptop/i)).toBeInTheDocument()
  })

  it('displays multi-way synonym as equals chain', () => {
    vi.mocked(useSynonyms).mockReturnValue({
      data: { hits: [MULTI_WAY_SYN], nbHits: 1 },
      isLoading: false,
    } as any)
    render(<Synonyms />, { wrapper })
    // Multi-way renders as "laptop = notebook = computer"
    expect(screen.getByText(/laptop = notebook = computer/)).toBeInTheDocument()
  })

  it('displays one-way synonym with arrow', () => {
    vi.mocked(useSynonyms).mockReturnValue({
      data: { hits: [ONE_WAY_SYN], nbHits: 1 },
      isLoading: false,
    } as any)
    render(<Synonyms />, { wrapper })
    // One-way renders as "phone → smartphone, mobile"
    expect(screen.getByText(/phone → smartphone/)).toBeInTheDocument()
  })

  it('renders multiple synonyms', () => {
    vi.mocked(useSynonyms).mockReturnValue({
      data: { hits: [MULTI_WAY_SYN, ONE_WAY_SYN], nbHits: 2 },
      isLoading: false,
    } as any)
    render(<Synonyms />, { wrapper })
    expect(screen.getByText(/laptop = notebook = computer/)).toBeInTheDocument()
    expect(screen.getByText(/phone → smartphone/)).toBeInTheDocument()
  })

  it('shows Add Synonym button', () => {
    vi.mocked(useSynonyms).mockReturnValue({
      data: { hits: [], nbHits: 0 },
      isLoading: false,
    } as any)
    render(<Synonyms />, { wrapper })
    expect(screen.getByRole('button', { name: /add synonym/i })).toBeInTheDocument()
  })

  it('opens create dialog when Add Synonym is clicked', async () => {
    const user = userEvent.setup()
    vi.mocked(useSynonyms).mockReturnValue({
      data: { hits: [], nbHits: 0 },
      isLoading: false,
    } as any)
    render(<Synonyms />, { wrapper })
    await user.click(screen.getByRole('button', { name: /add synonym/i }))
    // Dialog should appear with synonym type selector
    expect(screen.getByRole('dialog')).toBeInTheDocument()
  })

  it('shows type label badges on synonym rows', () => {
    vi.mocked(useSynonyms).mockReturnValue({
      data: { hits: [MULTI_WAY_SYN, ONE_WAY_SYN], nbHits: 2 },
      isLoading: false,
    } as any)
    render(<Synonyms />, { wrapper })
    expect(screen.getByText('Multi-way')).toBeInTheDocument()
    expect(screen.getByText('One-way')).toBeInTheDocument()
  })
})
