import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import api from '@/lib/api';
import type { Experiment } from '@/lib/types';
import { useToast } from '@/hooks/use-toast';

interface CreateExperimentPayload {
  name: string;
  indexName: string;
  trafficSplit: number;
  control: { name: string };
  variant: {
    name: string;
    queryOverrides?: Record<string, unknown>;
    indexName?: string;
  };
  primaryMetric: string;
  minimumDays: number;
}

export function useExperiment(experimentId: string) {
  return useQuery<Experiment>({
    queryKey: ['experiment', experimentId],
    queryFn: async () => {
      const { data } = await api.get(`/2/abtests/${experimentId}`);
      return data;
    },
    enabled: !!experimentId,
    retry: 1,
  });
}

export function useExperiments() {
  return useQuery<Experiment[]>({
    queryKey: ['experiments'],
    queryFn: async () => {
      const { data } = await api.get('/2/abtests');
      return data.abtests || [];
    },
    staleTime: 15000,
    retry: 1,
  });
}

export function useCreateExperiment() {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: async (payload: CreateExperimentPayload) => {
      const { data } = await api.post('/2/abtests', payload);
      return data;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['experiments'] });
      toast({ title: 'Experiment created' });
    },
    onError: (error: Error) => {
      toast({
        variant: 'destructive',
        title: 'Failed to create experiment',
        description: error.message,
      });
    },
  });
}

export function useStopExperiment() {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: async (id: string) => {
      const { data } = await api.post(`/2/abtests/${id}/stop`);
      return data;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['experiments'] });
      toast({ title: 'Experiment stopped' });
    },
    onError: (error: Error) => {
      toast({
        variant: 'destructive',
        title: 'Failed to stop experiment',
        description: error.message,
      });
    },
  });
}

export function useDeleteExperiment() {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: async (id: string) => {
      await api.delete(`/2/abtests/${id}`);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['experiments'] });
      toast({ title: 'Experiment deleted' });
    },
    onError: (error: Error) => {
      toast({
        variant: 'destructive',
        title: 'Failed to delete experiment',
        description: error.message,
      });
    },
  });
}

export interface ExperimentResultsResponse {
  experimentID: string;
  name: string;
  status: string;
  indexName: string;
  startDate: string | null;
  endedAt: string | null;
  conclusion: ExperimentConclusionResponse | null;
  trafficSplit: number;
  primaryMetric: string;
  gate: {
    minimumNReached: boolean;
    minimumDaysReached: boolean;
    readyToRead: boolean;
    requiredSearchesPerArm: number;
    currentSearchesPerArm: number;
    progressPct: number;
    estimatedDaysRemaining: number | null;
  };
  control: ArmResultsResponse;
  variant: ArmResultsResponse;
  significance: SignificanceResponse | null;
  bayesian: { probVariantBetter: number } | null;
  sampleRatioMismatch: boolean;
  cupedApplied: boolean;
  guardRailAlerts: GuardRailAlertResponse[];
  outlierUsersExcluded: number;
  noStableIdQueries: number;
  recommendation: string | null;
  interleaving: InterleavingResultsResponse | null;
}

export interface InterleavingResultsResponse {
  deltaAB: number;
  winsControl: number;
  winsVariant: number;
  ties: number;
  pValue: number;
  significant: boolean;
  totalQueries: number;
  dataQualityOk: boolean;
}

export interface GuardRailAlertResponse {
  metricName: string;
  controlValue: number;
  variantValue: number;
  dropPct: number;
}

export interface ExperimentConclusionResponse {
  winner: string | null;
  reason: string;
  controlMetric: number;
  variantMetric: number;
  confidence: number;
  significant: boolean;
  promoted: boolean;
}

export interface ArmResultsResponse {
  name: string;
  searches: number;
  users: number;
  clicks: number;
  conversions: number;
  revenue: number;
  ctr: number;
  conversionRate: number;
  revenuePerSearch: number;
  zeroResultRate: number;
  abandonmentRate: number;
  meanClickRank: number;
}

export interface SignificanceResponse {
  zScore: number;
  pValue: number;
  confidence: number;
  significant: boolean;
  relativeImprovement: number;
  winner: string | null;
}

export interface ConcludeExperimentPayload {
  winner: string | null;
  reason: string;
  controlMetric: number;
  variantMetric: number;
  confidence: number;
  significant: boolean;
  promoted: boolean;
}

export function useConcludeExperiment() {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: async ({ id, payload }: { id: string; payload: ConcludeExperimentPayload }) => {
      const { data } = await api.post(`/2/abtests/${id}/conclude`, payload);
      return data;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['experiments'] });
      queryClient.invalidateQueries({ queryKey: ['experiment-results'] });
      toast({ title: 'Experiment concluded' });
    },
    onError: (error: Error) => {
      toast({
        variant: 'destructive',
        title: 'Failed to conclude experiment',
        description: error.message,
      });
    },
  });
}

export function useExperimentResults(experimentId: string) {
  return useQuery<ExperimentResultsResponse>({
    queryKey: ['experiment-results', experimentId],
    queryFn: async () => {
      const { data } = await api.get(`/2/abtests/${experimentId}/results`);
      return data;
    },
    enabled: !!experimentId,
    refetchInterval: 30000,
    retry: 1,
  });
}
