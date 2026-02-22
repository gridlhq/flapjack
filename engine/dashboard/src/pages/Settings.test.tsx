import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { Settings } from './Settings'

vi.mock('@/hooks/useSettings', () => ({
  useSettings: vi.fn(),
  useUpdateSettings: () => ({ mutateAsync: vi.fn().mockResolvedValue({}), isPending: false }),
}))

vi.mock('@/hooks/useIndexes', () => ({
  useCompactIndex: () => ({ mutateAsync: vi.fn(), isPending: false }),
}))

// Monaco editor is heavy — stub it out
vi.mock('@monaco-editor/react', () => ({
  default: ({ value }: { value: string }) => (
    <div data-testid="monaco-editor">{value}</div>
  ),
}))

vi.mock('@/components/settings/SettingsForm', () => ({
  SettingsForm: ({ onChange }: { onChange: (updates: any) => void }) => (
    <div data-testid="settings-form">
      <button onClick={() => onChange({ searchableAttributes: ['name'] })}>
        Change Setting
      </button>
    </div>
  ),
}))

import { useSettings } from '@/hooks/useSettings'

const SAMPLE_SETTINGS = {
  searchableAttributes: ['name', 'description'],
  attributesForFaceting: ['brand'],
  customRanking: [],
  ranking: [],
}

function makeWrapper(indexName?: string) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  const path = indexName ? `/index/${indexName}/settings` : '/settings'
  const route = indexName ? '/index/:indexName/settings' : '/settings'

  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={qc}>
      <MemoryRouter initialEntries={[path]}>
        <Routes>
          <Route path={route} element={children} />
          <Route path="/overview" element={<div>Overview</div>} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>
  )
}

describe('Settings', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('shows "No index selected" when no indexName param', () => {
    vi.mocked(useSettings).mockReturnValue({ data: undefined, isLoading: false } as any)
    render(<Settings />, { wrapper: makeWrapper(undefined) })
    expect(screen.getByText('No index selected')).toBeInTheDocument()
    expect(screen.getByRole('link', { name: /go to overview/i })).toBeInTheDocument()
  })

  it('shows loading skeleton when settings are loading', () => {
    vi.mocked(useSettings).mockReturnValue({ data: undefined, isLoading: true } as any)
    render(<Settings />, { wrapper: makeWrapper('products') })
    // SettingsForm not rendered yet during load
    expect(screen.queryByTestId('settings-form')).not.toBeInTheDocument()
  })

  it('renders settings form when loaded', () => {
    vi.mocked(useSettings).mockReturnValue({ data: SAMPLE_SETTINGS, isLoading: false } as any)
    render(<Settings />, { wrapper: makeWrapper('products') })
    expect(screen.getByTestId('settings-form')).toBeInTheDocument()
  })

  it('shows Save and Reset buttons when form is dirty', async () => {
    const user = userEvent.setup()
    vi.mocked(useSettings).mockReturnValue({ data: SAMPLE_SETTINGS, isLoading: false } as any)
    render(<Settings />, { wrapper: makeWrapper('products') })

    // Initially save/reset may not be visible (not dirty)
    await user.click(screen.getByRole('button', { name: /change setting/i }))

    expect(screen.getByRole('button', { name: /save/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /reset/i })).toBeInTheDocument()
  })

  it('reset clears dirty state', async () => {
    const user = userEvent.setup()
    vi.mocked(useSettings).mockReturnValue({ data: SAMPLE_SETTINGS, isLoading: false } as any)
    render(<Settings />, { wrapper: makeWrapper('products') })

    await user.click(screen.getByRole('button', { name: /change setting/i }))
    expect(screen.getByRole('button', { name: /save/i })).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: /reset/i }))
    // After reset the Save/Reset buttons should disappear
    await waitFor(() => {
      expect(screen.queryByRole('button', { name: /save/i })).not.toBeInTheDocument()
    })
  })

  it('shows back link to index overview', () => {
    vi.mocked(useSettings).mockReturnValue({ data: SAMPLE_SETTINGS, isLoading: false } as any)
    render(<Settings />, { wrapper: makeWrapper('products') })
    // Should show a back navigation element
    expect(screen.getByText('products')).toBeInTheDocument()
  })
})
