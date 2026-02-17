import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import api from '@/lib/api';
import type { Rule, RuleSearchResponse } from '@/lib/types';
import { useToast } from '@/hooks/use-toast';

interface UseRulesOptions {
  indexName: string;
  query?: string;
  page?: number;
  hitsPerPage?: number;
}

export function useRules({ indexName, query = '', page = 0, hitsPerPage = 50 }: UseRulesOptions) {
  return useQuery({
    queryKey: ['rules', indexName, query, page, hitsPerPage],
    queryFn: async () => {
      const response = await api.post<RuleSearchResponse>(
        `/1/indexes/${indexName}/rules/search`,
        { query, page, hitsPerPage }
      );
      return response.data;
    },
    enabled: !!indexName,
  });
}

export function useSaveRule(indexName: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: async (rule: Rule) => {
      const response = await api.put(
        `/1/indexes/${indexName}/rules/${rule.objectID}`,
        rule
      );
      return response.data;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['rules', indexName] });
      toast({ title: 'Rule saved' });
    },
    onError: (error: any) => {
      toast({
        title: 'Failed to save rule',
        description: error.response?.data || error.message,
        variant: 'destructive',
      });
    },
  });
}

export function useDeleteRule(indexName: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: async (objectID: string) => {
      const response = await api.delete(
        `/1/indexes/${indexName}/rules/${objectID}`
      );
      return response.data;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['rules', indexName] });
      toast({ title: 'Rule deleted' });
    },
    onError: (error: any) => {
      toast({
        title: 'Failed to delete rule',
        description: error.response?.data || error.message,
        variant: 'destructive',
      });
    },
  });
}

export function useClearRules(indexName: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: async () => {
      const response = await api.post(`/1/indexes/${indexName}/rules/clear`);
      return response.data;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['rules', indexName] });
      toast({ title: 'All rules cleared' });
    },
    onError: (error: any) => {
      toast({
        title: 'Failed to clear rules',
        description: error.response?.data || error.message,
        variant: 'destructive',
      });
    },
  });
}
