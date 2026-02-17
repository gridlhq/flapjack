import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { SearchBox } from './SearchBox'
import type { SearchParams } from '@/lib/types'

describe('SearchBox', () => {
  const defaultParams: SearchParams = {
    query: '',
    page: 0,
  }

  const defaultProps = {
    indexName: 'test-index',
    params: defaultParams,
    onParamsChange: vi.fn(),
  }

  it('renders search input', () => {
    render(<SearchBox {...defaultProps} />)

    expect(
      screen.getByPlaceholderText('Search documents...')
    ).toBeInTheDocument()
  })

  it('renders with initial query from params', () => {
    const params = { ...defaultParams, query: 'laptop' }

    render(<SearchBox {...defaultProps} params={params} />)

    expect(screen.getByDisplayValue('laptop')).toBeInTheDocument()
  })

  it('calls onParamsChange when search is submitted', async () => {
    const user = userEvent.setup()
    const handleParamsChange = vi.fn()

    render(
      <SearchBox {...defaultProps} onParamsChange={handleParamsChange} />
    )

    const input = screen.getByPlaceholderText('Search documents...')
    await user.type(input, 'laptop')

    const searchButton = screen.getByRole('button', { name: /search/i })
    await user.click(searchButton)

    expect(handleParamsChange).toHaveBeenCalledWith({ query: 'laptop' })
  })

  it('submits search on Enter key', async () => {
    const user = userEvent.setup()
    const handleParamsChange = vi.fn()

    render(
      <SearchBox {...defaultProps} onParamsChange={handleParamsChange} />
    )

    const input = screen.getByPlaceholderText('Search documents...')
    await user.type(input, 'laptop{Enter}')

    expect(handleParamsChange).toHaveBeenCalledWith({ query: 'laptop' })
  })

  it('shows filters panel when filters button clicked', async () => {
    const user = userEvent.setup()

    render(<SearchBox {...defaultProps} />)

    // Filters panel not visible initially
    expect(
      screen.queryByPlaceholderText(/category:books/i)
    ).not.toBeInTheDocument()

    // Click filters button (icon button)
    const filtersButton = screen.getByRole('button', { name: '' })
    await user.click(filtersButton)

    // Filters panel now visible
    expect(
      screen.getByPlaceholderText(/category:books/i)
    ).toBeInTheDocument()
  })

  it('applies filters when Apply Filters clicked', async () => {
    const user = userEvent.setup()
    const handleParamsChange = vi.fn()

    render(
      <SearchBox {...defaultProps} onParamsChange={handleParamsChange} />
    )

    // Open filters panel
    const filtersButton = screen.getByRole('button', { name: '' })
    await user.click(filtersButton)

    // Enter filter
    const filterInput = screen.getByPlaceholderText(/category:books/i)
    await user.type(filterInput, 'category:electronics')

    // Apply filters
    const applyButton = screen.getByRole('button', { name: /apply filters/i })
    await user.click(applyButton)

    expect(handleParamsChange).toHaveBeenCalledWith({
      filters: 'category:electronics',
    })
  })

  it('closes filters panel when Cancel clicked', async () => {
    const user = userEvent.setup()

    render(<SearchBox {...defaultProps} />)

    // Open filters panel
    const filtersButton = screen.getByRole('button', { name: '' })
    await user.click(filtersButton)

    expect(
      screen.getByPlaceholderText(/category:books/i)
    ).toBeInTheDocument()

    // Click cancel
    const cancelButton = screen.getByRole('button', { name: /cancel/i })
    await user.click(cancelButton)

    // Filters panel closed
    expect(
      screen.queryByPlaceholderText(/category:books/i)
    ).not.toBeInTheDocument()
  })

  it('shows active filter badge when filters applied', () => {
    const params = {
      ...defaultParams,
      filters: 'category:electronics',
    }

    render(<SearchBox {...defaultProps} params={params} />)

    expect(screen.getByText('Active filter:')).toBeInTheDocument()
    expect(screen.getByText('category:electronics')).toBeInTheDocument()
  })

  it('clears filters when Clear button clicked', async () => {
    const user = userEvent.setup()
    const handleParamsChange = vi.fn()

    const params = {
      ...defaultParams,
      filters: 'category:electronics',
    }

    render(
      <SearchBox
        {...defaultProps}
        params={params}
        onParamsChange={handleParamsChange}
      />
    )

    // Find and click clear button
    const clearButton = screen.getByRole('button', { name: /clear filters/i })
    await user.click(clearButton)

    expect(handleParamsChange).toHaveBeenCalledWith({ filters: undefined })
  })

  it('highlights filters button when filters are active', () => {
    const params = {
      ...defaultParams,
      filters: 'category:electronics',
    }

    const { container } = render(
      <SearchBox {...defaultProps} params={params} />
    )

    // Filters button should have default variant (not outline) when active
    // This is a simplified check - in real test we'd check computed styles
    expect(container).toMatchSnapshot()
  })

  it('updates query input as user types', async () => {
    const user = userEvent.setup()

    render(<SearchBox {...defaultProps} />)

    const input = screen.getByPlaceholderText('Search documents...')
    await user.type(input, 'test')

    expect(input).toHaveValue('test')
  })

  it('does not call onParamsChange while typing', async () => {
    const user = userEvent.setup()
    const handleParamsChange = vi.fn()

    render(
      <SearchBox {...defaultProps} onParamsChange={handleParamsChange} />
    )

    const input = screen.getByPlaceholderText('Search documents...')
    await user.type(input, 'laptop')

    // Should not be called until search is submitted
    expect(handleParamsChange).not.toHaveBeenCalled()
  })
})
