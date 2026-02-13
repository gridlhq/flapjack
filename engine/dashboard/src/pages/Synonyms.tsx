import { useState, useCallback } from 'react';
import { useParams, Link } from 'react-router-dom';
import { ChevronLeft, Plus, Trash2, Search, X, ArrowRight } from 'lucide-react';
import { Card } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import { Label } from '@/components/ui/label';
import { Skeleton } from '@/components/ui/skeleton';
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter, DialogDescription } from '@/components/ui/dialog';
import { useSynonyms, useSaveSynonym, useDeleteSynonym, useClearSynonyms } from '@/hooks/useSynonyms';
import type { Synonym, SynonymType } from '@/lib/types';

const SYNONYM_TYPE_LABELS: Record<SynonymType, string> = {
  synonym: 'Multi-way',
  onewaysynonym: 'One-way',
  altcorrection1: 'Alt. Correction 1',
  altcorrection2: 'Alt. Correction 2',
  placeholder: 'Placeholder',
};

function getSynonymDescription(synonym: Synonym): string {
  switch (synonym.type) {
    case 'synonym':
      return synonym.synonyms.join(' = ');
    case 'onewaysynonym':
      return `${synonym.input} → ${synonym.synonyms.join(', ')}`;
    case 'altcorrection1':
    case 'altcorrection2':
      return `${synonym.word} → ${synonym.corrections.join(', ')}`;
    case 'placeholder':
      return `{${synonym.placeholder}} → ${synonym.replacements.join(', ')}`;
  }
}

function getEmptySynonym(type: SynonymType): Synonym {
  const objectID = `syn-${Date.now()}`;
  switch (type) {
    case 'synonym':
      return { type: 'synonym', objectID, synonyms: ['', ''] };
    case 'onewaysynonym':
      return { type: 'onewaysynonym', objectID, input: '', synonyms: [''] };
    case 'altcorrection1':
      return { type: 'altcorrection1', objectID, word: '', corrections: [''] };
    case 'altcorrection2':
      return { type: 'altcorrection2', objectID, word: '', corrections: [''] };
    case 'placeholder':
      return { type: 'placeholder', objectID, placeholder: '', replacements: [''] };
  }
}

export function Synonyms() {
  const { indexName } = useParams<{ indexName: string }>();
  const [searchQuery, setSearchQuery] = useState('');
  const [editingSynonym, setEditingSynonym] = useState<Synonym | null>(null);
  const [isCreating, setIsCreating] = useState(false);

  const { data, isLoading } = useSynonyms({
    indexName: indexName || '',
    query: searchQuery,
  });

  const saveSynonym = useSaveSynonym(indexName || '');
  const deleteSynonym = useDeleteSynonym(indexName || '');
  const clearSynonyms = useClearSynonyms(indexName || '');

  const handleSave = useCallback(async (synonym: Synonym) => {
    await saveSynonym.mutateAsync(synonym);
    setEditingSynonym(null);
    setIsCreating(false);
  }, [saveSynonym]);

  const handleDelete = useCallback(async (objectID: string) => {
    if (!confirm(`Delete synonym "${objectID}"?`)) return;
    await deleteSynonym.mutateAsync(objectID);
  }, [deleteSynonym]);

  const handleClearAll = useCallback(async () => {
    if (!confirm('Delete ALL synonyms for this index? This cannot be undone.')) return;
    await clearSynonyms.mutateAsync();
  }, [clearSynonyms]);

  const handleCreate = useCallback((type: SynonymType) => {
    setEditingSynonym(getEmptySynonym(type));
    setIsCreating(true);
  }, []);

  if (!indexName) {
    return (
      <Card className="p-8 text-center">
        <h3 className="text-lg font-semibold mb-2">No index selected</h3>
        <Link to="/overview"><Button>Go to Overview</Button></Link>
      </Card>
    );
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Link to={`/index/${encodeURIComponent(indexName)}`}>
            <Button variant="ghost" size="sm">
              <ChevronLeft className="h-4 w-4 mr-1" />
              {indexName}
            </Button>
          </Link>
          <span className="text-muted-foreground">/</span>
          <h2 className="text-xl font-semibold">Synonyms</h2>
          {data && (
            <Badge variant="secondary" className="ml-2" data-testid="synonym-count">{data.nbHits}</Badge>
          )}
        </div>
        <div className="flex items-center gap-2">
          {data && data.nbHits > 0 && (
            <Button variant="outline" size="sm" onClick={handleClearAll}>
              <Trash2 className="h-4 w-4 mr-1" />
              Clear All
            </Button>
          )}
          <Button onClick={() => handleCreate('synonym')}>
            <Plus className="h-4 w-4 mr-1" />
            Add Synonym
          </Button>
        </div>
      </div>

      {/* Search */}
      <div className="relative">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
        <Input
          placeholder="Search synonyms..."
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          className="pl-9"
        />
      </div>

      {/* Synonym list */}
      {isLoading ? (
        <div className="space-y-3">
          {[1, 2, 3].map((i) => (
            <Card key={i} className="p-4">
              <Skeleton className="h-5 w-full" />
            </Card>
          ))}
        </div>
      ) : !data || data.nbHits === 0 ? (
        <Card className="p-8 text-center">
          <h3 className="text-lg font-semibold mb-2">No synonyms</h3>
          <p className="text-sm text-muted-foreground mb-4">
            Synonyms help users find results even when they use different words.
            For example: "hoodie" = "sweatshirt" = "pullover".
          </p>
          <div className="flex items-center justify-center gap-2">
            {(['synonym', 'onewaysynonym'] as SynonymType[]).map((type) => (
              <Button key={type} variant="outline" onClick={() => handleCreate(type)}>
                <Plus className="h-4 w-4 mr-1" />
                {SYNONYM_TYPE_LABELS[type]}
              </Button>
            ))}
          </div>
        </Card>
      ) : (
        <div className="space-y-2" data-testid="synonyms-list">
          {data.hits.map((synonym) => (
            <Card key={synonym.objectID} className="p-4 hover:bg-accent/50 transition-colors">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-3 min-w-0">
                  <Badge variant="outline" className="shrink-0 text-xs">
                    {SYNONYM_TYPE_LABELS[synonym.type]}
                  </Badge>
                  <span className="text-sm font-mono truncate">
                    {getSynonymDescription(synonym)}
                  </span>
                </div>
                <div className="flex items-center gap-1 shrink-0">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => { setEditingSynonym({ ...synonym }); setIsCreating(false); }}
                  >
                    Edit
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => handleDelete(synonym.objectID)}
                    disabled={deleteSynonym.isPending}
                    aria-label="Delete"
                  >
                    <Trash2 className="h-4 w-4 text-destructive" />
                  </Button>
                </div>
              </div>
            </Card>
          ))}
        </div>
      )}

      {/* Edit / Create Dialog */}
      {editingSynonym && (
        <SynonymEditor
          synonym={editingSynonym}
          isCreating={isCreating}
          onSave={handleSave}
          onCancel={() => { setEditingSynonym(null); setIsCreating(false); }}
          isPending={saveSynonym.isPending}
        />
      )}
    </div>
  );
}

interface SynonymEditorProps {
  synonym: Synonym;
  isCreating: boolean;
  onSave: (synonym: Synonym) => void;
  onCancel: () => void;
  isPending: boolean;
}

function SynonymEditor({ synonym: initial, isCreating, onSave, onCancel, isPending }: SynonymEditorProps) {
  const [draft, setDraft] = useState<Synonym>(initial);

  const updateField = (field: string, value: any) => {
    setDraft((prev) => ({ ...prev, [field]: value } as Synonym));
  };

  const updateListItem = (field: string, index: number, value: string) => {
    setDraft((prev) => {
      const list = [...((prev as any)[field] as string[])];
      list[index] = value;
      return { ...prev, [field]: list } as Synonym;
    });
  };

  const addListItem = (field: string) => {
    setDraft((prev) => {
      const list = [...((prev as any)[field] as string[]), ''];
      return { ...prev, [field]: list } as Synonym;
    });
  };

  const removeListItem = (field: string, index: number) => {
    setDraft((prev) => {
      const list = ((prev as any)[field] as string[]).filter((_, i) => i !== index);
      return { ...prev, [field]: list } as Synonym;
    });
  };

  const isValid = (): boolean => {
    if (!draft.objectID.trim()) return false;
    switch (draft.type) {
      case 'synonym':
        return draft.synonyms.length >= 2 && draft.synonyms.every((s) => s.trim());
      case 'onewaysynonym':
        return !!draft.input.trim() && draft.synonyms.length >= 1 && draft.synonyms.every((s) => s.trim());
      case 'altcorrection1':
      case 'altcorrection2':
        return !!draft.word.trim() && draft.corrections.length >= 1 && draft.corrections.every((c) => c.trim());
      case 'placeholder':
        return !!draft.placeholder.trim() && draft.replacements.length >= 1 && draft.replacements.every((r) => r.trim());
    }
  };

  return (
    <Dialog open onOpenChange={() => onCancel()}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>{isCreating ? 'Create Synonym' : 'Edit Synonym'}</DialogTitle>
          <DialogDescription>
            {SYNONYM_TYPE_LABELS[draft.type]} synonym
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          {/* Type selector (only when creating) */}
          {isCreating && (
            <div>
              <Label>Type</Label>
              <div className="flex flex-wrap gap-2 mt-1">
                {(Object.keys(SYNONYM_TYPE_LABELS) as SynonymType[]).map((type) => (
                  <Button
                    key={type}
                    variant={draft.type === type ? 'default' : 'outline'}
                    size="sm"
                    onClick={() => setDraft(getEmptySynonym(type))}
                  >
                    {SYNONYM_TYPE_LABELS[type]}
                  </Button>
                ))}
              </div>
            </div>
          )}

          {/* Object ID */}
          <div>
            <Label>ID</Label>
            <Input
              value={draft.objectID}
              onChange={(e) => updateField('objectID', e.target.value)}
              placeholder="synonym-id"
              className="font-mono"
              disabled={!isCreating}
            />
          </div>

          {/* Type-specific fields */}
          {draft.type === 'synonym' && (
            <div>
              <Label>Words (bidirectional)</Label>
              <div className="space-y-2 mt-1">
                {draft.synonyms.map((word, i) => (
                  <div key={i} className="flex gap-2">
                    <Input
                      value={word}
                      onChange={(e) => updateListItem('synonyms', i, e.target.value)}
                      placeholder={`Word ${i + 1}`}
                    />
                    {draft.synonyms.length > 2 && (
                      <Button variant="ghost" size="sm" onClick={() => removeListItem('synonyms', i)}>
                        <X className="h-4 w-4" />
                      </Button>
                    )}
                  </div>
                ))}
                <Button variant="outline" size="sm" onClick={() => addListItem('synonyms')}>
                  <Plus className="h-4 w-4 mr-1" /> Add Word
                </Button>
              </div>
            </div>
          )}

          {draft.type === 'onewaysynonym' && (
            <>
              <div>
                <Label>Input (source word)</Label>
                <Input
                  value={draft.input}
                  onChange={(e) => updateField('input', e.target.value)}
                  placeholder="e.g. phone"
                />
              </div>
              <div className="flex items-center gap-2 text-muted-foreground">
                <ArrowRight className="h-4 w-4" />
                <span className="text-sm">also matches:</span>
              </div>
              <div>
                <Label>Synonyms</Label>
                <div className="space-y-2 mt-1">
                  {draft.synonyms.map((word, i) => (
                    <div key={i} className="flex gap-2">
                      <Input
                        value={word}
                        onChange={(e) => updateListItem('synonyms', i, e.target.value)}
                        placeholder={`Synonym ${i + 1}`}
                      />
                      {draft.synonyms.length > 1 && (
                        <Button variant="ghost" size="sm" onClick={() => removeListItem('synonyms', i)}>
                          <X className="h-4 w-4" />
                        </Button>
                      )}
                    </div>
                  ))}
                  <Button variant="outline" size="sm" onClick={() => addListItem('synonyms')}>
                    <Plus className="h-4 w-4 mr-1" /> Add Synonym
                  </Button>
                </div>
              </div>
            </>
          )}

          {(draft.type === 'altcorrection1' || draft.type === 'altcorrection2') && (
            <>
              <div>
                <Label>Word</Label>
                <Input
                  value={draft.word}
                  onChange={(e) => updateField('word', e.target.value)}
                  placeholder="e.g. smartphone"
                />
              </div>
              <div>
                <Label>Corrections</Label>
                <div className="space-y-2 mt-1">
                  {draft.corrections.map((c, i) => (
                    <div key={i} className="flex gap-2">
                      <Input
                        value={c}
                        onChange={(e) => updateListItem('corrections', i, e.target.value)}
                        placeholder={`Correction ${i + 1}`}
                      />
                      {draft.corrections.length > 1 && (
                        <Button variant="ghost" size="sm" onClick={() => removeListItem('corrections', i)}>
                          <X className="h-4 w-4" />
                        </Button>
                      )}
                    </div>
                  ))}
                  <Button variant="outline" size="sm" onClick={() => addListItem('corrections')}>
                    <Plus className="h-4 w-4 mr-1" /> Add Correction
                  </Button>
                </div>
              </div>
            </>
          )}

          {draft.type === 'placeholder' && (
            <>
              <div>
                <Label>Placeholder token</Label>
                <Input
                  value={draft.placeholder}
                  onChange={(e) => updateField('placeholder', e.target.value)}
                  placeholder="e.g. brand_name"
                />
              </div>
              <div>
                <Label>Replacements</Label>
                <div className="space-y-2 mt-1">
                  {draft.replacements.map((r, i) => (
                    <div key={i} className="flex gap-2">
                      <Input
                        value={r}
                        onChange={(e) => updateListItem('replacements', i, e.target.value)}
                        placeholder={`Replacement ${i + 1}`}
                      />
                      {draft.replacements.length > 1 && (
                        <Button variant="ghost" size="sm" onClick={() => removeListItem('replacements', i)}>
                          <X className="h-4 w-4" />
                        </Button>
                      )}
                    </div>
                  ))}
                  <Button variant="outline" size="sm" onClick={() => addListItem('replacements')}>
                    <Plus className="h-4 w-4 mr-1" /> Add Replacement
                  </Button>
                </div>
              </div>
            </>
          )}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={onCancel}>Cancel</Button>
          <Button onClick={() => onSave(draft)} disabled={isPending || !isValid()}>
            {isPending ? 'Saving...' : isCreating ? 'Create' : 'Save'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
