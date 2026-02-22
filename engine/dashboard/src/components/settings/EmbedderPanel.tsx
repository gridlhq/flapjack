import { memo, useState } from 'react';
import { Plus } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { ConfirmDialog } from '@/components/ui/confirm-dialog';
import { SettingSection } from './shared';
import { EmbedderCard } from './EmbedderCard';
import { EmbedderDialog } from './EmbedderDialog';
import type { EmbedderConfig, IndexSettings } from '@/lib/types';

interface EmbedderPanelProps {
  embedders: Record<string, EmbedderConfig> | undefined;
  onChange: (updates: Partial<IndexSettings>) => void;
}

export const EmbedderPanel = memo(function EmbedderPanel({
  embedders,
  onChange,
}: EmbedderPanelProps) {
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editTarget, setEditTarget] = useState<{ name: string; config: EmbedderConfig } | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<string | null>(null);

  const entries = Object.entries(embedders || {});

  function handleAdd() {
    setEditTarget(null);
    setDialogOpen(true);
  }

  function handleEdit(name: string, config: EmbedderConfig) {
    setEditTarget({ name, config });
    setDialogOpen(true);
  }

  function handleSave(name: string, config: EmbedderConfig) {
    onChange({
      embedders: { ...(embedders || {}), [name]: config },
    });
  }

  function handleConfirmDelete() {
    if (!deleteTarget || !embedders) return;
    const { [deleteTarget]: _, ...rest } = embedders;
    onChange({ embedders: rest });
    setDeleteTarget(null);
  }

  return (
    <SettingSection
      title="Embedders"
      description="Configure embedding models for vector search"
    >
      <div data-testid="embedder-panel" className="space-y-3">
        {entries.length === 0 && (
          <p className="text-sm text-muted-foreground">No embedders configured</p>
        )}

        {entries.map(([name, config]) => (
          <EmbedderCard
            key={name}
            name={name}
            config={config}
            onEdit={() => handleEdit(name, config)}
            onDelete={() => setDeleteTarget(name)}
          />
        ))}

        <Button
          variant="outline"
          size="sm"
          data-testid="add-embedder-btn"
          onClick={handleAdd}
        >
          <Plus className="h-4 w-4 mr-1" />
          Add Embedder
        </Button>
      </div>

      <EmbedderDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        onSave={handleSave}
        editName={editTarget?.name}
        editConfig={editTarget?.config}
      />

      <ConfirmDialog
        open={!!deleteTarget}
        onOpenChange={(open) => { if (!open) setDeleteTarget(null); }}
        title="Delete Embedder"
        description={
          <>
            Are you sure you want to remove the embedder{' '}
            <code className="font-mono text-sm bg-muted px-1 py-0.5 rounded">
              {deleteTarget}
            </code>
            ? This action cannot be undone after saving.
          </>
        }
        confirmLabel="Confirm"
        variant="destructive"
        onConfirm={handleConfirmDelete}
      />
    </SettingSection>
  );
});
