import { useQuery } from '@tanstack/react-query';
import api from '@/lib/api';

export interface HealthDetail {
  status: string;
  active_writers: number;
  max_concurrent_writers: number;
  facet_cache_entries: number;
  facet_cache_cap: number;
  tenants_loaded: number;
  uptime_secs: number;
  version: string;
  heap_allocated_mb: number;
  system_limit_mb: number;
  pressure_level: string;
  allocator: string;
  build_profile: string;
}

export interface InternalStatus {
  node_id: string;
  replication_enabled: boolean;
  peer_count: number;
  storage_total_bytes: number;
  tenant_count: number;
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
      return data;
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
