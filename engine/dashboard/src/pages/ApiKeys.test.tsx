import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { ApiKeys } from './ApiKeys'

vi.mock('@/hooks/useApiKeys', () => ({
  useApiKeys: vi.fn(),
  useDeleteApiKey: () => ({ mutateAsync: vi.fn(), isPending: false }),
}))

vi.mock('@/hooks/useIndexes', () => ({
  useIndexes: vi.fn(),
}))

vi.mock('@/components/keys/CreateKeyDialog', () => ({
  CreateKeyDialog: ({ open }: { open: boolean }) =>
    open ? <div data-testid="create-key-dialog" /> : null,
}))

import { useApiKeys } from '@/hooks/useApiKeys'
import { useIndexes } from '@/hooks/useIndexes'

function wrapper({ children }: { children: React.ReactNode }) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return (
    <QueryClientProvider client={qc}>
      <MemoryRouter>{children}</MemoryRouter>
    </QueryClientProvider>
  )
}

const SAMPLE_KEY = {
  value: 'abc123def456',
  description: 'Search Key',
  acl: ['search'],
  indexes: ['products'],
  createdAt: 1700000000,
  maxHitsPerQuery: null,
  maxQueriesPerIPPerHour: null,
  expiresAt: null,
}

describe('ApiKeys', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    vi.mocked(useIndexes).mockReturnValue({ data: [], isLoading: false, error: null } as any)
  })

  it('shows loading skeletons while fetching', () => {
    vi.mocked(useApiKeys).mockReturnValue({ data: undefined, isLoading: true } as any)
    render(<ApiKeys />, { wrapper })
    // Skeletons render as elements — just verify no crash and no keys list yet
    expect(screen.queryByTestId('keys-list')).not.toBeInTheDocument()
  })

  it('shows empty state when no keys exist', () => {
    vi.mocked(useApiKeys).mockReturnValue({ data: [], isLoading: false } as any)
    render(<ApiKeys />, { wrapper })
    expect(screen.getByText('No API keys')).toBeInTheDocument()
    expect(screen.getByText('Create Your First Key')).toBeInTheDocument()
  })

  it('renders key cards when keys exist', () => {
    vi.mocked(useApiKeys).mockReturnValue({ data: [SAMPLE_KEY], isLoading: false } as any)
    render(<ApiKeys />, { wrapper })
    expect(screen.getByTestId('keys-list')).toBeInTheDocument()
    expect(screen.getByText('Search Key')).toBeInTheDocument()
    expect(screen.getByText('abc123def456')).toBeInTheDocument()
  })

  it('shows index scope badge on key', () => {
    vi.mocked(useApiKeys).mockReturnValue({ data: [SAMPLE_KEY], isLoading: false } as any)
    render(<ApiKeys />, { wrapper })
    // Key has indexes: ['products'] — find the badge inside the key-scope section
    const scopeSection = screen.getByTestId('key-scope')
    expect(within(scopeSection).getByText('products')).toBeInTheDocument()
  })

  it('shows "All Indexes" badge for keys with no index restriction', () => {
    const globalKey = { ...SAMPLE_KEY, indexes: [] }
    vi.mocked(useApiKeys).mockReturnValue({ data: [globalKey], isLoading: false } as any)
    render(<ApiKeys />, { wrapper })
    expect(screen.getByText('All Indexes')).toBeInTheDocument()
  })

  it('shows filter bar when keys and indexes exist', () => {
    vi.mocked(useApiKeys).mockReturnValue({ data: [SAMPLE_KEY], isLoading: false } as any)
    vi.mocked(useIndexes).mockReturnValue({
      data: [{ uid: 'products', entries: 10, dataSize: 1024 }],
      isLoading: false,
      error: null,
    } as any)
    render(<ApiKeys />, { wrapper })
    expect(screen.getByTestId('index-filter-bar')).toBeInTheDocument()
    expect(screen.getByTestId('filter-index-products')).toBeInTheDocument()
  })

  it('does not show filter bar when there are no keys', () => {
    vi.mocked(useApiKeys).mockReturnValue({ data: [], isLoading: false } as any)
    vi.mocked(useIndexes).mockReturnValue({
      data: [{ uid: 'products', entries: 10, dataSize: 1024 }],
      isLoading: false,
      error: null,
    } as any)
    render(<ApiKeys />, { wrapper })
    expect(screen.queryByTestId('index-filter-bar')).not.toBeInTheDocument()
  })

  it('opens create dialog when Create Key button is clicked', async () => {
    const user = userEvent.setup()
    vi.mocked(useApiKeys).mockReturnValue({ data: [], isLoading: false } as any)
    render(<ApiKeys />, { wrapper })

    expect(screen.queryByTestId('create-key-dialog')).not.toBeInTheDocument()
    // Use exact name to target the header button, not the empty-state "Create Your First Key"
    await user.click(screen.getByRole('button', { name: 'Create Key' }))
    expect(screen.getByTestId('create-key-dialog')).toBeInTheDocument()
  })

  it('shows permissions badges', () => {
    const keyWithMultipleAcl = { ...SAMPLE_KEY, acl: ['search', 'listIndexes'] }
    vi.mocked(useApiKeys).mockReturnValue({ data: [keyWithMultipleAcl], isLoading: false } as any)
    render(<ApiKeys />, { wrapper })
    expect(screen.getByText('search')).toBeInTheDocument()
    expect(screen.getByText('listIndexes')).toBeInTheDocument()
  })

  it('filters keys by selected index', async () => {
    const user = userEvent.setup()
    const keyA = { ...SAMPLE_KEY, value: 'key-a', description: 'Key A', indexes: ['products'] }
    const keyB = { ...SAMPLE_KEY, value: 'key-b', description: 'Key B', indexes: ['articles'] }
    vi.mocked(useApiKeys).mockReturnValue({ data: [keyA, keyB], isLoading: false } as any)
    vi.mocked(useIndexes).mockReturnValue({
      data: [
        { uid: 'products', entries: 10, dataSize: 0 },
        { uid: 'articles', entries: 5, dataSize: 0 },
      ],
      isLoading: false,
      error: null,
    } as any)
    render(<ApiKeys />, { wrapper })

    // Both keys visible initially
    expect(screen.getByText('Key A')).toBeInTheDocument()
    expect(screen.getByText('Key B')).toBeInTheDocument()

    // Click "products" filter
    await user.click(screen.getByTestId('filter-index-products'))

    // Only Key A visible
    expect(screen.getByText('Key A')).toBeInTheDocument()
    expect(screen.queryByText('Key B')).not.toBeInTheDocument()
  })
})
