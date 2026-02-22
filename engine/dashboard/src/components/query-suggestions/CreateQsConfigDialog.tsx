import { memo, useState, useCallback } from 'react';
import { X } from 'lucide-react';
import { useCreateQsConfig } from '@/hooks/useQuerySuggestions';
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
import { Label } from '@/components/ui/label';
import { Badge } from '@/components/ui/badge';

interface CreateQsConfigDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export const CreateQsConfigDialog = memo(function CreateQsConfigDialog({
  open,
  onOpenChange,
}: CreateQsConfigDialogProps) {
  const createConfig = useCreateQsConfig();

  const [indexName, setIndexName] = useState('');
  const [sourceIndex, setSourceIndex] = useState('');
  const [minHits, setMinHits] = useState('5');
  const [minLetters, setMinLetters] = useState('4');
  const [excludeInput, setExcludeInput] = useState('');
  const [excludeList, setExcludeList] = useState<string[]>([]);

  const resetForm = useCallback(() => {
    setIndexName('');
    setSourceIndex('');
    setMinHits('5');
    setMinLetters('4');
    setExcludeInput('');
    setExcludeList([]);
  }, []);

  const handleAddExclude = useCallback(() => {
    const word = excludeInput.trim().toLowerCase();
    if (word && !excludeList.includes(word)) {
      setExcludeList((prev) => [...prev, word]);
    }
    setExcludeInput('');
  }, [excludeInput, excludeList]);

  const handleExcludeKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        handleAddExclude();
      }
    },
    [handleAddExclude]
  );

  const handleRemoveExclude = useCallback((word: string) => {
    setExcludeList((prev) => prev.filter((w) => w !== word));
  }, []);

  const handleCreate = useCallback(async () => {
    if (!indexName.trim()) {
      alert('Suggestions index name is required');
      return;
    }
    if (!sourceIndex.trim()) {
      alert('Source index name is required');
      return;
    }

    try {
      await createConfig.mutateAsync({
        indexName: indexName.trim(),
        sourceIndices: [
          {
            indexName: sourceIndex.trim(),
            minHits: parseInt(minHits, 10) || 5,
            minLetters: parseInt(minLetters, 10) || 4,
          },
        ],
        exclude: excludeList.length > 0 ? excludeList : undefined,
      });
      resetForm();
      onOpenChange(false);
    } catch {
      // Error is already surfaced via the mutation's onError toast — dialog stays open for retry
    }
  }, [indexName, sourceIndex, minHits, minLetters, excludeList, createConfig, resetForm, onOpenChange]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[500px]">
        <DialogHeader>
          <DialogTitle>Create Query Suggestions Config</DialogTitle>
          <DialogDescription>
            Define a source index and filters. Flapjack will build a suggestions
            index from the most popular searches.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-2">
          {/* Suggestions index name */}
          <div className="space-y-1.5">
            <Label htmlFor="qs-index-name">Suggestions Index Name</Label>
            <Input
              id="qs-index-name"
              placeholder="e.g. my_suggestions"
              value={indexName}
              onChange={(e) => setIndexName(e.target.value)}
              aria-label="Suggestions index name"
            />
          </div>

          {/* Source index */}
          <div className="space-y-1.5">
            <Label htmlFor="qs-source-index">Source Index</Label>
            <Input
              id="qs-source-index"
              placeholder="e.g. products"
              value={sourceIndex}
              onChange={(e) => setSourceIndex(e.target.value)}
              aria-label="Source index name"
            />
            <p className="text-xs text-muted-foreground">
              The index whose search analytics will be used to build suggestions.
            </p>
          </div>

          {/* Min hits */}
          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1.5">
              <Label htmlFor="qs-min-hits">Min Hits</Label>
              <Input
                id="qs-min-hits"
                type="number"
                min={1}
                value={minHits}
                onChange={(e) => setMinHits(e.target.value)}
                aria-label="Minimum hits"
              />
              <p className="text-xs text-muted-foreground">Min search count to include a query.</p>
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="qs-min-letters">Min Letters</Label>
              <Input
                id="qs-min-letters"
                type="number"
                min={1}
                value={minLetters}
                onChange={(e) => setMinLetters(e.target.value)}
                aria-label="Minimum letters"
              />
              <p className="text-xs text-muted-foreground">Min query length to include.</p>
            </div>
          </div>

          {/* Exclude list */}
          <div className="space-y-1.5">
            <Label htmlFor="qs-exclude">Exclude Words</Label>
            <div className="flex gap-2">
              <Input
                id="qs-exclude"
                placeholder="Type a word and press Enter"
                value={excludeInput}
                onChange={(e) => setExcludeInput(e.target.value)}
                onKeyDown={handleExcludeKeyDown}
                aria-label="Exclude word"
              />
              <Button type="button" variant="outline" onClick={handleAddExclude}>
                Add
              </Button>
            </div>
            {excludeList.length > 0 && (
              <div className="flex flex-wrap gap-2 pt-1" data-testid="exclude-list">
                {excludeList.map((word) => (
                  <Badge key={word} variant="secondary" className="gap-1">
                    {word}
                    <button
                      onClick={() => handleRemoveExclude(word)}
                      className="ml-1 hover:text-destructive"
                      aria-label={`Remove ${word} from exclude list`}
                    >
                      <X className="h-3 w-3" />
                    </button>
                  </Badge>
                ))}
              </div>
            )}
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => { resetForm(); onOpenChange(false); }}>
            Cancel
          </Button>
          <Button onClick={handleCreate} disabled={createConfig.isPending}>
            {createConfig.isPending ? 'Creating…' : 'Create Config'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
});
