import { memo, useCallback, useMemo, useState } from 'react';
import { RotateCcw, Loader2, X, CheckCircle2 } from 'lucide-react';
import { Card } from '@/components/ui/card';
import { Label } from '@/components/ui/label';
import { Input } from '@/components/ui/input';
import { Textarea } from '@/components/ui/textarea';
import { Switch } from '@/components/ui/switch';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Skeleton } from '@/components/ui/skeleton';
import { ConfirmDialog } from '@/components/ui/confirm-dialog';
import { useIndexFields, type FieldInfo } from '@/hooks/useIndexFields';
import { useReindex } from '@/hooks/useReindex';
import { cn } from '@/lib/utils';
import type { IndexSettings } from '@/lib/types';

interface SettingsFormProps {
  settings: Partial<IndexSettings>;
  savedSettings?: Partial<IndexSettings>;
  onChange: (updates: Partial<IndexSettings>) => void;
  indexName: string;
}

interface SettingSectionProps {
  title: string;
  description?: string;
  warning?: string;
  warningDetail?: string;
  warningAction?: React.ReactNode;
  children: React.ReactNode;
}

const SettingSection = memo(function SettingSection({
  title,
  description,
  warning,
  warningDetail,
  warningAction,
  children,
}: SettingSectionProps) {
  return (
    <Card className="p-6 space-y-4">
      <div>
        <div className="flex items-center gap-2 flex-wrap">
          <h3 className="text-lg font-semibold">{title}</h3>
          {warning && (
            <Badge variant="destructive" className="text-xs">
              {warning}
            </Badge>
          )}
          {warningAction}
        </div>
        {description && (
          <p className="text-sm text-muted-foreground mt-1">{description}</p>
        )}
        {warningDetail && (
          <p className="text-xs text-muted-foreground mt-2">{warningDetail}</p>
        )}
      </div>
      <div className="space-y-4">{children}</div>
    </Card>
  );
});

interface FieldProps {
  label: string;
  description?: string;
  children: React.ReactNode;
}

const Field = memo(function Field({ label, description, children }: FieldProps) {
  return (
    <div className="space-y-2">
      <Label>{label}</Label>
      {description && (
        <p className="text-xs text-muted-foreground">{description}</p>
      )}
      {children}
    </div>
  );
});

interface FieldChipsProps {
  availableFields: FieldInfo[];
  selectedValues: string[];
  onToggle: (fieldName: string) => void;
  isLoading?: boolean;
}

const FieldChips = memo(function FieldChips({
  availableFields,
  selectedValues,
  onToggle,
  isLoading,
}: FieldChipsProps) {
  if (isLoading) {
    return (
      <div className="flex gap-1.5">
        <Skeleton className="h-6 w-16 rounded-full" />
        <Skeleton className="h-6 w-20 rounded-full" />
        <Skeleton className="h-6 w-14 rounded-full" />
      </div>
    );
  }
  if (!availableFields.length) return null;

  return (
    <div className="flex flex-wrap gap-1.5">
      {availableFields.map((field) => {
        const isSelected = selectedValues.includes(field.name);
        return (
          <button
            key={field.name}
            type="button"
            onClick={() => onToggle(field.name)}
            className={cn(
              'inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium transition-colors',
              isSelected
                ? 'bg-primary text-primary-foreground'
                : 'bg-muted text-muted-foreground hover:bg-muted/80 border border-border'
            )}
          >
            {field.name}
            {isSelected && <X className="h-3 w-3 ml-1" />}
          </button>
        );
      })}
    </div>
  );
});

export const SettingsForm = memo(function SettingsForm({
  settings,
  savedSettings,
  onChange,
  indexName,
}: SettingsFormProps) {
  const { data: fields = [], isLoading: fieldsLoading } = useIndexFields(indexName);
  const reindex = useReindex(indexName);
  const [showReindexConfirm, setShowReindexConfirm] = useState(false);

  // Compare current facet settings against the saved (server) values to determine
  // whether a reindex is needed. If they differ, the user changed facets since last save/reindex.
  const facetsNeedReindex = useMemo(() => {
    const current = [...(settings.attributesForFaceting || [])].sort();
    const saved = [...(savedSettings?.attributesForFaceting || [])].sort();
    return JSON.stringify(current) !== JSON.stringify(saved);
  }, [settings.attributesForFaceting, savedSettings?.attributesForFaceting]);

  const handleArrayChange = useCallback(
    (key: keyof IndexSettings, value: string) => {
      const array = value
        .split(',')
        .map((s) => s.trim())
        .filter(Boolean);
      onChange({ [key]: array.length > 0 ? array : undefined });
    },
    [onChange]
  );

  const handleNumberChange = useCallback(
    (key: keyof IndexSettings, value: string) => {
      const num = parseInt(value, 10);
      onChange({ [key]: isNaN(num) ? undefined : num });
    },
    [onChange]
  );

  const handleBooleanChange = useCallback(
    (key: keyof IndexSettings, checked: boolean) => {
      onChange({ [key]: checked });
    },
    [onChange]
  );

  const handleFieldToggle = useCallback(
    (key: keyof IndexSettings, fieldName: string) => {
      const current = (settings[key] as string[] | undefined) || [];
      const updated = current.includes(fieldName)
        ? current.filter((f) => f !== fieldName)
        : [...current, fieldName];
      onChange({ [key]: updated.length > 0 ? updated : undefined });
    },
    [settings, onChange]
  );

  return (
    <div className="space-y-6">
      {/* Search Behavior */}
      <SettingSection
        title="Search Behavior"
        description="Configure how search queries are processed"
      >
        <Field
          label="Searchable Attributes"
          description="Click fields to toggle, or type comma-separated values below"
        >
          <FieldChips
            availableFields={fields}
            selectedValues={settings.searchableAttributes || []}
            onToggle={(name) => handleFieldToggle('searchableAttributes', name)}
            isLoading={fieldsLoading}
          />
          <Textarea
            value={settings.searchableAttributes?.join(', ') || ''}
            onChange={(e) =>
              handleArrayChange('searchableAttributes', e.target.value)
            }
            placeholder="title, description, tags"
            rows={2}
          />
        </Field>

        <Field
          label="Hits Per Page"
          description="Default number of results per page"
        >
          <Input
            type="number"
            min="1"
            max="1000"
            value={settings.hitsPerPage || ''}
            onChange={(e) => handleNumberChange('hitsPerPage', e.target.value)}
            placeholder="20"
          />
        </Field>
      </SettingSection>

      {/* Faceting */}
      <SettingSection
        title="Faceting"
        description="Configure faceted search and filtering"
        warning={facetsNeedReindex ? 'Reindex needed' : undefined}
        warningDetail={
          facetsNeedReindex
            ? 'Facet attributes have changed. Save your settings, then re-index so existing documents pick up the new facets.'
            : undefined
        }
        warningAction={
          facetsNeedReindex ? (
            <Button
              variant="outline"
              size="sm"
              onClick={() => setShowReindexConfirm(true)}
              disabled={reindex.isPending}
              className="h-6 text-xs"
            >
              {reindex.isPending ? (
                <>
                  <Loader2 className="h-3 w-3 mr-1 animate-spin" />
                  Re-indexing...
                </>
              ) : (
                <>
                  <RotateCcw className="h-3 w-3 mr-1" />
                  Re-index now
                </>
              )}
            </Button>
          ) : (
            <span className="inline-flex items-center gap-1 text-xs text-muted-foreground">
              <CheckCircle2 className="h-3.5 w-3.5 text-green-500" />
              Up to date
            </span>
          )
        }
      >
        <Field
          label="Attributes For Faceting"
          description="Click fields to toggle, or type comma-separated values below"
        >
          <FieldChips
            availableFields={fields}
            selectedValues={settings.attributesForFaceting || []}
            onToggle={(name) => handleFieldToggle('attributesForFaceting', name)}
            isLoading={fieldsLoading}
          />
          <Textarea
            value={settings.attributesForFaceting?.join(', ') || ''}
            onChange={(e) =>
              handleArrayChange('attributesForFaceting', e.target.value)
            }
            placeholder="category, brand, color"
            rows={2}
          />
        </Field>
      </SettingSection>

      {/* Ranking */}
      <SettingSection
        title="Ranking & Sorting"
        description="Configure how results are ranked"
      >
        <Field
          label="Ranking Criteria"
          description="Comma-separated list of ranking criteria (typo, geo, words, filters, proximity, attribute, exact)"
        >
          <Textarea
            value={settings.ranking?.join(', ') || ''}
            onChange={(e) => handleArrayChange('ranking', e.target.value)}
            placeholder="typo, geo, words, filters, proximity, attribute, exact"
            rows={3}
          />
        </Field>

        <Field
          label="Custom Ranking"
          description="Comma-separated list of custom ranking attributes (use asc() or desc())"
        >
          <Textarea
            value={settings.customRanking?.join(', ') || ''}
            onChange={(e) => handleArrayChange('customRanking', e.target.value)}
            placeholder="desc(popularity), asc(price)"
            rows={2}
          />
        </Field>
      </SettingSection>

      {/* Display */}
      <SettingSection
        title="Display & Highlighting"
        description="Configure what data is returned and highlighted"
      >
        <Field
          label="Attributes To Retrieve"
          description="Click fields to toggle, or type comma-separated values below"
        >
          <FieldChips
            availableFields={fields}
            selectedValues={settings.attributesToRetrieve || []}
            onToggle={(name) => handleFieldToggle('attributesToRetrieve', name)}
            isLoading={fieldsLoading}
          />
          <Textarea
            value={settings.attributesToRetrieve?.join(', ') || ''}
            onChange={(e) =>
              handleArrayChange('attributesToRetrieve', e.target.value)
            }
            placeholder="title, description, image, price"
            rows={2}
          />
        </Field>

        <Field
          label="Attributes To Highlight"
          description="Click fields to toggle, or type comma-separated values below"
        >
          <FieldChips
            availableFields={fields}
            selectedValues={settings.attributesToHighlight || []}
            onToggle={(name) => handleFieldToggle('attributesToHighlight', name)}
            isLoading={fieldsLoading}
          />
          <Textarea
            value={settings.attributesToHighlight?.join(', ') || ''}
            onChange={(e) =>
              handleArrayChange('attributesToHighlight', e.target.value)
            }
            placeholder="title, description"
            rows={2}
          />
        </Field>

        <div className="grid grid-cols-2 gap-4">
          <Field label="Highlight Pre Tag" description="Opening tag for highlights">
            <Input
              value={settings.highlightPreTag || ''}
              onChange={(e) =>
                onChange({ highlightPreTag: e.target.value || undefined })
              }
              placeholder="<em>"
            />
          </Field>

          <Field label="Highlight Post Tag" description="Closing tag for highlights">
            <Input
              value={settings.highlightPostTag || ''}
              onChange={(e) =>
                onChange({ highlightPostTag: e.target.value || undefined })
              }
              placeholder="</em>"
            />
          </Field>
        </div>
      </SettingSection>

      {/* Advanced */}
      <SettingSection title="Advanced" description="Advanced search configuration">
        <Field
          label="Remove Stop Words"
          description="Enable stop words removal for better search relevance"
        >
          <Switch
            checked={settings.removeStopWords === true}
            onCheckedChange={(checked) =>
              handleBooleanChange('removeStopWords', checked)
            }
          />
        </Field>

        <Field
          label="Ignore Plurals"
          description="Treat singular and plural forms as equivalent"
        >
          <Switch
            checked={settings.ignorePlurals === true}
            onCheckedChange={(checked) =>
              handleBooleanChange('ignorePlurals', checked)
            }
          />
        </Field>

        <div className="grid grid-cols-2 gap-4">
          <Field
            label="Min Word Size for 1 Typo"
            description="Minimum word length to allow 1 typo"
          >
            <Input
              type="number"
              min="1"
              max="10"
              value={settings.minWordSizefor1Typo || ''}
              onChange={(e) =>
                handleNumberChange('minWordSizefor1Typo', e.target.value)
              }
              placeholder="4"
            />
          </Field>

          <Field
            label="Min Word Size for 2 Typos"
            description="Minimum word length to allow 2 typos"
          >
            <Input
              type="number"
              min="1"
              max="20"
              value={settings.minWordSizefor2Typos || ''}
              onChange={(e) =>
                handleNumberChange('minWordSizefor2Typos', e.target.value)
              }
              placeholder="8"
            />
          </Field>
        </div>
      </SettingSection>

      <ConfirmDialog
        open={showReindexConfirm}
        onOpenChange={setShowReindexConfirm}
        title="Re-index All Documents"
        description={
          <>
            This will clear and re-add all documents in{' '}
            <code className="font-mono text-sm bg-muted px-1 py-0.5 rounded">
              {indexName}
            </code>{' '}
            so they are indexed with the current settings. This may take a moment
            for large indexes.
          </>
        }
        confirmLabel="Re-index"
        onConfirm={() => {
          reindex.mutate(undefined, {
            onSettled: () => setShowReindexConfirm(false),
          });
        }}
        isPending={reindex.isPending}
      />
    </div>
  );
});
