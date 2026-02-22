import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import api from '@/lib/api';
import type { QsConfig, QsBuildStatus, QsLogEntry } from '@/lib/types';
import { useToast } from '@/hooks/use-toast';

export function useQsConfigs() {
  return useQuery<QsConfig[]>({
    queryKey: ['qsConfigs'],
    queryFn: async () => {
      const response = await api.get<QsConfig[]>('/1/configs');
      return response.data;
    },
  });
}

export function useQsBuildStatus(indexName: string) {
  return useQuery<QsBuildStatus>({
    queryKey: ['qsStatus', indexName],
    queryFn: async () => {
      const response = await api.get<QsBuildStatus>(`/1/configs/${indexName}/status`);
      return response.data;
    },
    refetchInterval: (query) => {
      // Poll every 2s while a build is running
      const data = query.state.data;
      return data?.isRunning ? 2000 : false;
    },
  });
}

export function useQsLogs(indexName: string) {
  return useQuery<QsLogEntry[]>({
    queryKey: ['qsLogs', indexName],
    queryFn: async () => {
      const response = await api.get<QsLogEntry[]>(`/1/logs/${indexName}`);
      return response.data;
    },
  });
}

export function useCreateQsConfig() {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: async (config: QsConfig) => {
      const response = await api.post<{ status: number; message: string }>('/1/configs', config);
      return response.data;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['qsConfigs'] });
      toast({
        title: 'Config created',
        description: 'Query Suggestions config created. Building index now.',
      });
    },
    onError: (error: Error) => {
      toast({
        variant: 'destructive',
        title: 'Failed to create config',
        description: error.message || 'An error occurred.',
      });
    },
  });
}

export function useDeleteQsConfig() {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: async (indexName: string) => {
      await api.delete(`/1/configs/${indexName}`);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['qsConfigs'] });
      toast({
        title: 'Config deleted',
        description: 'Query Suggestions config deleted. The suggestions index is preserved.',
      });
    },
    onError: (error: Error) => {
      toast({
        variant: 'destructive',
        title: 'Failed to delete config',
        description: error.message || 'An error occurred.',
      });
    },
  });
}

export function useTriggerQsBuild() {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: async (indexName: string) => {
      await api.post(`/1/configs/${indexName}/build`, {});
    },
    onSuccess: (_data, indexName) => {
      queryClient.invalidateQueries({ queryKey: ['qsStatus', indexName] });
      toast({
        title: 'Build triggered',
        description: 'Rebuilding Query Suggestions index.',
      });
    },
    onError: (error: Error) => {
      toast({
        variant: 'destructive',
        title: 'Failed to trigger build',
        description: error.message || 'An error occurred.',
      });
    },
  });
}
