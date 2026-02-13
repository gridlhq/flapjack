import { useState, useCallback, useRef, useEffect, type DragEvent } from 'react';
import { useAddDocuments } from '@/hooks/useDocuments';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Upload, FileJson, Database, Plus, Trash2, Copy, Check } from 'lucide-react';
import { useIndexFields, type FieldInfo } from '@/hooks/useIndexFields';
import { SampleDataTabContent } from './SampleDataTabContent';

interface AddDocumentsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  indexName: string;
}

function parseDocuments(text: string): Record<string, unknown>[] {
  const trimmed = text.trim();
  if (!trimmed) throw new Error('No content provided');

  const parsed = JSON.parse(trimmed);

  // Accept a single object or an array of objects
  if (Array.isArray(parsed)) {
    if (parsed.length === 0) throw new Error('Array is empty');
    if (typeof parsed[0] !== 'object' || parsed[0] === null) {
      throw new Error('Array must contain objects');
    }
    return parsed;
  }

  if (typeof parsed === 'object' && parsed !== null) {
    return [parsed];
  }

  throw new Error('Expected a JSON object or array of objects');
}

// --- Form builder types & helpers ---

interface FormField {
  id: string;
  name: string;
  type: 'text' | 'number' | 'boolean';
  value: string;
}

let nextFieldId = 0;
function makeFieldId() {
  return `field_${++nextFieldId}`;
}

function toFormFields(detected: FieldInfo[]): FormField[] {
  return detected.map((f) => ({
    id: makeFieldId(),
    name: f.name,
    type: f.type,
    value: '',
  }));
}

function buildDocumentFromFields(fields: FormField[]): Record<string, unknown> | null {
  const filled = fields.filter((f) => f.name.trim() && f.value.trim());
  if (filled.length === 0) return null;

  const doc: Record<string, unknown> = {};
  doc.objectID = crypto.randomUUID().slice(0, 8);

  for (const f of filled) {
    const key = f.name.trim();
    switch (f.type) {
      case 'number':
        doc[key] = Number(f.value) || 0;
        break;
      case 'boolean':
        doc[key] = f.value === 'true';
        break;
      default:
        doc[key] = f.value;
        break;
    }
  }
  return doc;
}

export function AddDocumentsDialog({
  open,
  onOpenChange,
  indexName,
}: AddDocumentsDialogProps) {
  const addDocuments = useAddDocuments(indexName);
  const [activeTab, setActiveTab] = useState('json');
  const [jsonText, setJsonText] = useState('');
  const [error, setError] = useState('');
  const [dragOver, setDragOver] = useState(false);
  const [fileName, setFileName] = useState('');
  const [copied, setCopied] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  // Form builder state
  const { data: detectedFields } = useIndexFields(indexName, open);
  const [fields, setFields] = useState<FormField[]>([]);
  const [formInitialized, setFormInitialized] = useState(false);
  const formDrivenRef = useRef(true); // tracks whether form is driving the textarea

  // Populate fields from detected schema once when dialog opens
  useEffect(() => {
    if (open && !formInitialized && detectedFields) {
      if (detectedFields.length > 0) {
        setFields(toFormFields(detectedFields));
      } else {
        setFields([{ id: makeFieldId(), name: '', type: 'text', value: '' }]);
      }
      setFormInitialized(true);
    }
    if (!open) {
      setFormInitialized(false);
      formDrivenRef.current = true;
    }
  }, [open, detectedFields, formInitialized]);

  // When form fields change, update JSON textarea (if form is driving)
  useEffect(() => {
    if (!formDrivenRef.current) return;
    const doc = buildDocumentFromFields(fields);
    if (doc) {
      setJsonText(JSON.stringify(doc, null, 2));
    }
  }, [fields]);

  const addField = useCallback(() => {
    setFields((prev) => [...prev, { id: makeFieldId(), name: '', type: 'text', value: '' }]);
  }, []);

  const removeField = useCallback((id: string) => {
    setFields((prev) => prev.filter((f) => f.id !== id));
  }, []);

  const updateField = useCallback((id: string, key: keyof FormField, val: string) => {
    formDrivenRef.current = true;
    setFields((prev) =>
      prev.map((f) => (f.id === id ? { ...f, [key]: val } : f))
    );
  }, []);

  const handleSubmit = useCallback(async () => {
    setError('');
    try {
      const docs = parseDocuments(jsonText);
      await addDocuments.mutateAsync(docs);
      setJsonText('');
      setFileName('');
      setFields([]);
      setFormInitialized(false);
      onOpenChange(false);
    } catch (err) {
      if (err instanceof SyntaxError) {
        setError('Invalid JSON: ' + err.message);
      } else if (err instanceof Error) {
        setError(err.message);
      }
    }
  }, [jsonText, addDocuments, onOpenChange]);

  const handleFileRead = useCallback((file: File) => {
    setError('');
    if (!file.name.endsWith('.json')) {
      setError('Only .json files are supported');
      return;
    }
    if (file.size > 50 * 1024 * 1024) {
      setError('File too large (max 50MB)');
      return;
    }
    setFileName(file.name);
    const reader = new FileReader();
    reader.onload = (e) => {
      const text = e.target?.result as string;
      setJsonText(text);
    };
    reader.onerror = () => setError('Failed to read file');
    reader.readAsText(file);
  }, []);

  const handleDrop = useCallback(
    (e: DragEvent) => {
      e.preventDefault();
      setDragOver(false);
      const file = e.dataTransfer.files[0];
      if (file) handleFileRead(file);
    },
    [handleFileRead]
  );

  const handleFileInput = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      if (file) handleFileRead(file);
    },
    [handleFileRead]
  );

  const handleCopy = useCallback(async () => {
    if (!jsonText.trim()) return;
    try {
      await navigator.clipboard.writeText(jsonText);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // fallback
    }
  }, [jsonText]);

  const docCount = (() => {
    try {
      if (!jsonText.trim()) return 0;
      const docs = parseDocuments(jsonText);
      return docs.length;
    } catch {
      return 0;
    }
  })();

  const showFooterSubmit = activeTab !== 'sample';

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl max-h-[85vh] flex flex-col">
        <DialogHeader>
          <DialogTitle>Add Documents to "{indexName}"</DialogTitle>
          <DialogDescription>
            Add documents via JSON, upload a file, or load sample data.
          </DialogDescription>
        </DialogHeader>

        <Tabs value={activeTab} onValueChange={setActiveTab} className="flex-1 min-h-0 flex flex-col">
          <TabsList>
            <TabsTrigger value="json">
              <FileJson className="h-4 w-4 mr-1.5" />
              JSON
            </TabsTrigger>
            <TabsTrigger value="file">
              <Upload className="h-4 w-4 mr-1.5" />
              Upload
            </TabsTrigger>
            <TabsTrigger value="sample">
              <Database className="h-4 w-4 mr-1.5" />
              Sample Data
            </TabsTrigger>
          </TabsList>

          <TabsContent value="json" className="flex-1 min-h-0 mt-3 space-y-3">
            {/* Form builder */}
            <div className="space-y-2">
              <p className="text-xs text-muted-foreground font-medium">Build a document</p>
              <div className="space-y-1.5 max-h-36 overflow-y-auto pr-1">
                {fields.map((field) => (
                  <div key={field.id} className="flex items-center gap-1.5">
                    <Input
                      placeholder="Field name"
                      value={field.name}
                      onChange={(e) => updateField(field.id, 'name', e.target.value)}
                      className="w-28 text-sm h-8"
                    />
                    <select
                      value={field.type}
                      onChange={(e) => updateField(field.id, 'type', e.target.value)}
                      className="h-8 rounded-md border border-input bg-background px-1.5 text-xs"
                    >
                      <option value="text">Text</option>
                      <option value="number">Number</option>
                      <option value="boolean">Boolean</option>
                    </select>
                    {field.type === 'boolean' ? (
                      <select
                        value={field.value}
                        onChange={(e) => updateField(field.id, 'value', e.target.value)}
                        className="h-8 flex-1 rounded-md border border-input bg-background px-1.5 text-xs"
                      >
                        <option value="">—</option>
                        <option value="true">true</option>
                        <option value="false">false</option>
                      </select>
                    ) : (
                      <Input
                        placeholder="Value"
                        type={field.type === 'number' ? 'number' : 'text'}
                        value={field.value}
                        onChange={(e) => updateField(field.id, 'value', e.target.value)}
                        className="flex-1 text-sm h-8"
                      />
                    )}
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => removeField(field.id)}
                      className="px-1.5 h-8 text-muted-foreground hover:text-destructive"
                    >
                      <Trash2 className="h-3.5 w-3.5" />
                    </Button>
                  </div>
                ))}
              </div>
              <Button variant="outline" size="sm" onClick={addField} className="h-7 text-xs">
                <Plus className="h-3.5 w-3.5 mr-1" />
                Add Field
              </Button>
            </div>

            {/* JSON editor with copy button */}
            <div className="relative">
              <div className="flex items-center justify-between mb-1">
                <p className="text-xs text-muted-foreground font-medium">JSON</p>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={handleCopy}
                  disabled={!jsonText.trim()}
                  className="h-6 px-2 text-xs text-muted-foreground"
                >
                  {copied ? (
                    <>
                      <Check className="h-3 w-3 mr-1" />
                      Copied
                    </>
                  ) : (
                    <>
                      <Copy className="h-3 w-3 mr-1" />
                      Copy
                    </>
                  )}
                </Button>
              </div>
              <textarea
                className="w-full h-44 p-3 text-sm font-mono rounded-md border border-input bg-background resize-y focus:outline-none focus:ring-2 focus:ring-ring"
                placeholder={'[\n  { "objectID": "1", "title": "My document", "body": "..." }\n]'}
                value={jsonText}
                onChange={(e) => {
                  formDrivenRef.current = false;
                  setJsonText(e.target.value);
                  setError('');
                }}
              />
            </div>
          </TabsContent>

          <TabsContent value="file" className="mt-3">
            <div
              onDragOver={(e) => { e.preventDefault(); setDragOver(true); }}
              onDragLeave={() => setDragOver(false)}
              onDrop={handleDrop}
              onClick={() => fileInputRef.current?.click()}
              className={`flex flex-col items-center justify-center gap-3 p-10 rounded-md border-2 border-dashed cursor-pointer transition-colors ${
                dragOver
                  ? 'border-primary bg-primary/5'
                  : 'border-border hover:border-muted-foreground'
              }`}
            >
              <Upload className="h-8 w-8 text-muted-foreground" />
              <div className="text-center">
                <p className="text-sm font-medium">
                  {fileName || 'Drop a .json file here or click to browse'}
                </p>
                <p className="text-xs text-muted-foreground mt-1">Max 50MB</p>
              </div>
              <input
                ref={fileInputRef}
                type="file"
                accept=".json"
                onChange={handleFileInput}
                className="hidden"
              />
            </div>
            {fileName && jsonText && (
              <p className="text-sm text-muted-foreground mt-2">
                Loaded {fileName} — {docCount} document(s) detected
              </p>
            )}
          </TabsContent>

          <TabsContent value="sample" className="mt-3">
            <SampleDataTabContent
              indexName={indexName}
              onSuccess={() => onOpenChange(false)}
            />
          </TabsContent>
        </Tabs>

        {error && (
          <p className="text-sm text-destructive">{error}</p>
        )}

        {showFooterSubmit && (
          <DialogFooter className="flex items-center justify-between">
            <span className="text-sm text-muted-foreground">
              {docCount > 0 ? `${docCount} document(s)` : ''}
            </span>
            <div className="flex gap-2">
              <Button variant="outline" onClick={() => onOpenChange(false)} disabled={addDocuments.isPending}>
                Cancel
              </Button>
              <Button
                onClick={handleSubmit}
                disabled={addDocuments.isPending || !jsonText.trim()}
              >
                {addDocuments.isPending ? 'Adding...' : `Add Document${docCount !== 1 ? 's' : ''}`}
              </Button>
            </div>
          </DialogFooter>
        )}
      </DialogContent>
    </Dialog>
  );
}
