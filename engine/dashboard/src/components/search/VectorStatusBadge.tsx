import { memo } from 'react';
import { Badge } from '@/components/ui/badge';
import type { EmbedderConfig, IndexMode } from '@/lib/types';

interface VectorStatusBadgeProps {
  embedders: Record<string, EmbedderConfig> | undefined;
  mode: IndexMode | undefined;
}

export const VectorStatusBadge = memo(function VectorStatusBadge({
  embedders,
  mode,
}: VectorStatusBadgeProps) {
  const embedderCount = embedders ? Object.keys(embedders).length : 0;
  if (embedderCount === 0) return null;

  const modeLabel = mode === 'neuralSearch' ? 'Neural' : 'Keyword';
  const embedderLabel = embedderCount === 1 ? '1 embedder' : `${embedderCount} embedders`;

  return (
    <Badge
      variant="secondary"
      data-testid="vector-status-badge"
      className="text-xs"
    >
      Vector Search · {embedderLabel} · {modeLabel}
    </Badge>
  );
});
