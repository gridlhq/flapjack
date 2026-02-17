import { useQuery } from '@tanstack/react-query';
import api from '@/lib/api';
import type { SearchResponse } from '@/lib/types';

export interface FieldInfo {
  name: string;
  type: 'text' | 'number' | 'boolean';
}

function inferType(value: unknown): 'text' | 'number' | 'boolean' {
  if (typeof value === 'number') return 'number';
  if (typeof value === 'boolean') return 'boolean';
  return 'text';
}

export function useIndexFields(indexName: string, enabled = true) {
  return useQuery<FieldInfo[]>({
    queryKey: ['index-fields', indexName],
    queryFn: async () => {
      const { data } = await api.post<SearchResponse>(
        `/1/indexes/${indexName}/query`,
        { query: '', hitsPerPage: 1 }
      );
      if (!data.hits || data.hits.length === 0) return [];

      const sample = data.hits[0];
      return Object.entries(sample)
        .filter(([key]) => key !== 'objectID' && !key.startsWith('_'))
        .map(([key, value]) => ({
          name: key,
          type: inferType(value),
        }));
    },
    enabled: enabled && !!indexName,
    staleTime: 60000,
  });
}
