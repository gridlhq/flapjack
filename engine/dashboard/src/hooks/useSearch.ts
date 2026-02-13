import { useQuery } from '@tanstack/react-query';
import api from '@/lib/api';
import type { SearchParams, SearchResponse, Document } from '@/lib/types';

interface UseSearchOptions {
  indexName: string;
  params: SearchParams;
  enabled?: boolean;
  userToken?: string;
}

export function useSearch<T = Document>({ indexName, params, enabled = true, userToken }: UseSearchOptions) {
  return useQuery({
    queryKey: ['search', indexName, params],
    queryFn: async () => {
      const payload = { analytics: false, ...params };
      const headers: Record<string, string> = {};
      if (userToken) {
        headers['x-algolia-usertoken'] = userToken;
      }
      const response = await api.post<SearchResponse<T>>(
        `/1/indexes/${indexName}/query`,
        payload,
        { headers }
      );
      return response.data;
    },
    enabled: enabled && !!indexName,
    staleTime: 0, // Always refetch for fresh results
    retry: false,
  });
}

export function useFacetSearch(indexName: string, facetName: string, facetQuery?: string) {
  return useQuery({
    queryKey: ['facetSearch', indexName, facetName, facetQuery],
    queryFn: async () => {
      const response = await api.post(
        `/1/indexes/${indexName}/facets/${facetName}/query`,
        { facetQuery }
      );
      return response.data;
    },
    enabled: !!indexName && !!facetName,
  });
}
