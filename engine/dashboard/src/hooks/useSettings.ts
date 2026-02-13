import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import api from '@/lib/api';
import type { IndexSettings } from '@/lib/types';
import { useToast } from '@/hooks/use-toast';

export function useSettings(indexName: string) {
  return useQuery({
    queryKey: ['settings', indexName],
    queryFn: async () => {
      const response = await api.get<IndexSettings>(
        `/1/indexes/${indexName}/settings`
      );
      return response.data;
    },
    enabled: !!indexName,
  });
}

export function useUpdateSettings(indexName: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: async (settings: Partial<IndexSettings>) => {
      const response = await api.put(
        `/1/indexes/${indexName}/settings`,
        settings
      );
      return response.data;
    },
    onSuccess: () => {
      // Invalidate settings cache
      queryClient.invalidateQueries({ queryKey: ['settings', indexName] });
      // Also invalidate search results as settings affect them
      queryClient.invalidateQueries({ queryKey: ['search', indexName] });

      toast({
        title: 'Settings saved',
        description: `Settings for ${indexName} have been updated successfully.`,
      });
    },
    onError: (error: Error) => {
      toast({
        variant: 'destructive',
        title: 'Failed to save settings',
        description: error.message || 'An error occurred while saving settings.',
      });
    },
  });
}
