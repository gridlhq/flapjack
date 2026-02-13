import { memo, useState, useMemo, lazy, Suspense } from 'react';
import { ChevronDown, ChevronRight, Copy, Check, Trash2, Code } from 'lucide-react';
import { Card } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import type { Document } from '@/lib/types';

// Lazy load Monaco Editor for performance
const Editor = lazy(() =>
  import('@monaco-editor/react').then((module) => ({
    default: module.default,
  }))
);

const PREVIEW_FIELD_COUNT = 6;

interface HighlightResultValue {
  value: string;
  matchLevel: 'none' | 'partial' | 'full';
  matchedWords?: string[];
  fullyHighlighted?: boolean;
}

type HighlightResult = Record<string, HighlightResultValue | HighlightResultValue[] | Record<string, unknown>>;

interface DocumentCardProps {
  document: Document;
  fieldOrder?: string[];
  onDelete?: (objectID: string) => void;
  isDeleting?: boolean;
  onClick?: () => void;
}

/**
 * Get the highlighted HTML string for a field, falling back to plain value.
 * Returns an object with { html, hasMatch } so we can style matched fields.
 */
function getFieldDisplay(
  key: string,
  rawValue: unknown,
  highlightResult?: HighlightResult
): { html: string; hasMatch: boolean } {
  const hr = highlightResult?.[key];

  // Handle single highlight result
  if (hr && typeof hr === 'object' && 'value' in hr) {
    const single = hr as HighlightResultValue;
    return {
      html: single.value,
      hasMatch: single.matchLevel !== 'none',
    };
  }

  // Handle array highlight results - join them
  if (Array.isArray(hr)) {
    const items = hr as HighlightResultValue[];
    return {
      html: items.map((item) => item.value).join(', '),
      hasMatch: items.some((item) => item.matchLevel !== 'none'),
    };
  }

  // Fallback: plain value
  if (rawValue === null || rawValue === undefined) {
    return { html: '<span class="text-muted-foreground italic">null</span>', hasMatch: false };
  }
  if (typeof rawValue === 'object') {
    // Escape HTML in JSON strings
    const json = JSON.stringify(rawValue);
    return { html: escapeHtml(json), hasMatch: false };
  }
  return { html: escapeHtml(String(rawValue)), hasMatch: false };
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

export const DocumentCard = memo(function DocumentCard({
  document,
  fieldOrder,
  onDelete,
  isDeleting,
  onClick,
}: DocumentCardProps) {
  const [showAllFields, setShowAllFields] = useState(false);
  const [showJson, setShowJson] = useState(false);
  const [isCopied, setIsCopied] = useState(false);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(JSON.stringify(document, null, 2));
      setIsCopied(true);
      setTimeout(() => setIsCopied(false), 2000);
    } catch (err) {
      console.error('Failed to copy:', err);
    }
  };

  const { objectID, _highlightResult, ...fieldData } = document;
  const highlightResult = _highlightResult as HighlightResult | undefined;

  // Use stable field order from parent if provided, falling back to this doc's own keys.
  // This ensures every card in a result set shows fields in the same order.
  const allKeys = useMemo(() => {
    if (!fieldOrder) return Object.keys(fieldData);
    const docKeys = new Set(Object.keys(fieldData));
    // Ordered keys present in this doc, then any extras not in the canonical order
    const ordered = fieldOrder.filter((k) => docKeys.has(k));
    for (const k of docKeys) {
      if (!fieldOrder.includes(k)) ordered.push(k);
    }
    return ordered;
  }, [fieldData, fieldOrder]);
  const previewKeys = allKeys.slice(0, PREVIEW_FIELD_COUNT);
  const extraKeys = allKeys.slice(PREVIEW_FIELD_COUNT);
  const visibleKeys = showAllFields ? allKeys : previewKeys;

  return (
    <Card
      className={`overflow-hidden${onClick ? ' cursor-pointer hover:ring-1 hover:ring-primary/50 transition-shadow' : ''}`}
      data-testid="document-card"
      onClick={onClick ? (e) => {
        // Don't fire click analytics when clicking action buttons
        if ((e.target as HTMLElement).closest('button, a')) return;
        onClick();
      } : undefined}
    >
      <div className="p-4">
        {/* Header with ID and actions */}
        <div className="flex items-start justify-between gap-4 mb-3">
          <div className="flex items-center gap-2 min-w-0">
            <Badge variant="outline" className="font-mono text-xs">
              {objectID}
            </Badge>
          </div>

          <div className="flex items-center gap-1 shrink-0">
            <Button
              variant="ghost"
              size="sm"
              className="h-7 px-2 text-xs"
              onClick={() => setShowJson(!showJson)}
              title="Toggle JSON view"
            >
              <Code className="h-3 w-3 mr-1" />
              JSON
            </Button>
            <Button
              variant="ghost"
              size="sm"
              className="h-7 px-2 text-xs"
              onClick={handleCopy}
            >
              {isCopied ? (
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
            {onDelete && (
              <Button
                variant="ghost"
                size="sm"
                className="h-7 px-2 text-muted-foreground hover:text-destructive"
                onClick={() => onDelete(objectID)}
                disabled={isDeleting}
                title="Delete document"
              >
                <Trash2 className="h-3.5 w-3.5" />
              </Button>
            )}
          </div>
        </div>

        {/* Field display with highlighting */}
        {!showJson && (
          <div className="space-y-1.5 text-sm">
            {visibleKeys.map((key) => {
              const { html } = getFieldDisplay(key, fieldData[key], highlightResult);
              return (
                <div key={key} className="flex gap-2 leading-relaxed">
                  <span className="font-medium text-muted-foreground min-w-[100px] shrink-0">
                    {key}:
                  </span>
                  <span
                    className="min-w-0 break-words [&>em]:bg-yellow-200 dark:[&>em]:bg-yellow-800 [&>em]:not-italic [&>em]:font-medium [&>em]:rounded-sm [&>em]:px-0.5"
                    dangerouslySetInnerHTML={{ __html: html }}
                  />
                </div>
              );
            })}

            {/* Expand/collapse extra fields */}
            {extraKeys.length > 0 && (
              <button
                type="button"
                onClick={() => setShowAllFields(!showAllFields)}
                className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors pt-1"
              >
                {showAllFields ? (
                  <>
                    <ChevronDown className="h-3 w-3" />
                    Show less
                  </>
                ) : (
                  <>
                    <ChevronRight className="h-3 w-3" />
                    +{extraKeys.length} more field{extraKeys.length !== 1 ? 's' : ''}
                  </>
                )}
              </button>
            )}
          </div>
        )}

        {/* Full JSON viewer */}
        {showJson && (
          <Suspense
            fallback={
              <div className="h-64 flex items-center justify-center text-muted-foreground">
                Loading editor...
              </div>
            }
          >
            <div className="border rounded-md overflow-hidden">
              <Editor
                height="400px"
                defaultLanguage="json"
                value={JSON.stringify(document, null, 2)}
                options={{
                  readOnly: true,
                  minimap: { enabled: false },
                  scrollBeyondLastLine: false,
                  lineNumbers: 'off',
                  folding: true,
                  fontSize: 13,
                }}
                theme="vs-dark"
              />
            </div>
          </Suspense>
        )}
      </div>
    </Card>
  );
});
