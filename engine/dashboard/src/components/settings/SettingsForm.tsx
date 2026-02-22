import { memo, useCallback, useMemo, useState } from 'react';
import { RotateCcw, Loader2, CheckCircle2 } from 'lucide-react';
import { Input } from '@/components/ui/input';
import { Textarea } from '@/components/ui/textarea';
import { Switch } from '@/components/ui/switch';
import { Button } from '@/components/ui/button';
import { ConfirmDialog } from '@/components/ui/confirm-dialog';
import { SettingSection, Field, FieldChips } from './shared';
import { SearchModeSection } from './SearchModeSection';
import { EmbedderPanel } from './EmbedderPanel';
import { useIndexFields } from '@/hooks/useIndexFields';
import { useReindex } from '@/hooks/useReindex';
import type { IndexSettings } from '@/lib/types';

interface SettingsFormProps {
  settings: Partial<IndexSettings>;
  savedSettings?: Partial<IndexSettings>;
  onChange: (updates: Partial<IndexSettings>) => void;
  indexName: string;
}

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

      {/* Search Mode */}
      <SearchModeSection
        mode={settings.mode}
        embedders={settings.embedders}
        onChange={onChange}
      />

      {/* Embedders */}
      <EmbedderPanel
        embedders={settings.embedders}
        onChange={onChange}
      />

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
