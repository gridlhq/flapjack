import { useQuery } from '@tanstack/react-query';
import api from '@/lib/api';

export interface DateRange {
  startDate: string;
  endDate: string;
}

function dateParams(range: DateRange) {
  return `startDate=${range.startDate}&endDate=${range.endDate}`;
}

export function defaultRange(days: number): DateRange {
  const end = new Date();
  const start = new Date();
  start.setDate(start.getDate() - days);
  return {
    startDate: start.toISOString().slice(0, 10),
    endDate: end.toISOString().slice(0, 10),
  };
}

/** Returns the previous period of the same length, ending where `range` starts. */
export function previousRange(range: DateRange): DateRange {
  const start = new Date(range.startDate);
  const end = new Date(range.endDate);
  const days = Math.round((end.getTime() - start.getTime()) / (1000 * 60 * 60 * 24));
  const prevEnd = new Date(start);
  prevEnd.setDate(prevEnd.getDate() - 1);
  const prevStart = new Date(prevEnd);
  prevStart.setDate(prevStart.getDate() - days + 1);
  return {
    startDate: prevStart.toISOString().slice(0, 10),
    endDate: prevEnd.toISOString().slice(0, 10),
  };
}

export function useTopSearches(index: string, range: DateRange, limit = 10, clickAnalytics = false, country?: string, tags?: string) {
  return useQuery({
    queryKey: ['analytics', 'topSearches', index, range, limit, clickAnalytics, country, tags],
    queryFn: async () => {
      let url = `/2/searches?index=${encodeURIComponent(index)}&${dateParams(range)}&limit=${limit}&clickAnalytics=${clickAnalytics}`;
      if (country) url += `&country=${encodeURIComponent(country)}`;
      if (tags) url += `&tags=${encodeURIComponent(tags)}`;
      const { data } = await api.get(url);
      return data;
    },
    enabled: !!index,
    staleTime: 60000,
    retry: 1,
  });
}

export function useSearchCount(index: string, range: DateRange) {
  return useQuery({
    queryKey: ['analytics', 'searchCount', index, range],
    queryFn: async () => {
      const { data } = await api.get(
        `/2/searches/count?index=${encodeURIComponent(index)}&${dateParams(range)}`
      );
      return data;
    },
    enabled: !!index,
    staleTime: 60000,
    retry: 1,
  });
}

export function useNoResults(index: string, range: DateRange, limit = 1000) {
  return useQuery({
    queryKey: ['analytics', 'noResults', index, range, limit],
    queryFn: async () => {
      const { data } = await api.get(
        `/2/searches/noResults?index=${encodeURIComponent(index)}&${dateParams(range)}&limit=${limit}`
      );
      return data;
    },
    enabled: !!index,
    staleTime: 60000,
    retry: 1,
  });
}

export function useNoResultRate(index: string, range: DateRange) {
  return useQuery({
    queryKey: ['analytics', 'noResultRate', index, range],
    queryFn: async () => {
      const { data } = await api.get(
        `/2/searches/noResultRate?index=${encodeURIComponent(index)}&${dateParams(range)}`
      );
      return data;
    },
    enabled: !!index,
    staleTime: 60000,
    retry: 1,
  });
}

export function useNoClickRate(index: string, range: DateRange) {
  return useQuery({
    queryKey: ['analytics', 'noClickRate', index, range],
    queryFn: async () => {
      const { data } = await api.get(
        `/2/searches/noClickRate?index=${encodeURIComponent(index)}&${dateParams(range)}`
      );
      return data;
    },
    enabled: !!index,
    staleTime: 60000,
    retry: 1,
  });
}

export function useClickThroughRate(index: string, range: DateRange) {
  return useQuery({
    queryKey: ['analytics', 'ctr', index, range],
    queryFn: async () => {
      const { data } = await api.get(
        `/2/clicks/clickThroughRate?index=${encodeURIComponent(index)}&${dateParams(range)}`
      );
      return data;
    },
    enabled: !!index,
    staleTime: 60000,
    retry: 1,
  });
}

export function useAverageClickPosition(index: string, range: DateRange) {
  return useQuery({
    queryKey: ['analytics', 'avgClickPos', index, range],
    queryFn: async () => {
      const { data } = await api.get(
        `/2/clicks/averageClickPosition?index=${encodeURIComponent(index)}&${dateParams(range)}`
      );
      return data;
    },
    enabled: !!index,
    staleTime: 60000,
    retry: 1,
  });
}

export function useClickPositions(index: string, range: DateRange) {
  return useQuery({
    queryKey: ['analytics', 'clickPositions', index, range],
    queryFn: async () => {
      const { data } = await api.get(
        `/2/clicks/positions?index=${encodeURIComponent(index)}&${dateParams(range)}`
      );
      return data;
    },
    enabled: !!index,
    staleTime: 60000,
    retry: 1,
  });
}

export function useConversionRate(index: string, range: DateRange) {
  return useQuery({
    queryKey: ['analytics', 'conversionRate', index, range],
    queryFn: async () => {
      const { data } = await api.get(
        `/2/conversions/conversionRate?index=${encodeURIComponent(index)}&${dateParams(range)}`
      );
      return data;
    },
    enabled: !!index,
    staleTime: 60000,
    retry: 1,
  });
}

export function useTopHits(index: string, range: DateRange, limit = 1000) {
  return useQuery({
    queryKey: ['analytics', 'topHits', index, range, limit],
    queryFn: async () => {
      const { data } = await api.get(
        `/2/hits?index=${encodeURIComponent(index)}&${dateParams(range)}&limit=${limit}`
      );
      return data;
    },
    enabled: !!index,
    staleTime: 60000,
    retry: 1,
  });
}

export function useTopFilters(index: string, range: DateRange, limit = 1000) {
  return useQuery({
    queryKey: ['analytics', 'topFilters', index, range, limit],
    queryFn: async () => {
      const { data } = await api.get(
        `/2/filters?index=${encodeURIComponent(index)}&${dateParams(range)}&limit=${limit}`
      );
      return data;
    },
    enabled: !!index,
    staleTime: 60000,
    retry: 1,
  });
}

export function useFilterValues(index: string, attribute: string, range: DateRange, limit = 1000) {
  return useQuery({
    queryKey: ['analytics', 'filterValues', index, attribute, range, limit],
    queryFn: async () => {
      const { data } = await api.get(
        `/2/filters/${encodeURIComponent(attribute)}?index=${encodeURIComponent(index)}&${dateParams(range)}&limit=${limit}`
      );
      return data;
    },
    enabled: !!index && !!attribute,
    staleTime: 60000,
    retry: 1,
  });
}

export function useFiltersNoResults(index: string, range: DateRange, limit = 1000) {
  return useQuery({
    queryKey: ['analytics', 'filtersNoResults', index, range, limit],
    queryFn: async () => {
      const { data } = await api.get(
        `/2/filters/noResults?index=${encodeURIComponent(index)}&${dateParams(range)}&limit=${limit}`
      );
      return data;
    },
    enabled: !!index,
    staleTime: 60000,
    retry: 1,
  });
}

export function useUsersCount(index: string, range: DateRange) {
  return useQuery({
    queryKey: ['analytics', 'usersCount', index, range],
    queryFn: async () => {
      const { data } = await api.get(
        `/2/users/count?index=${encodeURIComponent(index)}&${dateParams(range)}`
      );
      return data;
    },
    enabled: !!index,
    staleTime: 60000,
    retry: 1,
  });
}

export function useAnalyticsOverview(range: DateRange) {
  return useQuery({
    queryKey: ['analytics', 'overview', range],
    queryFn: async () => {
      const { data } = await api.get(
        `/2/overview?${dateParams(range)}`
      );
      return data;
    },
    staleTime: 60000,
    retry: 1,
  });
}

export function useDeviceBreakdown(index: string, range: DateRange) {
  return useQuery({
    queryKey: ['analytics', 'devices', index, range],
    queryFn: async () => {
      const { data } = await api.get(
        `/2/devices?index=${encodeURIComponent(index)}&${dateParams(range)}`
      );
      return data;
    },
    enabled: !!index,
    staleTime: 60000,
    retry: 1,
  });
}

export function useGeoBreakdown(index: string, range: DateRange) {
  return useQuery({
    queryKey: ['analytics', 'geo', index, range],
    queryFn: async () => {
      const { data } = await api.get(
        `/2/geo?index=${encodeURIComponent(index)}&${dateParams(range)}`
      );
      return data;
    },
    enabled: !!index,
    staleTime: 60000,
    retry: 1,
  });
}

export function useGeoTopSearches(index: string, country: string, range: DateRange) {
  return useQuery({
    queryKey: ['analytics', 'geo', 'topSearches', index, country, range],
    queryFn: async () => {
      const { data } = await api.get(
        `/2/geo/${encodeURIComponent(country)}?index=${encodeURIComponent(index)}&${dateParams(range)}`
      );
      return data;
    },
    enabled: !!index && !!country,
    staleTime: 60000,
    retry: 1,
  });
}

export function useGeoRegions(index: string, country: string, range: DateRange) {
  return useQuery({
    queryKey: ['analytics', 'geo', 'regions', index, country, range],
    queryFn: async () => {
      const { data } = await api.get(
        `/2/geo/${encodeURIComponent(country)}/regions?index=${encodeURIComponent(index)}&${dateParams(range)}`
      );
      return data;
    },
    enabled: !!index && !!country,
    staleTime: 60000,
    retry: 1,
  });
}

export function useAnalyticsStatus(index: string) {
  return useQuery({
    queryKey: ['analytics', 'status', index],
    queryFn: async () => {
      const { data } = await api.get(
        `/2/status?index=${encodeURIComponent(index)}`
      );
      return data;
    },
    enabled: !!index,
    staleTime: 60000,
    retry: 1,
  });
}
