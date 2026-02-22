import { memo } from 'react';
import { Pencil, Trash2 } from 'lucide-react';
import { Card } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import type { EmbedderConfig } from '@/lib/types';

interface EmbedderCardProps {
  name: string;
  config: EmbedderConfig;
  onEdit: () => void;
  onDelete: () => void;
}

export const EmbedderCard = memo(function EmbedderCard({
  name,
  config,
  onEdit,
  onDelete,
}: EmbedderCardProps) {
  return (
    <Card
      data-testid={`embedder-card-${name}`}
      className="p-4 flex items-center justify-between"
    >
      <div className="flex items-center gap-3 min-w-0">
        <span className="font-semibold truncate">{name}</span>
        <Badge variant="secondary" className="text-xs shrink-0">
          {config.source}
        </Badge>
        {config.model && (
          <span className="text-sm text-muted-foreground truncate">
            {config.model}
          </span>
        )}
        {config.dimensions != null && (
          <span className="text-sm text-muted-foreground shrink-0">
            {config.dimensions}
          </span>
        )}
      </div>

      <div className="flex items-center gap-1 shrink-0">
        <Button
          variant="ghost"
          size="sm"
          data-testid={`embedder-edit-${name}`}
          onClick={onEdit}
        >
          <Pencil className="h-4 w-4" />
        </Button>
        <Button
          variant="ghost"
          size="sm"
          data-testid={`embedder-delete-${name}`}
          onClick={onDelete}
        >
          <Trash2 className="h-4 w-4" />
        </Button>
      </div>
    </Card>
  );
});
