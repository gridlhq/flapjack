import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { SearchBrowse } from './SearchBrowse'

vi.mock('@/hooks/useIndexes', () => ({
  useIndexes: vi.fn(),
}))

// Mock child components — SearchBrowse tests focus on the page shell
vi.mock('@/components/search/SearchBox', () => ({
  SearchBox: () => <div data-testid="search-box" />,
}))
vi.mock('@/components/search/ResultsPanel', () => ({
  ResultsPanel: () => <div data-testid="results-panel" />,
}))
vi.mock('@/components/search/FacetsPanel', () => ({
  FacetsPanel: () => <div data-testid="facets-panel" />,
}))
vi.mock('@/components/documents/AddDocumentsDialog', () => ({
  AddDocumentsDialog: ({ open }: { open: boolean }) =>
    open ? <div data-testid="add-documents-dialog" /> : null,
}))

import { useIndexes } from '@/hooks/useIndexes'

const INDEX = { uid: 'products', entries: 1234, dataSize: 5_000_000 }

function makeWrapper(path: string) {
  return function wrapper({ children }: { children: React.ReactNode }) {
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    return (
      <QueryClientProvider client={qc}>
        <MemoryRouter initialEntries={[path]}>
          <Routes>
            <Route path="/index/:indexName" element={children} />
            <Route path="/no-index" element={children} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>
    )
  }
}

const withIndex = makeWrapper('/index/products')
const withoutIndex = makeWrapper('/no-index')

describe('SearchBrowse', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('shows no-index state when indexName is not in route', () => {
    vi.mocked(useIndexes).mockReturnValue({ data: [INDEX], isLoading: false } as any)
    render(<SearchBrowse />, { wrapper: withoutIndex })
    expect(screen.getByText('No index selected')).toBeInTheDocument()
    expect(screen.getByRole('link', { name: /go to overview/i })).toBeInTheDocument()
  })

  it('renders the index name in the header', () => {
    vi.mocked(useIndexes).mockReturnValue({ data: [INDEX], isLoading: false } as any)
    render(<SearchBrowse />, { wrapper: withIndex })
    expect(screen.getByRole('heading', { name: 'products' })).toBeInTheDocument()
  })

  it('shows index stats (doc count and size) in the header', () => {
    vi.mocked(useIndexes).mockReturnValue({ data: [INDEX], isLoading: false } as any)
    render(<SearchBrowse />, { wrapper: withIndex })
    expect(screen.getByText(/1,234 docs/)).toBeInTheDocument()
  })

  it('renders the SearchBox, ResultsPanel, and FacetsPanel', () => {
    vi.mocked(useIndexes).mockReturnValue({ data: [INDEX], isLoading: false } as any)
    render(<SearchBrowse />, { wrapper: withIndex })
    expect(screen.getByTestId('search-box')).toBeInTheDocument()
    expect(screen.getByTestId('results-panel')).toBeInTheDocument()
    expect(screen.getByTestId('facets-panel')).toBeInTheDocument()
  })

  it('opens the Add Documents dialog when the button is clicked', async () => {
    const user = userEvent.setup()
    vi.mocked(useIndexes).mockReturnValue({ data: [INDEX], isLoading: false } as any)
    render(<SearchBrowse />, { wrapper: withIndex })

    expect(screen.queryByTestId('add-documents-dialog')).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: /add documents/i }))
    expect(screen.getByTestId('add-documents-dialog')).toBeInTheDocument()
  })

  it('shows the Track Analytics toggle', () => {
    vi.mocked(useIndexes).mockReturnValue({ data: [INDEX], isLoading: false } as any)
    render(<SearchBrowse />, { wrapper: withIndex })
    expect(screen.getByLabelText(/track analytics/i)).toBeInTheDocument()
  })

  it('shows nav links to Settings, Analytics, Synonyms, and Merchandising', () => {
    vi.mocked(useIndexes).mockReturnValue({ data: [INDEX], isLoading: false } as any)
    render(<SearchBrowse />, { wrapper: withIndex })
    expect(screen.getByRole('link', { name: /settings/i })).toBeInTheDocument()
    expect(screen.getByRole('link', { name: /analytics/i })).toBeInTheDocument()
    expect(screen.getByRole('link', { name: /synonyms/i })).toBeInTheDocument()
    expect(screen.getByRole('link', { name: /merchandising/i })).toBeInTheDocument()
  })
})
