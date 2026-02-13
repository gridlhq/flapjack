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
    refetchInterval: 10000, // Check health every 10 seconds
    retry: 2,
  });
}
