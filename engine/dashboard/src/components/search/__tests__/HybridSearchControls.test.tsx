import { render, screen, act } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import { HybridSearchControls } from '../HybridSearchControls';

describe('HybridSearchControls', () => {
  const defaultProps = {
    embedderNames: [] as string[],
    onParamsChange: vi.fn(),
  };

  it('renders nothing when no embedders configured (empty embedderNames)', () => {
    const { container } = render(<HybridSearchControls {...defaultProps} />);
    expect(container.firstChild).toBeNull();
  });

  it('renders semantic ratio slider when embedders configured', () => {
    render(
      <HybridSearchControls {...defaultProps} embedderNames={['default']} />
    );
    expect(screen.getByTestId('semantic-ratio-slider')).toBeInTheDocument();
  });

  it('slider defaults to 0.5', () => {
    render(
      <HybridSearchControls {...defaultProps} embedderNames={['default']} />
    );
    const slider = screen.getByTestId('semantic-ratio-slider') as HTMLInputElement;
    expect(slider.value).toBe('0.5');
  });

  it('calls onParamsChange with hybrid params when slider changes', async () => {
    const onParamsChange = vi.fn();
    render(
      <HybridSearchControls
        {...defaultProps}
        embedderNames={['default']}
        onParamsChange={onParamsChange}
      />
    );

    const slider = screen.getByTestId('semantic-ratio-slider');
    // Fire native change event to simulate slider drag
    const nativeInputValueSetter = Object.getOwnPropertyDescriptor(
      HTMLInputElement.prototype,
      'value'
    )!.set!;
    await act(async () => {
      nativeInputValueSetter.call(slider, '0.7');
      slider.dispatchEvent(new Event('change', { bubbles: true }));
    });

    expect(onParamsChange).toHaveBeenCalledWith(
      expect.objectContaining({
        hybrid: expect.objectContaining({ semanticRatio: 0.7 }),
      })
    );
  });

  it('shows embedder selector when multiple embedders configured', () => {
    render(
      <HybridSearchControls
        {...defaultProps}
        embedderNames={['default', 'backup']}
      />
    );
    expect(screen.getByTestId('embedder-select')).toBeInTheDocument();
  });

  it('does not show embedder selector when only one embedder configured', () => {
    render(
      <HybridSearchControls {...defaultProps} embedderNames={['default']} />
    );
    expect(screen.queryByTestId('embedder-select')).not.toBeInTheDocument();
  });

  it('shows "Hybrid Search" label with current ratio display', () => {
    render(
      <HybridSearchControls {...defaultProps} embedderNames={['default']} />
    );
    expect(screen.getByTestId('hybrid-controls')).toBeInTheDocument();
    expect(screen.getByText(/hybrid search/i)).toBeInTheDocument();
    expect(screen.getByTestId('semantic-ratio-label')).toBeInTheDocument();
  });

  it('ratio 0.0 shows "Keyword only" label', () => {
    render(
      <HybridSearchControls
        {...defaultProps}
        embedderNames={['default']}
        initialRatio={0.0}
      />
    );
    expect(screen.getByTestId('semantic-ratio-label')).toHaveTextContent(
      'Keyword only'
    );
  });

  it('ratio 1.0 shows "Semantic only" label', () => {
    render(
      <HybridSearchControls
        {...defaultProps}
        embedderNames={['default']}
        initialRatio={1.0}
      />
    );
    expect(screen.getByTestId('semantic-ratio-label')).toHaveTextContent(
      'Semantic only'
    );
  });

  it('emits initial hybrid params on mount when embedders configured', () => {
    const onParamsChange = vi.fn();
    render(
      <HybridSearchControls
        {...defaultProps}
        embedderNames={['default']}
        onParamsChange={onParamsChange}
      />
    );
    // Should emit the initial ratio (0.5) on mount so parent state matches UI
    expect(onParamsChange).toHaveBeenCalledWith({
      hybrid: { semanticRatio: 0.5 },
    });
  });

  it('does not emit on mount when no embedders', () => {
    const onParamsChange = vi.fn();
    render(
      <HybridSearchControls
        {...defaultProps}
        embedderNames={[]}
        onParamsChange={onParamsChange}
      />
    );
    expect(onParamsChange).not.toHaveBeenCalled();
  });

  it('default ratio 0.5 shows "Balanced" label', () => {
    render(
      <HybridSearchControls {...defaultProps} embedderNames={['default']} />
    );
    expect(screen.getByTestId('semantic-ratio-label')).toHaveTextContent('Balanced');
  });

  it('calls onParamsChange with selected embedder when embedder select changes', async () => {
    const onParamsChange = vi.fn();
    render(
      <HybridSearchControls
        {...defaultProps}
        embedderNames={['alpha', 'beta']}
        onParamsChange={onParamsChange}
      />
    );

    // Clear initial mount emission
    onParamsChange.mockClear();

    const embedderSelect = screen.getByTestId('embedder-select');
    // Fire native change event to simulate selecting a different embedder
    await act(async () => {
      const nativeSetter = Object.getOwnPropertyDescriptor(
        HTMLSelectElement.prototype,
        'value'
      )!.set!;
      nativeSetter.call(embedderSelect, 'beta');
      embedderSelect.dispatchEvent(new Event('change', { bubbles: true }));
    });

    expect(onParamsChange).toHaveBeenCalledWith({
      hybrid: expect.objectContaining({ embedder: 'beta' }),
    });
  });

  it('syncs selected embedder when embedder list loads asynchronously', async () => {
    const onParamsChange = vi.fn();
    // First render: no embedders (settings still loading)
    const { rerender } = render(
      <HybridSearchControls
        {...defaultProps}
        embedderNames={[]}
        onParamsChange={onParamsChange}
      />
    );

    // Settings loaded: now have two embedders
    rerender(
      <HybridSearchControls
        {...defaultProps}
        embedderNames={['alpha', 'beta']}
        onParamsChange={onParamsChange}
      />
    );

    // Move slider to trigger emission — it should include the synced embedder
    onParamsChange.mockClear();
    const slider = screen.getByTestId('semantic-ratio-slider');
    const nativeInputValueSetter = Object.getOwnPropertyDescriptor(
      HTMLInputElement.prototype,
      'value'
    )!.set!;
    await act(async () => {
      nativeInputValueSetter.call(slider, '0.8');
      slider.dispatchEvent(new Event('change', { bubbles: true }));
    });

    // With 2 embedders, hybrid params should include the first embedder name
    expect(onParamsChange).toHaveBeenCalledWith({
      hybrid: expect.objectContaining({ embedder: 'alpha' }),
    });
  });
});
