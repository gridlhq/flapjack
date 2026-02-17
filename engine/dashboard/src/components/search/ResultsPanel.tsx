import { memo, useCallback, useMemo, useState } from 'react';
import { ChevronLeft, ChevronRight } from 'lucide-react';
import { useSearch } from '@/hooks/useSearch';
import { useDeleteDocument } from '@/hooks/useDocuments';
import { Card } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Skeleton } from '@/components/ui/skeleton';
import { ConfirmDialog } from '@/components/ui/confirm-dialog';
import { DocumentCard } from '@/components/search/DocumentCard';
import type { SearchParams } from '@/lib/types';

interface ResultsPanelProps {
  indexName: string;
  params: SearchParams;
  onParamsChange: (updates: Partial<SearchParams>) => void;
  onResultClick?: (objectID: string, position: number, queryID?: string) => void;
  userToken?: string;
}

export const ResultsPanel = memo(function ResultsPanel({
  indexName,
  params,
  onParamsChange,
  onResultClick,
  userToken,
}: ResultsPanelProps) {
  const { data, isLoading, error } = useSearch({
    indexName,
    params,
    userToken,
  });
  const deleteDoc = useDeleteDocument(indexName);
  const [pendingDeleteId, setPendingDeleteId] = useState<string | null>(null);

  const handlePageChange = useCallback(
    (newPage: number) => {
      onParamsChange({ page: newPage });
    },
    [onParamsChange]
  );

  const handleDelete = useCallback(
    (objectID: string) => {
      setPendingDeleteId(objectID);
    },
    []
  );

  const confirmDelete = useCallback(() => {
    if (pendingDeleteId) {
      deleteDoc.mutate(pendingDeleteId, {
        onSettled: () => setPendingDeleteId(null),
      });
    }
  }, [pendingDeleteId, deleteDoc]);

  // Compute a stable field order from all hits so every DocumentCard
  // shows fields in the same order (first-seen across the result set).
  const fieldOrder = useMemo(() => {
    if (!data?.hits?.length) return [];
    const seen = new Set<string>();
    const order: string[] = [];
    for (const hit of data.hits) {
      for (const key of Object.keys(hit)) {
        if (key === 'objectID' || key === '_highlightResult') continue;
        if (!seen.has(key)) {
          seen.add(key);
          order.push(key);
        }
      }
    }
    return order;
  }, [data?.hits]);

  const currentPage = params.page || 0;
  const totalPages = data?.nbPages || 0;
  const hasPrevPage = currentPage > 0;
  const hasNextPage = currentPage < totalPages - 1;

  if (error) {
    return (
      <Card className="p-8 text-center">
        <h3 className="text-lg font-semibold text-destructive mb-2">Search Error</h3>
        <p className="text-sm text-muted-foreground">
          {error instanceof Error ? error.message : 'Failed to search'}
        </p>
      </Card>
    );
  }

  if (isLoading) {
    return (
      <div className="flex flex-col gap-4">
        <Card className="p-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Skeleton className="h-4 w-20" />
              <Skeleton className="h-4 w-12" />
            </div>
          </div>
        </Card>
        {[1, 2, 3].map((i) => (
          <Card key={i} className="p-4 space-y-3">
            <div className="flex items-center justify-between">
              <Skeleton className="h-5 w-48" />
              <Skeleton className="h-6 w-16 rounded-full" />
            </div>
            <Skeleton className="h-4 w-full" />
            <Skeleton className="h-4 w-3/4" />
          </Card>
        ))}
      </div>
    );
  }

  if (!data?.hits?.length) {
    return (
      <Card className="p-8 text-center">
        <h3 className="text-lg font-semibold mb-2">No results found</h3>
        <p className="text-sm text-muted-foreground">
          {params.query
            ? `No documents match "${params.query}"`
            : 'Try a different search query'}
        </p>
      </Card>
    );
  }

  return (
    <div className="flex flex-col gap-4 h-full" data-testid="results-panel">
      {/* Results header */}
      <Card className="p-4">
        <div className="flex items-center justify-between">
          <div className="text-sm">
            <span className="font-semibold">{data.nbHits.toLocaleString()}</span>
            <span className="text-muted-foreground ml-1">
              {data.nbHits === 1 ? 'result' : 'results'}
            </span>
            <span className="text-muted-foreground mx-2">•</span>
            <span className="text-muted-foreground">
              {data.processingTimeMS}ms
            </span>
          </div>

          {/* Pagination */}
          {totalPages > 1 && (
            <div className="flex items-center gap-2">
              <Button
                variant="outline"
                size="sm"
                onClick={() => handlePageChange(currentPage - 1)}
                disabled={!hasPrevPage}
              >
                <ChevronLeft className="h-4 w-4" />
              </Button>
              <span className="text-sm text-muted-foreground">
                Page {currentPage + 1} of {totalPages}
              </span>
              <Button
                variant="outline"
                size="sm"
                onClick={() => handlePageChange(currentPage + 1)}
                disabled={!hasNextPage}
              >
                <ChevronRight className="h-4 w-4" />
              </Button>
            </div>
          )}
        </div>
      </Card>

      {/* Results list - scrollable */}
      <div className="flex-1 overflow-y-auto space-y-3">
        {data.hits.map((hit, index) => (
          <DocumentCard
            key={hit.objectID || index}
            document={hit}
            fieldOrder={fieldOrder}
            onDelete={handleDelete}
            isDeleting={deleteDoc.isPending}
            onClick={
              onResultClick
                ? () => {
                    const position = (params.page || 0) * (params.hitsPerPage || 20) + index + 1;
                    onResultClick(hit.objectID, position, data.queryID);
                  }
                : undefined
            }
          />
        ))}
      </div>

      <ConfirmDialog
        open={pendingDeleteId !== null}
        onOpenChange={(open) => { if (!open) setPendingDeleteId(null); }}
        title="Delete Document"
        description={
          <>
            Are you sure you want to delete{' '}
            <code className="font-mono text-sm bg-muted px-1 py-0.5 rounded">
              {pendingDeleteId}
            </code>
            ? This action cannot be undone.
          </>
        }
        confirmLabel="Delete"
        variant="destructive"
        onConfirm={confirmDelete}
        isPending={deleteDoc.isPending}
      />
    </div>
  );
});
