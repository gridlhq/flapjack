import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { MerchandisingStudio } from './MerchandisingStudio'

vi.mock('@/hooks/useSearch', () => ({
  useSearch: vi.fn(),
}))

vi.mock('@/hooks/useRules', () => ({
  useRules: vi.fn(),
  useSaveRule: () => ({ mutateAsync: vi.fn().mockResolvedValue({}), isPending: false }),
}))

vi.mock('@/hooks/use-toast', () => ({
  useToast: () => ({ toast: vi.fn() }),
}))

import { useSearch } from '@/hooks/useSearch'
import { useRules } from '@/hooks/useRules'

const HITS = [
  { objectID: 'prod-1', name: 'Apple iPhone 15', brand: 'Apple' },
  { objectID: 'prod-2', name: 'Samsung Galaxy S24', brand: 'Samsung' },
  { objectID: 'prod-3', name: 'Google Pixel 8', brand: 'Google' },
]

function makeWrapper(path: string) {
  return function wrapper({ children }: { children: React.ReactNode }) {
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    return (
      <QueryClientProvider client={qc}>
        <MemoryRouter initialEntries={[path]}>
          <Routes>
            <Route path="/index/:indexName/merchandising" element={children} />
            <Route path="/no-index" element={children} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>
    )
  }
}

const withIndex = makeWrapper('/index/products/merchandising')
const withoutIndex = makeWrapper('/no-index')

describe('MerchandisingStudio', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    vi.mocked(useRules).mockReturnValue({ data: { hits: [], nbHits: 0 }, isLoading: false } as any)
    vi.mocked(useSearch).mockReturnValue({ data: undefined, isLoading: false } as any)
  })

  it('shows no-index state when indexName is missing from route', () => {
    render(<MerchandisingStudio />, { wrapper: withoutIndex })
    expect(screen.getByText('No index selected')).toBeInTheDocument()
  })

  it('renders the Merchandising Studio heading', () => {
    render(<MerchandisingStudio />, { wrapper: withIndex })
    expect(screen.getByRole('heading', { name: /merchandising studio/i })).toBeInTheDocument()
  })

  it('shows the search input and Search button', () => {
    render(<MerchandisingStudio />, { wrapper: withIndex })
    expect(screen.getByTestId('merch-search-input')).toBeInTheDocument()
    expect(screen.getByTestId('merch-search-btn')).toBeInTheDocument()
  })

  it('shows the "enter a search query" prompt before any search is submitted', () => {
    render(<MerchandisingStudio />, { wrapper: withIndex })
    expect(screen.getByText('Enter a search query')).toBeInTheDocument()
  })

  it('shows search results after submitting a query', async () => {
    const user = userEvent.setup()
    vi.mocked(useSearch).mockReturnValue({
      data: { hits: HITS, nbHits: 3, processingTimeMS: 2 },
      isLoading: false,
    } as any)
    render(<MerchandisingStudio />, { wrapper: withIndex })

    await user.type(screen.getByTestId('merch-search-input'), 'phone')
    await user.click(screen.getByTestId('merch-search-btn'))

    expect(screen.getAllByTestId('merch-card')).toHaveLength(3)
  })

  it('shows Pin and Hide buttons on each result card', async () => {
    const user = userEvent.setup()
    vi.mocked(useSearch).mockReturnValue({
      data: { hits: HITS, nbHits: 3, processingTimeMS: 2 },
      isLoading: false,
    } as any)
    render(<MerchandisingStudio />, { wrapper: withIndex })

    await user.type(screen.getByTestId('merch-search-input'), 'phone')
    await user.click(screen.getByTestId('merch-search-btn'))

    // Each card should have a Pin button and a Hide button
    const pinButtons = screen.getAllByTitle(/^pin to this position$/i)
    const hideButtons = screen.getAllByTitle(/hide from results/i)
    expect(pinButtons).toHaveLength(3)
    expect(hideButtons).toHaveLength(3)
  })

  it('shows changes summary badge when a result is pinned', async () => {
    const user = userEvent.setup()
    vi.mocked(useSearch).mockReturnValue({
      data: { hits: HITS, nbHits: 3, processingTimeMS: 2 },
      isLoading: false,
    } as any)
    render(<MerchandisingStudio />, { wrapper: withIndex })

    await user.type(screen.getByTestId('merch-search-input'), 'phone')
    await user.click(screen.getByTestId('merch-search-btn'))

    // Pin the first result
    const pinBtns = screen.getAllByTitle(/^pin to this position$/i)
    await user.click(pinBtns[0])

    // Badge and Save as Rule button should appear
    expect(screen.getByText(/1 pinned/)).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /save as rule/i })).toBeInTheDocument()
  })
})
