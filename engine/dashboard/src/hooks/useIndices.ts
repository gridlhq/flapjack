import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import api from '@/lib/api';
import { Index } from '@/lib/types';
import { useToast } from '@/hooks/use-toast';

export function useIndices() {
  return useQuery<Index[]>({
    queryKey: ['indices'],
    queryFn: async () => {
      const { data } = await api.get('/1/indexes');
      const items = data.results || data.items || data || [];
      // Map 'name' to 'uid' for compatibility
      return items.map((item: any) => ({
        ...item,
        uid: item.uid || item.name,
      }));
    },
    staleTime: 30000, // 30s cache
    retry: 1,
  });
}

export function useCreateIndex() {
  const queryClient = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: async (params: { uid: string }) => {
      const { data } = await api.post('/1/indexes', params);
      return data;
    },
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({ queryKey: ['indices'] });
      toast({ title: 'Index created', description: `Index "${variables.uid}" has been created.` });
    },
    onError: (error: Error) => {
      toast({ variant: 'destructive', title: 'Failed to create index', description: error.message });
    },
  });
}

export function useDeleteIndex() {
  const queryClient = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: async (indexName: string) => {
      await api.delete(`/1/indexes/${indexName}`);
    },
    onSuccess: (_data, indexName) => {
      queryClient.invalidateQueries({ queryKey: ['indices'] });
      toast({ title: 'Index deleted', description: `Index "${indexName}" has been deleted.` });
    },
    onError: (error: Error) => {
      toast({ variant: 'destructive', title: 'Failed to delete index', description: error.message });
    },
  });
}

export function useCompactIndex() {
  const queryClient = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: async (indexName: string) => {
      const { data } = await api.post(`/1/indexes/${indexName}/compact`);
      return data;
    },
    onSuccess: (_data, indexName) => {
      queryClient.invalidateQueries({ queryKey: ['indices'] });
      toast({ title: 'Compaction started', description: `Index "${indexName}" is being compacted.` });
    },
    onError: (error: Error) => {
      toast({ variant: 'destructive', title: 'Failed to compact index', description: error.message });
    },
  });
}

export function useIndexStats(indexName: string) {
  return useQuery({
    queryKey: ['index-stats', indexName],
    queryFn: async () => {
      const { data } = await api.get(`/1/indexes/${indexName}`);
      return data;
    },
    enabled: !!indexName,
  });
}
