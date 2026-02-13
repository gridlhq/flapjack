import { memo, useState, useCallback } from 'react';
import { Shield } from 'lucide-react';
import { useCreateApiKey } from '@/hooks/useApiKeys';
import { useIndices } from '@/hooks/useIndices';
import { InfoTooltip } from '@/components/ui/info-tooltip';
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

interface CreateKeyDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

const ACL_OPTIONS = [
  { value: 'search', label: 'Search', description: 'Perform search queries' },
  { value: 'browse', label: 'Browse', description: 'Browse all documents' },
  { value: 'addObject', label: 'Add Object', description: 'Add new documents' },
  { value: 'deleteObject', label: 'Delete Object', description: 'Delete documents' },
  { value: 'deleteIndex', label: 'Delete Index', description: 'Delete entire indices' },
  { value: 'settings', label: 'Settings', description: 'Modify index settings' },
  { value: 'listIndexes', label: 'List Indexes', description: 'List all indices' },
  { value: 'analytics', label: 'Analytics', description: 'Access analytics data' },
];

export const CreateKeyDialog = memo(function CreateKeyDialog({
  open,
  onOpenChange,
}: CreateKeyDialogProps) {
  const createKey = useCreateApiKey();
  const { data: indices } = useIndices();

  const [description, setDescription] = useState('');
  const [selectedAcl, setSelectedAcl] = useState<string[]>(['search']);
  const [selectedIndices, setSelectedIndices] = useState<string[]>([]);
  const [maxHitsPerQuery, setMaxHitsPerQuery] = useState('');
  const [maxQueriesPerIPPerHour, setMaxQueriesPerIPPerHour] = useState('');

  const handleCreate = useCallback(async () => {
    if (selectedAcl.length === 0) {
      alert('Please select at least one permission');
      return;
    }

    try {
      await createKey.mutateAsync({
        description: description || undefined,
        acl: selectedAcl,
        indexes: selectedIndices.length > 0 ? selectedIndices : undefined,
        maxHitsPerQuery: maxHitsPerQuery
          ? parseInt(maxHitsPerQuery, 10)
          : undefined,
        maxQueriesPerIPPerHour: maxQueriesPerIPPerHour
          ? parseInt(maxQueriesPerIPPerHour, 10)
          : undefined,
      });

      // Reset form
      setDescription('');
      setSelectedAcl(['search']);
      setSelectedIndices([]);
      setMaxHitsPerQuery('');
      setMaxQueriesPerIPPerHour('');
      onOpenChange(false);
    } catch (err) {
      console.error('Failed to create key:', err);
    }
  }, [
    description,
    selectedAcl,
    selectedIndices,
    maxHitsPerQuery,
    maxQueriesPerIPPerHour,
    createKey,
    onOpenChange,
  ]);

  const toggleAcl = useCallback((value: string) => {
    setSelectedAcl((prev) =>
      prev.includes(value)
        ? prev.filter((v) => v !== value)
        : [...prev, value]
    );
  }, []);

  const toggleIndex = useCallback((indexUid: string) => {
    setSelectedIndices((prev) =>
      prev.includes(indexUid)
        ? prev.filter((v) => v !== indexUid)
        : [...prev, indexUid]
    );
  }, []);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>Create API Key</DialogTitle>
          <DialogDescription>
            Generate a new API key with custom permissions and restrictions
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-6">
          {/* Description */}
          <div className="space-y-2">
            <Label>Description (optional)</Label>
            <Input
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="e.g., Frontend search key"
            />
          </div>

          {/* ACL */}
          <div className="space-y-2">
            <Label>Permissions *</Label>
            <p className="text-xs text-muted-foreground">
              Select which operations this key can perform
            </p>
            <div className="grid grid-cols-2 gap-2">
              {ACL_OPTIONS.map((option) => (
                <button
                  key={option.value}
                  onClick={() => toggleAcl(option.value)}
                  className={`text-left p-3 rounded-md border transition-colors ${
                    selectedAcl.includes(option.value)
                      ? 'border-primary bg-primary/10'
                      : 'border-border hover:border-primary/50'
                  }`}
                >
                  <div className="font-medium text-sm">{option.label}</div>
                  <div className="text-xs text-muted-foreground">
                    {option.description}
                  </div>
                </button>
              ))}
            </div>
            {selectedAcl.length > 0 && (
              <div className="flex flex-wrap gap-1 mt-2">
                {selectedAcl.map((acl) => (
                  <Badge key={acl} variant="secondary">
                    {acl}
                  </Badge>
                ))}
              </div>
            )}
          </div>

          {/* Index Scope */}
          <div className="space-y-2 rounded-md border border-border p-4" data-testid="index-scope-section">
            <div className="flex items-center gap-2">
              <Shield className="h-4 w-4 text-amber-500" />
              <Label className="text-base font-semibold">Index Scope</Label>
              <InfoTooltip content="Scoping a key to specific indices restricts what data it can access — essential for secure multi-index deployments." />
            </div>
            <p className="text-xs text-muted-foreground">
              Restrict this key to specific indices for access control, or leave unselected for access to all indices
            </p>
            {indices && indices.length > 0 ? (
              <div className="flex flex-wrap gap-2">
                {indices.map((index) => (
                  <button
                    key={index.uid}
                    onClick={() => toggleIndex(index.uid)}
                    className={`px-3 py-1 rounded-md text-sm border transition-colors ${
                      selectedIndices.includes(index.uid)
                        ? 'border-primary bg-primary/10'
                        : 'border-border hover:border-primary/50'
                    }`}
                  >
                    {index.name || index.uid}
                  </button>
                ))}
              </div>
            ) : (
              <p className="text-xs text-muted-foreground italic">
                No indices created yet. Create an index first to scope keys.
              </p>
            )}
            {selectedIndices.length > 0 && (
              <div className="flex items-center gap-2 mt-1 text-sm" data-testid="scope-summary">
                <span className="text-muted-foreground">This key can access:</span>
                {selectedIndices.map((idx) => (
                  <Badge key={idx} variant="outline">{idx}</Badge>
                ))}
              </div>
            )}
          </div>

          {/* Rate limits */}
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-2">
              <Label>Max Hits Per Query (optional)</Label>
              <Input
                type="number"
                min="1"
                value={maxHitsPerQuery}
                onChange={(e) => setMaxHitsPerQuery(e.target.value)}
                placeholder="Unlimited"
              />
            </div>

            <div className="space-y-2">
              <Label>Max Queries Per IP Per Hour (optional)</Label>
              <Input
                type="number"
                min="1"
                value={maxQueriesPerIPPerHour}
                onChange={(e) => setMaxQueriesPerIPPerHour(e.target.value)}
                placeholder="Unlimited"
              />
            </div>
          </div>
        </div>

        <DialogFooter>
          <Button
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={createKey.isPending}
          >
            Cancel
          </Button>
          <Button onClick={handleCreate} disabled={createKey.isPending}>
            {createKey.isPending ? 'Creating...' : 'Create Key'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
});
