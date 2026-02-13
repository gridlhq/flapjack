import { useMutation, useQueryClient } from '@tanstack/react-query';
import api from '@/lib/api';
import { useToast } from '@/hooks/use-toast';
import { addActiveTask, removeActiveTask } from '@/hooks/useIndexingStatus';

interface BrowseResponse {
  hits: Record<string, unknown>[];
  cursor: string | null;
  nbHits: number;
}

export function useReindex(indexName: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation<void, Error>({
    mutationFn: async () => {
      // Step 1: Browse ALL documents via cursor pagination
      const allDocs: Record<string, unknown>[] = [];

      async function browsePage(cursor: string | null): Promise<string | null> {
        const resp = await api.post<BrowseResponse>(
          `/1/indexes/${indexName}/browse`,
          { cursor, hitsPerPage: 1000 }
        );
        allDocs.push(...resp.data.hits);
        return resp.data.cursor;
      }

      let nextCursor = await browsePage(null);
      while (nextCursor) {
        nextCursor = await browsePage(nextCursor);
      }

      if (allDocs.length === 0) {
        throw new Error('No documents to re-index');
      }

      // Track in the header indexing queue so users see progress
      const taskId = `reindex-${indexName}-${Date.now()}`;
      addActiveTask({
        taskID: taskId,
        indexName,
        documentCount: allDocs.length,
        startedAt: Date.now(),
      });

      try {
        // Step 2: Clear the index (preserves settings)
        await api.post(`/1/indexes/${indexName}/clear`);

        // Step 3: Re-add all documents in batches of 1000
        const BATCH_SIZE = 1000;
        for (let i = 0; i < allDocs.length; i += BATCH_SIZE) {
          const batch = allDocs.slice(i, i + BATCH_SIZE);
          const requests = batch.map((doc) => ({
            action: 'addObject',
            body: doc,
          }));
          await api.post(`/1/indexes/${indexName}/batch`, { requests });
        }
      } finally {
        removeActiveTask(taskId);
      }
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['search'] });
      queryClient.invalidateQueries({ queryKey: ['indices'] });
      queryClient.invalidateQueries({ queryKey: ['index-stats'] });
      toast({
        title: 'Re-index complete',
        description: `All documents in "${indexName}" have been re-indexed with current settings.`,
      });
    },
    onError: (error) => {
      toast({
        variant: 'destructive',
        title: 'Re-index failed',
        description: error.message,
      });
    },
  });
}
