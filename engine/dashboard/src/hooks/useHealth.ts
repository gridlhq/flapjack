import { useQuery } from '@tanstack/react-query';
import api from '@/lib/api';

interface HealthResponse {
  status: 'ok' | 'error';
}

export function useHealth() {
  return useQuery({
    queryKey: ['health'],
    queryFn: async () => {
      await api.get('/health');
      // Backend returns 200 OK when healthy (may have empty body)
      return { status: 'ok' } as HealthResponse;
    },
    refetchInterval: 3000, // Check health every 3 seconds
    retry: 1,
    retryDelay: 1000,
  });
}
