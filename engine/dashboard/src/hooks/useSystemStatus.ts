import { useQuery } from '@tanstack/react-query';
import api from '@/lib/api';

export interface HealthDetail {
  status: string;
  active_writers: number;
  max_concurrent_writers: number;
  facet_cache_entries: number;
  facet_cache_cap: number;
}

export interface InternalStatus {
  node_id: string;
  replication_enabled: boolean;
  peer_count: number;
  ssl_renewal?: {
    next_renewal?: string;
    certificate_expiry?: string;
  };
}

export function useHealthDetail() {
  return useQuery<HealthDetail>({
    queryKey: ['health-detail'],
    queryFn: async () => {
      const { data } = await api.get('/health');
      // If backend returns empty 200, provide defaults
      return {
        status: data?.status || 'ok',
        active_writers: data?.active_writers ?? 0,
        max_concurrent_writers: data?.max_concurrent_writers ?? 0,
        facet_cache_entries: data?.facet_cache_entries ?? 0,
        facet_cache_cap: data?.facet_cache_cap ?? 0,
      };
    },
    refetchInterval: 5000,
    retry: 1,
  });
}

export function useInternalStatus() {
  return useQuery<InternalStatus>({
    queryKey: ['internal-status'],
    queryFn: async () => {
      const { data } = await api.get('/internal/status');
      return data;
    },
    refetchInterval: 10000,
    retry: 1,
  });
}
