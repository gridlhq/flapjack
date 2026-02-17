import { useState, useEffect, useCallback } from 'react';
import { useIndexFields, type FieldInfo } from '@/hooks/useIndexFields';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { Plus, Trash2 } from 'lucide-react';

interface FormField {
  id: string;
  name: string;
  type: 'text' | 'number' | 'boolean';
  value: string;
}

interface FormTabContentProps {
  indexName: string;
  onDocumentReady: (doc: Record<string, unknown> | null) => void;
}

function buildDocument(fields: FormField[]): Record<string, unknown> | null {
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

let nextId = 0;
function makeId() {
  return `field_${++nextId}`;
}

function toFormFields(detected: FieldInfo[]): FormField[] {
  return detected.map((f) => ({
    id: makeId(),
    name: f.name,
    type: f.type,
    value: '',
  }));
}

export function FormTabContent({ indexName, onDocumentReady }: FormTabContentProps) {
  const { data: detectedFields } = useIndexFields(indexName);
  const [fields, setFields] = useState<FormField[]>([]);
  const [initialized, setInitialized] = useState(false);

  // Populate fields from detected schema once
  useEffect(() => {
    if (!initialized && detectedFields) {
      if (detectedFields.length > 0) {
        setFields(toFormFields(detectedFields));
      } else {
        // Empty index — start with one blank field
        setFields([{ id: makeId(), name: '', type: 'text', value: '' }]);
      }
      setInitialized(true);
    }
  }, [detectedFields, initialized]);

  // Push document up whenever fields change
  useEffect(() => {
    onDocumentReady(buildDocument(fields));
  }, [fields, onDocumentReady]);

  const addField = useCallback(() => {
    setFields((prev) => [...prev, { id: makeId(), name: '', type: 'text', value: '' }]);
  }, []);

  const removeField = useCallback((id: string) => {
    setFields((prev) => prev.filter((f) => f.id !== id));
  }, []);

  const updateField = useCallback((id: string, key: keyof FormField, val: string) => {
    setFields((prev) =>
      prev.map((f) => (f.id === id ? { ...f, [key]: val } : f))
    );
  }, []);

  const preview = buildDocument(fields);

  return (
    <div className="space-y-3">
      {/* Field rows */}
      <div className="space-y-2 max-h-52 overflow-y-auto pr-1">
        {fields.map((field) => (
          <div key={field.id} className="flex items-center gap-2">
            <Input
              placeholder="Field name"
              value={field.name}
              onChange={(e) => updateField(field.id, 'name', e.target.value)}
              className="w-32 text-sm"
            />
            <select
              value={field.type}
              onChange={(e) => updateField(field.id, 'type', e.target.value)}
              className="h-10 rounded-md border border-input bg-background px-2 text-sm"
            >
              <option value="text">Text</option>
              <option value="number">Number</option>
              <option value="boolean">Boolean</option>
            </select>
            {field.type === 'boolean' ? (
              <select
                value={field.value}
                onChange={(e) => updateField(field.id, 'value', e.target.value)}
                className="h-10 flex-1 rounded-md border border-input bg-background px-2 text-sm"
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
                className="flex-1 text-sm"
              />
            )}
            <Button
              variant="ghost"
              size="sm"
              onClick={() => removeField(field.id)}
              className="px-2 text-muted-foreground hover:text-destructive"
            >
              <Trash2 className="h-4 w-4" />
            </Button>
          </div>
        ))}
      </div>

      <Button variant="outline" size="sm" onClick={addField}>
        <Plus className="h-4 w-4 mr-1" />
        Add Field
      </Button>

      {/* JSON Preview */}
      {preview && (
        <div className="space-y-1">
          <p className="text-xs text-muted-foreground font-medium">Preview</p>
          <pre className="text-xs font-mono bg-muted p-2 rounded-md max-h-28 overflow-y-auto">
            {JSON.stringify(preview, null, 2)}
          </pre>
        </div>
      )}
    </div>
  );
}
