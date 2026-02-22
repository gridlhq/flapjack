import { render, screen, within, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, it, expect, vi } from 'vitest';
import { EmbedderPanel } from '../EmbedderPanel';
import type { EmbedderConfig } from '@/lib/types';

describe('EmbedderPanel', () => {
  const defaultProps = {
    embedders: undefined as Record<string, EmbedderConfig> | undefined,
    onChange: vi.fn(),
  };

  it('renders "No embedders configured" when embedders is undefined', () => {
    render(<EmbedderPanel {...defaultProps} />);
    expect(screen.getByText(/no embedders configured/i)).toBeInTheDocument();
  });

  it('renders existing embedder cards with name, source, and dimensions', () => {
    const embedders: Record<string, EmbedderConfig> = {
      default: { source: 'userProvided', dimensions: 384 },
      backup: { source: 'openAi', model: 'text-embedding-3-small' },
    };

    render(<EmbedderPanel {...defaultProps} embedders={embedders} />);

    expect(screen.getByTestId('embedder-card-default')).toBeInTheDocument();
    expect(screen.getByTestId('embedder-card-backup')).toBeInTheDocument();

    const defaultCard = screen.getByTestId('embedder-card-default');
    expect(within(defaultCard).getByText('default')).toBeInTheDocument();
    expect(within(defaultCard).getByText('userProvided')).toBeInTheDocument();
    expect(within(defaultCard).getByText('384')).toBeInTheDocument();

    const backupCard = screen.getByTestId('embedder-card-backup');
    expect(within(backupCard).getByText('backup')).toBeInTheDocument();
    expect(within(backupCard).getByText('openAi')).toBeInTheDocument();
  });

  it('renders Add Embedder button', () => {
    render(<EmbedderPanel {...defaultProps} />);
    expect(screen.getByTestId('add-embedder-btn')).toBeInTheDocument();
  });

  it('opens add dialog when Add Embedder clicked', async () => {
    const user = userEvent.setup();
    render(<EmbedderPanel {...defaultProps} />);

    await user.click(screen.getByTestId('add-embedder-btn'));

    expect(screen.getByTestId('embedder-dialog')).toBeInTheDocument();
  });

  it('shows source-specific fields: openAi shows apiKey and model fields, hides rest fields', async () => {
    const user = userEvent.setup();
    render(<EmbedderPanel {...defaultProps} />);

    await user.click(screen.getByTestId('add-embedder-btn'));

    // Switch away from default first, then back to openAi to test actual switching
    const sourceSelect = screen.getByTestId('embedder-source-select');
    await user.selectOptions(sourceSelect, 'userProvided');
    await user.selectOptions(sourceSelect, 'openAi');

    expect(screen.getByTestId('embedder-apikey-input')).toBeInTheDocument();
    expect(screen.getByTestId('embedder-model-input')).toBeInTheDocument();
    expect(screen.queryByTestId('embedder-url-input')).not.toBeInTheDocument();
  });

  it('shows source-specific fields: rest shows url field', async () => {
    const user = userEvent.setup();
    render(<EmbedderPanel {...defaultProps} />);

    await user.click(screen.getByTestId('add-embedder-btn'));

    const sourceSelect = screen.getByTestId('embedder-source-select');
    await user.selectOptions(sourceSelect, 'rest');

    expect(screen.getByTestId('embedder-url-input')).toBeInTheDocument();
  });

  it('shows source-specific fields: userProvided shows only dimensions field', async () => {
    const user = userEvent.setup();
    render(<EmbedderPanel {...defaultProps} />);

    await user.click(screen.getByTestId('add-embedder-btn'));

    const sourceSelect = screen.getByTestId('embedder-source-select');
    await user.selectOptions(sourceSelect, 'userProvided');

    expect(screen.getByTestId('embedder-dimensions-input')).toBeInTheDocument();
    expect(screen.queryByTestId('embedder-apikey-input')).not.toBeInTheDocument();
    expect(screen.queryByTestId('embedder-url-input')).not.toBeInTheDocument();
    expect(screen.queryByTestId('embedder-model-input')).not.toBeInTheDocument();
  });

  it('shows source-specific fields: fastEmbed shows model selector', async () => {
    const user = userEvent.setup();
    render(<EmbedderPanel {...defaultProps} />);

    await user.click(screen.getByTestId('add-embedder-btn'));

    const sourceSelect = screen.getByTestId('embedder-source-select');
    await user.selectOptions(sourceSelect, 'fastEmbed');

    expect(screen.getByTestId('embedder-model-input')).toBeInTheDocument();
  });

  it('calls onChange to add new embedder on save', async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    render(<EmbedderPanel {...defaultProps} onChange={onChange} />);

    await user.click(screen.getByTestId('add-embedder-btn'));

    await user.type(screen.getByTestId('embedder-name-input'), 'my-embedder');

    const sourceSelect = screen.getByTestId('embedder-source-select');
    await user.selectOptions(sourceSelect, 'userProvided');

    await user.type(screen.getByTestId('embedder-dimensions-input'), '384');

    await user.click(screen.getByTestId('embedder-save-btn'));

    expect(onChange).toHaveBeenCalledWith({
      embedders: {
        'my-embedder': expect.objectContaining({
          source: 'userProvided',
          dimensions: 384,
        }),
      },
    });
  });

  it('calls onChange to remove embedder when delete clicked', async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    const embedders: Record<string, EmbedderConfig> = {
      default: { source: 'userProvided', dimensions: 384 },
      backup: { source: 'openAi', model: 'text-embedding-3-small' },
    };

    render(<EmbedderPanel {...defaultProps} embedders={embedders} onChange={onChange} />);

    await user.click(screen.getByTestId('embedder-delete-default'));

    // Confirm delete
    const confirmBtn = screen.getByRole('button', { name: /confirm/i });
    await user.click(confirmBtn);

    expect(onChange).toHaveBeenCalledWith({
      embedders: {
        backup: { source: 'openAi', model: 'text-embedding-3-small' },
      },
    });
  });

  it('validates that embedder name is non-empty', async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    render(<EmbedderPanel {...defaultProps} onChange={onChange} />);

    await user.click(screen.getByTestId('add-embedder-btn'));

    // Leave name empty, try to save
    const sourceSelect = screen.getByTestId('embedder-source-select');
    await user.selectOptions(sourceSelect, 'userProvided');
    await user.type(screen.getByTestId('embedder-dimensions-input'), '384');

    await user.click(screen.getByTestId('embedder-save-btn'));

    // onChange should NOT have been called
    expect(onChange).not.toHaveBeenCalled();
  });

  it('validates that fastEmbed requires model selection', async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    render(<EmbedderPanel {...defaultProps} onChange={onChange} />);

    await user.click(screen.getByTestId('add-embedder-btn'));

    await user.type(screen.getByTestId('embedder-name-input'), 'test-fe');

    const sourceSelect = screen.getByTestId('embedder-source-select');
    await user.selectOptions(sourceSelect, 'fastEmbed');

    // Don't select a model, try to save
    await user.click(screen.getByTestId('embedder-save-btn'));

    expect(onChange).not.toHaveBeenCalled();
    expect(screen.getByText(/model selection is required/i)).toBeInTheDocument();
  });

  it('validates that REST request template is valid JSON', async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    render(<EmbedderPanel {...defaultProps} onChange={onChange} />);

    await user.click(screen.getByTestId('add-embedder-btn'));

    await user.type(screen.getByTestId('embedder-name-input'), 'test-rest');

    const sourceSelect = screen.getByTestId('embedder-source-select');
    await user.selectOptions(sourceSelect, 'rest');

    await user.type(screen.getByTestId('embedder-url-input'), 'https://api.example.com');

    // Set invalid JSON in request template (use fireEvent because userEvent.type treats { as special key)
    const requestTextarea = screen.getByLabelText(/request template/i);
    fireEvent.change(requestTextarea, { target: { value: '{invalid json' } });

    await user.click(screen.getByTestId('embedder-save-btn'));

    expect(onChange).not.toHaveBeenCalled();
    expect(screen.getByText(/not valid json/i)).toBeInTheDocument();
  });

  it('validates that dimensions is a positive number when provided', async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    render(<EmbedderPanel {...defaultProps} onChange={onChange} />);

    await user.click(screen.getByTestId('add-embedder-btn'));

    await user.type(screen.getByTestId('embedder-name-input'), 'test');

    const sourceSelect = screen.getByTestId('embedder-source-select');
    await user.selectOptions(sourceSelect, 'userProvided');

    // Type 0 (not positive)
    await user.type(screen.getByTestId('embedder-dimensions-input'), '0');

    await user.click(screen.getByTestId('embedder-save-btn'));

    expect(onChange).not.toHaveBeenCalled();
  });

  it('opens edit dialog with prefilled values when edit button clicked', async () => {
    const user = userEvent.setup();
    const embedders: Record<string, EmbedderConfig> = {
      default: { source: 'userProvided', dimensions: 384 },
    };

    render(<EmbedderPanel {...defaultProps} embedders={embedders} />);

    await user.click(screen.getByTestId('embedder-edit-default'));

    // Dialog should open with prefilled values
    expect(screen.getByTestId('embedder-dialog')).toBeInTheDocument();
    const nameInput = screen.getByTestId('embedder-name-input') as HTMLInputElement;
    expect(nameInput.value).toBe('default');
    expect(nameInput).toBeDisabled();

    const sourceSelect = screen.getByTestId('embedder-source-select') as HTMLSelectElement;
    expect(sourceSelect.value).toBe('userProvided');

    const dimInput = screen.getByTestId('embedder-dimensions-input') as HTMLInputElement;
    expect(dimInput.value).toBe('384');
  });

  it('preserves existing embedders when adding a new one', async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    const embedders: Record<string, EmbedderConfig> = {
      existing: { source: 'openAi', model: 'text-embedding-3-small' },
    };

    render(<EmbedderPanel {...defaultProps} embedders={embedders} onChange={onChange} />);

    await user.click(screen.getByTestId('add-embedder-btn'));
    await user.type(screen.getByTestId('embedder-name-input'), 'new-emb');

    const sourceSelect = screen.getByTestId('embedder-source-select');
    await user.selectOptions(sourceSelect, 'userProvided');
    await user.type(screen.getByTestId('embedder-dimensions-input'), '256');

    await user.click(screen.getByTestId('embedder-save-btn'));

    expect(onChange).toHaveBeenCalledWith({
      embedders: expect.objectContaining({
        existing: { source: 'openAi', model: 'text-embedding-3-small' },
        'new-emb': expect.objectContaining({ source: 'userProvided', dimensions: 256 }),
      }),
    });
  });

  it('renders "No embedders configured" when embedders is empty object', () => {
    render(<EmbedderPanel {...defaultProps} embedders={{}} />);
    expect(screen.getByText(/no embedders configured/i)).toBeInTheDocument();
  });

  it('resets model field when switching source from openAi to fastEmbed', async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    const embedders: Record<string, EmbedderConfig> = {
      myemb: { source: 'openAi', model: 'text-embedding-3-small', apiKey: 'sk-xxx' },
    };

    render(<EmbedderPanel {...defaultProps} embedders={embedders} onChange={onChange} />);

    // Open edit dialog — model is prefilled with 'text-embedding-3-small'
    await user.click(screen.getByTestId('embedder-edit-myemb'));

    const modelInput = screen.getByTestId('embedder-model-input') as HTMLInputElement;
    expect(modelInput.value).toBe('text-embedding-3-small');

    // Switch source to fastEmbed
    const sourceSelect = screen.getByTestId('embedder-source-select');
    await user.selectOptions(sourceSelect, 'fastEmbed');

    // The model select for fastEmbed should be empty (reset), not carry over the openAi model
    const feModelSelect = screen.getByTestId('embedder-model-input') as HTMLSelectElement;
    expect(feModelSelect.value).toBe('');

    // Trying to save without selecting a fastEmbed model should fail validation
    await user.click(screen.getByTestId('embedder-save-btn'));
    expect(onChange).not.toHaveBeenCalled();
    expect(screen.getByText(/model selection is required/i)).toBeInTheDocument();
  });

  it('saves edited embedder with updated config via onChange', async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    const embedders: Record<string, EmbedderConfig> = {
      default: { source: 'userProvided', dimensions: 384 },
    };

    render(<EmbedderPanel {...defaultProps} embedders={embedders} onChange={onChange} />);

    // Open edit dialog
    await user.click(screen.getByTestId('embedder-edit-default'));

    // Change dimensions from 384 to 512
    const dimInput = screen.getByTestId('embedder-dimensions-input');
    await user.clear(dimInput);
    await user.type(dimInput, '512');

    // Save
    await user.click(screen.getByTestId('embedder-save-btn'));

    expect(onChange).toHaveBeenCalledWith({
      embedders: {
        default: expect.objectContaining({
          source: 'userProvided',
          dimensions: 512,
        }),
      },
    });
  });
});
