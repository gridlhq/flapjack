import { memo, useState, useCallback, useEffect, useRef } from 'react';
import type { SearchParams, HybridSearchParams } from '@/lib/types';

interface HybridSearchControlsProps {
  embedderNames: string[];
  onParamsChange: (updates: Partial<SearchParams>) => void;
  initialRatio?: number;
}

function getRatioLabel(ratio: number): string {
  if (ratio === 0) return 'Keyword only';
  if (ratio === 1) return 'Semantic only';
  if (ratio === 0.5) return 'Balanced';
  const pct = Math.round(ratio * 100);
  return `${pct}% semantic`;
}

export const HybridSearchControls = memo(function HybridSearchControls({
  embedderNames,
  onParamsChange,
  initialRatio = 0.5,
}: HybridSearchControlsProps) {
  const [ratio, setRatio] = useState(initialRatio);
  const [selectedEmbedder, setSelectedEmbedder] = useState(embedderNames[0] || '');

  // Sync selectedEmbedder when embedderNames loads asynchronously or changes
  useEffect(() => {
    if (embedderNames.length > 0 && !embedderNames.includes(selectedEmbedder)) {
      setSelectedEmbedder(embedderNames[0]);
    }
  }, [embedderNames, selectedEmbedder]);

  const emitChange = useCallback(
    (newRatio: number, embedder: string) => {
      const hybrid: HybridSearchParams = { semanticRatio: newRatio };
      if (embedderNames.length > 1 && embedder) {
        hybrid.embedder = embedder;
      }
      onParamsChange({ hybrid });
    },
    [embedderNames, onParamsChange]
  );

  // Emit initial params when embedders first become available so parent
  // state matches the slider's visual default
  const hasEmittedInitial = useRef(false);
  useEffect(() => {
    if (embedderNames.length > 0 && !hasEmittedInitial.current) {
      hasEmittedInitial.current = true;
      emitChange(initialRatio, embedderNames[0] || '');
    }
  }, [embedderNames, initialRatio, emitChange]);

  const handleRatioChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const newRatio = parseFloat(e.target.value);
      setRatio(newRatio);
      emitChange(newRatio, selectedEmbedder);
    },
    [emitChange, selectedEmbedder]
  );

  const handleEmbedderChange = useCallback(
    (e: React.ChangeEvent<HTMLSelectElement>) => {
      const embedder = e.target.value;
      setSelectedEmbedder(embedder);
      emitChange(ratio, embedder);
    },
    [emitChange, ratio]
  );

  if (embedderNames.length === 0) return null;

  return (
    <div
      data-testid="hybrid-controls"
      className="flex items-center gap-4 rounded-md border border-input bg-background px-4 py-2 text-sm"
    >
      <span className="font-medium whitespace-nowrap">Hybrid Search</span>

      <input
        type="range"
        data-testid="semantic-ratio-slider"
        min={0}
        max={1}
        step={0.1}
        value={ratio}
        onChange={handleRatioChange}
        className="w-40 accent-primary"
      />

      <span
        data-testid="semantic-ratio-label"
        className="text-muted-foreground whitespace-nowrap min-w-[100px]"
      >
        {getRatioLabel(ratio)}
      </span>

      {embedderNames.length > 1 && (
        <select
          data-testid="embedder-select"
          value={selectedEmbedder}
          onChange={handleEmbedderChange}
          className="flex h-8 rounded-md border border-input bg-background px-2 py-1 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
        >
          {embedderNames.map((name) => (
            <option key={name} value={name}>
              {name}
            </option>
          ))}
        </select>
      )}
    </div>
  );
});
