import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import api from '@/lib/api';
import type { ApiKey } from '@/lib/types';
import { useToast } from '@/hooks/use-toast';

export function useApiKeys() {
  return useQuery({
    queryKey: ['apiKeys'],
    queryFn: async () => {
      const response = await api.get<{ keys: ApiKey[] }>('/1/keys');
      return response.data.keys;
    },
  });
}

export function useCreateApiKey() {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: async (params: {
      description?: string;
      acl: string[];
      indexes?: string[];
      expiresAt?: number;
      maxHitsPerQuery?: number;
      maxQueriesPerIPPerHour?: number;
    }) => {
      const response = await api.post<ApiKey>('/1/keys', params);
      return response.data;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['apiKeys'] });
      toast({
        title: 'API key created',
        description: 'Your new API key has been created successfully.',
      });
    },
    onError: (error: Error) => {
      toast({
        variant: 'destructive',
        title: 'Failed to create API key',
        description: error.message || 'An error occurred while creating the API key.',
      });
    },
  });
}

export function useDeleteApiKey() {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: async (keyValue: string) => {
      await api.delete(`/1/keys/${keyValue}`);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['apiKeys'] });
      toast({
        title: 'API key deleted',
        description: 'The API key has been deleted successfully.',
      });
    },
    onError: (error: Error) => {
      toast({
        variant: 'destructive',
        title: 'Failed to delete API key',
        description: error.message || 'An error occurred while deleting the API key.',
      });
    },
  });
}
