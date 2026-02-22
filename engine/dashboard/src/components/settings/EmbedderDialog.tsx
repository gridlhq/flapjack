import { memo, useState, useEffect } from 'react';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Textarea } from '@/components/ui/textarea';
import { Label } from '@/components/ui/label';
import { Button } from '@/components/ui/button';
import type { EmbedderConfig, EmbedderSource } from '@/lib/types';

const FAST_EMBED_MODELS = [
  'bge-small-en-v1.5',
  'bge-base-en-v1.5',
  'bge-large-en-v1.5',
  'all-MiniLM-L6-v2',
  'all-MiniLM-L12-v2',
  'nomic-embed-text-v1.5',
  'multilingual-e5-small',
] as const;

interface EmbedderDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSave: (name: string, config: EmbedderConfig) => void;
  editName?: string;
  editConfig?: EmbedderConfig;
}

export const EmbedderDialog = memo(function EmbedderDialog({
  open,
  onOpenChange,
  onSave,
  editName,
  editConfig,
}: EmbedderDialogProps) {
  const isEditing = !!editName;

  const [name, setName] = useState('');
  const [source, setSource] = useState<EmbedderSource>('openAi');
  const [dimensions, setDimensions] = useState('');
  const [apiKey, setApiKey] = useState('');
  const [model, setModel] = useState('');
  const [url, setUrl] = useState('');
  const [requestTemplate, setRequestTemplate] = useState('');
  const [responseTemplate, setResponseTemplate] = useState('');
  const [headers, setHeaders] = useState('');
  const [documentTemplate, setDocumentTemplate] = useState('');
  const [validationError, setValidationError] = useState('');

  // Reset form when dialog opens/closes or edit target changes
  useEffect(() => {
    if (open) {
      if (editConfig) {
        setName(editName || '');
        setSource(editConfig.source);
        setDimensions(editConfig.dimensions?.toString() || '');
        setApiKey(editConfig.apiKey || '');
        setModel(editConfig.model || '');
        setUrl(editConfig.url || '');
        setRequestTemplate(editConfig.request ? JSON.stringify(editConfig.request, null, 2) : '');
        setResponseTemplate(editConfig.response ? JSON.stringify(editConfig.response, null, 2) : '');
        setHeaders(editConfig.headers ? JSON.stringify(editConfig.headers, null, 2) : '');
        setDocumentTemplate(editConfig.documentTemplate || '');
      } else {
        setName('');
        setSource('openAi');
        setDimensions('');
        setApiKey('');
        setModel('');
        setUrl('');
        setRequestTemplate('');
        setResponseTemplate('');
        setHeaders('');
        setDocumentTemplate('');
      }
      setValidationError('');
    }
  }, [open, editName, editConfig]);

  function handleSave() {
    // Validate name
    if (!name.trim()) {
      setValidationError('Embedder name is required');
      return;
    }

    // Validate dimensions for userProvided (required) and for any source when provided
    const dimNum = dimensions ? parseInt(dimensions, 10) : undefined;
    if (source === 'userProvided' && (!dimNum || dimNum <= 0)) {
      setValidationError('Dimensions must be a positive number for userProvided embedder');
      return;
    }
    if (dimensions && (!dimNum || dimNum <= 0)) {
      setValidationError('Dimensions must be a positive number');
      return;
    }

    // Validate fastEmbed requires a model selection
    if (source === 'fastEmbed' && !model.trim()) {
      setValidationError('Model selection is required for fastEmbed embedder');
      return;
    }

    // Validate JSON fields for REST source before saving
    if (source === 'rest') {
      if (requestTemplate.trim()) {
        try { JSON.parse(requestTemplate); } catch {
          setValidationError('Request template is not valid JSON');
          return;
        }
      }
      if (responseTemplate.trim()) {
        try { JSON.parse(responseTemplate); } catch {
          setValidationError('Response template is not valid JSON');
          return;
        }
      }
      if (headers.trim()) {
        try { JSON.parse(headers); } catch {
          setValidationError('Headers is not valid JSON');
          return;
        }
      }
    }

    const config: EmbedderConfig = { source };

    if (dimNum && dimNum > 0) config.dimensions = dimNum;
    if (documentTemplate.trim()) config.documentTemplate = documentTemplate.trim();

    if (source === 'openAi') {
      if (apiKey.trim()) config.apiKey = apiKey.trim();
      if (model.trim()) config.model = model.trim();
    } else if (source === 'rest') {
      if (url.trim()) config.url = url.trim();
      if (requestTemplate.trim()) config.request = JSON.parse(requestTemplate);
      if (responseTemplate.trim()) config.response = JSON.parse(responseTemplate);
      if (headers.trim()) config.headers = JSON.parse(headers);
    } else if (source === 'fastEmbed') {
      config.model = model.trim();
    }

    onSave(name.trim(), config);
    onOpenChange(false);
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent data-testid="embedder-dialog" className="max-w-lg max-h-[85vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>{isEditing ? 'Edit Embedder' : 'Add Embedder'}</DialogTitle>
          <DialogDescription>
            Configure an embedding model for vector search
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          {/* Name */}
          <div className="space-y-2">
            <Label htmlFor="embedder-name">Name</Label>
            <Input
              id="embedder-name"
              data-testid="embedder-name-input"
              value={name}
              onChange={(e) => { setName(e.target.value); setValidationError(''); }}
              placeholder="default"
              disabled={isEditing}
            />
          </div>

          {/* Source */}
          <div className="space-y-2">
            <Label htmlFor="embedder-source">Source</Label>
            <select
              id="embedder-source"
              data-testid="embedder-source-select"
              value={source}
              onChange={(e) => {
                setSource(e.target.value as EmbedderSource);
                // Reset source-specific fields to prevent stale values
                // (e.g. openAi model leaking into fastEmbed on source switch)
                setModel('');
                setApiKey('');
                setUrl('');
                setRequestTemplate('');
                setResponseTemplate('');
                setHeaders('');
                setValidationError('');
              }}
              className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
            >
              <option value="openAi">openAi</option>
              <option value="rest">rest</option>
              <option value="userProvided">userProvided</option>
              <option value="fastEmbed">fastEmbed</option>
            </select>
          </div>

          {/* Dimensions (all sources, required for userProvided) */}
          <div className="space-y-2">
            <Label htmlFor="embedder-dimensions">
              Dimensions{source === 'userProvided' ? ' (required)' : ''}
            </Label>
            <Input
              id="embedder-dimensions"
              data-testid="embedder-dimensions-input"
              type="number"
              min="1"
              value={dimensions}
              onChange={(e) => { setDimensions(e.target.value); setValidationError(''); }}
              placeholder={source === 'userProvided' ? '384' : 'Auto-detected'}
            />
          </div>

          {/* Source-specific fields */}
          {source === 'openAi' && (
            <>
              <div className="space-y-2">
                <Label htmlFor="embedder-apikey">API Key</Label>
                <Input
                  id="embedder-apikey"
                  data-testid="embedder-apikey-input"
                  type="password"
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                  placeholder="sk-..."
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="embedder-model">Model</Label>
                <Input
                  id="embedder-model"
                  data-testid="embedder-model-input"
                  value={model}
                  onChange={(e) => setModel(e.target.value)}
                  placeholder="text-embedding-3-small"
                />
              </div>
            </>
          )}

          {source === 'rest' && (
            <>
              <div className="space-y-2">
                <Label htmlFor="embedder-url">URL</Label>
                <Input
                  id="embedder-url"
                  data-testid="embedder-url-input"
                  value={url}
                  onChange={(e) => setUrl(e.target.value)}
                  placeholder="https://api.example.com/embed"
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="embedder-request">Request Template (JSON)</Label>
                <Textarea
                  id="embedder-request"
                  value={requestTemplate}
                  onChange={(e) => setRequestTemplate(e.target.value)}
                  placeholder='{"input": "{{text}}"}'
                  rows={3}
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="embedder-response">Response Template (JSON)</Label>
                <Textarea
                  id="embedder-response"
                  value={responseTemplate}
                  onChange={(e) => setResponseTemplate(e.target.value)}
                  placeholder='{"embedding": "{{embedding}}"}'
                  rows={3}
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="embedder-headers">Headers (JSON)</Label>
                <Textarea
                  id="embedder-headers"
                  value={headers}
                  onChange={(e) => setHeaders(e.target.value)}
                  placeholder='{"Authorization": "Bearer ..."}'
                  rows={2}
                />
              </div>
            </>
          )}

          {source === 'fastEmbed' && (
            <div className="space-y-2">
              <Label htmlFor="embedder-fe-model">Model</Label>
              <select
                id="embedder-fe-model"
                data-testid="embedder-model-input"
                value={model}
                onChange={(e) => setModel(e.target.value)}
                className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
              >
                <option value="">Select a model...</option>
                {FAST_EMBED_MODELS.map((m) => (
                  <option key={m} value={m}>{m}</option>
                ))}
              </select>
            </div>
          )}

          {/* Document Template (all sources) */}
          <div className="space-y-2">
            <Label htmlFor="embedder-doc-template">Document Template</Label>
            <Textarea
              id="embedder-doc-template"
              value={documentTemplate}
              onChange={(e) => setDocumentTemplate(e.target.value)}
              placeholder="{{doc.title}} {{doc.description}}"
              rows={2}
            />
          </div>

          {validationError && (
            <p className="text-sm text-destructive">{validationError}</p>
          )}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button data-testid="embedder-save-btn" onClick={handleSave}>
            {isEditing ? 'Save Changes' : 'Add Embedder'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
});
