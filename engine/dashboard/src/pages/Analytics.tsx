import React, { useState, useMemo } from 'react';
import { useParams, useNavigate, Link } from 'react-router-dom';
import { useIndices } from '@/hooks/useIndices';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import api from '@/lib/api';
import {
  useSearchCount,
  useUsersCount,
  useNoResultRate,
  useTopSearches,
  useNoResults,
  useTopFilters,
  useFilterValues,
  useFiltersNoResults,
  useDeviceBreakdown,
  useGeoBreakdown,
  useGeoTopSearches,
  useGeoRegions,
  defaultRange,
  previousRange,
  type DateRange,
} from '@/hooks/useAnalytics';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Skeleton } from '@/components/ui/skeleton';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs';
import {
  Search,
  Users,
  AlertCircle,
  CheckCircle2,
  Filter,
  ArrowUpRight,
  ArrowDownRight,
  Minus,
  ChevronDown,
  ChevronRight,
  Loader2,
  Monitor,
  Smartphone,
  Tablet,
  Globe,
  MapPin,
  ChevronLeft as ChevronLeftIcon,
  RefreshCw,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
} from 'recharts';

const RANGE_OPTIONS = [
  { label: '7d', days: 7 },
  { label: '30d', days: 30 },
  { label: '90d', days: 90 },
];

export function Analytics() {
  const { indexName: urlIndexName } = useParams<{ indexName: string }>();
  const navigate = useNavigate();
  const { data: indices } = useIndices();
  const [rangeDays, setRangeDays] = useState(7);
  const queryClient = useQueryClient();

  const range: DateRange = useMemo(() => defaultRange(rangeDays), [rangeDays]);
  const prevRange: DateRange = useMemo(() => previousRange(range), [range]);

  const indexName = urlIndexName || indices?.[0]?.uid || '';

  // If accessed without an index in the URL, redirect to the first available index
  React.useEffect(() => {
    if (!urlIndexName && indices?.length) {
      navigate(`/index/${encodeURIComponent(indices[0].uid)}/analytics`, { replace: true });
    }
  }, [urlIndexName, indices, navigate]);

  const clearMutation = useMutation({
    mutationFn: async (index: string) => {
      const res = await api.delete('/2/analytics/clear', { data: { index } });
      return res.data;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['analytics'] });
    },
  });

  const flushMutation = useMutation({
    mutationFn: async () => {
      const res = await api.post('/2/analytics/flush');
      return res.data;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['analytics'] });
    },
  });

  const rangeLabel = range.startDate && range.endDate
    ? `${formatDateShort(range.startDate)} - ${formatDateShort(range.endDate)}`
    : '';

  return (
    <div className="space-y-6">
      {/* Breadcrumb + Header */}
      <div className="space-y-3">
        {urlIndexName && (
          <div className="flex items-center gap-2 text-sm" data-testid="analytics-breadcrumb">
            <Link to="/overview" className="text-muted-foreground hover:text-foreground transition-colors">
              Overview
            </Link>
            <span className="text-muted-foreground">/</span>
            <Link to={`/index/${encodeURIComponent(urlIndexName)}`} className="text-muted-foreground hover:text-foreground transition-colors font-medium">
              {urlIndexName}
            </Link>
            <span className="text-muted-foreground">/</span>
            <span className="text-foreground font-medium">Analytics</span>
          </div>
        )}
        <div className="flex items-center justify-between flex-wrap gap-4">
          <div>
            <h1 className="text-3xl font-bold" data-testid="analytics-heading">Analytics</h1>
            {rangeLabel && (
              <p className="text-sm text-muted-foreground mt-1" data-testid="analytics-date-label">{rangeLabel}</p>
            )}
          </div>
          <div className="flex items-center gap-3">
            {/* Flush buffered analytics to disk */}
            <Button
              variant="outline"
              size="sm"
              onClick={() => flushMutation.mutate()}
              disabled={flushMutation.isPending}
              title="Flush buffered analytics events to disk and refresh"
            >
              <RefreshCw className={`h-4 w-4 mr-1.5 ${flushMutation.isPending ? 'animate-spin' : ''}`} />
              {flushMutation.isPending ? 'Updating...' : 'Update'}
            </Button>
            {/* Clear analytics button */}
            {indexName && (
              <Button
                variant="outline"
                size="sm"
                onClick={() => {
                  if (confirm(`Clear all analytics data for "${indexName}"?`)) {
                    clearMutation.mutate(indexName);
                  }
                }}
                disabled={clearMutation.isPending}
                title="Delete all analytics data for this index"
              >
                {clearMutation.isPending ? (
                  <Loader2 className="h-4 w-4 mr-1.5 animate-spin" />
                ) : (
                  <AlertCircle className="h-4 w-4 mr-1.5" />
                )}
                {clearMutation.isPending ? 'Clearing...' : 'Clear Analytics'}
              </Button>
            )}
            {clearMutation.isSuccess && (
              <span className="text-xs text-green-600 flex items-center gap-1">
                <CheckCircle2 className="h-3 w-3" />
                Analytics cleared
              </span>
            )}

            {/* Date range toggle */}
            <div className="flex rounded-md border border-input" data-testid="analytics-date-range">
              {RANGE_OPTIONS.map((opt) => (
                <button
                  key={opt.days}
                  onClick={() => setRangeDays(opt.days)}
                  data-testid={`range-${opt.label}`}
                  className={`px-3 py-1.5 text-sm font-medium transition-colors ${
                    rangeDays === opt.days
                      ? 'bg-primary text-primary-foreground'
                      : 'text-muted-foreground hover:bg-accent'
                  } ${opt.days === 7 ? 'rounded-l-md' : ''} ${opt.days === 90 ? 'rounded-r-md' : ''}`}
                >
                  {opt.label}
                </button>
              ))}
            </div>
          </div>
        </div>
      </div>

      {!indexName ? (
        <Card>
          <CardContent className="py-12 text-center text-muted-foreground">
            <Search className="h-12 w-12 mx-auto mb-4 opacity-30" />
            <h3 className="text-lg font-medium mb-2">No Indices Found</h3>
            <p className="text-sm">Create a demo index (Movies or Products) to get started — analytics data is included automatically.</p>
          </CardContent>
        </Card>
      ) : (
        <Tabs defaultValue="overview" data-testid="analytics-tabs">
          <TabsList>
            <TabsTrigger value="overview" data-testid="tab-overview">Overview</TabsTrigger>
            <TabsTrigger value="searches" data-testid="tab-searches">Searches</TabsTrigger>
            <TabsTrigger value="noResults" data-testid="tab-no-results">No Results</TabsTrigger>
            <TabsTrigger value="filters" data-testid="tab-filters">Filters</TabsTrigger>
            <TabsTrigger value="devices" data-testid="tab-devices">Devices</TabsTrigger>
            <TabsTrigger value="geography" data-testid="tab-geography">Geography</TabsTrigger>
          </TabsList>

          <TabsContent value="overview">
            <OverviewTab index={indexName} range={range} prevRange={prevRange} />
          </TabsContent>
          <TabsContent value="searches">
            <SearchesTab index={indexName} range={range} />
          </TabsContent>
          <TabsContent value="noResults">
            <NoResultsTab index={indexName} range={range} prevRange={prevRange} />
          </TabsContent>
          <TabsContent value="filters">
            <FiltersTab index={indexName} range={range} />
          </TabsContent>
          <TabsContent value="devices">
            <DevicesTab index={indexName} range={range} />
          </TabsContent>
          <TabsContent value="geography">
            <GeographyTab index={indexName} range={range} />
          </TabsContent>
        </Tabs>
      )}
    </div>
  );
}

// ─── Overview Tab ──────────────────────────────────────────────

interface TabProps {
  index: string;
  range: DateRange;
  prevRange?: DateRange;
}

function OverviewTab({ index, range, prevRange }: TabProps) {
  const { data: searchCount, isLoading: countLoading } = useSearchCount(index, range);
  const { data: prevSearchCount } = useSearchCount(index, prevRange!);
  const { data: usersCount, isLoading: usersLoading } = useUsersCount(index, range);
  const { data: prevUsersCount } = useUsersCount(index, prevRange!);
  const { data: noResultRate, isLoading: nrrLoading } = useNoResultRate(index, range);
  const { data: prevNoResultRate } = useNoResultRate(index, prevRange!);
  const { data: topSearches, isLoading: topSearchesLoading } = useTopSearches(index, range, 10, false);

  return (
    <div className="space-y-6 mt-4">
      {/* KPI Cards */}
      <div className="grid gap-4 grid-cols-2 lg:grid-cols-3" data-testid="kpi-cards">
        <KpiCard
          title="Total Searches"
          value={searchCount?.count}
          prevValue={prevSearchCount?.count}
          loading={countLoading}
          icon={Search}
          sparkData={searchCount?.dates}
          sparkKey="count"
          format="number"
          tooltip="Total number of search queries received during this period"
        />
        <KpiCard
          title="Unique Users"
          value={usersCount?.count}
          prevValue={prevUsersCount?.count}
          loading={usersLoading}
          icon={Users}
          format="number"
          tooltip="Number of distinct users who performed searches"
        />
        <KpiCard
          title="No-Result Rate"
          value={noResultRate?.rate}
          prevValue={prevNoResultRate?.rate}
          loading={nrrLoading}
          icon={AlertCircle}
          sparkData={noResultRate?.dates}
          sparkKey="rate"
          format="percent"
          invertDelta
          tooltip="Percentage of searches that returned zero results. Lower is better."
        />
      </div>

      {/* Primary chart: Search Volume */}
      <Card data-testid="search-volume-chart">
        <CardHeader className="pb-2">
          <CardTitle className="text-base font-medium">Search Volume</CardTitle>
        </CardHeader>
        <CardContent>
          {countLoading ? (
            <Skeleton className="h-56 w-full" />
          ) : searchCount?.dates?.length ? (
            <ResponsiveContainer width="100%" height={240}>
              <AreaChart data={searchCount.dates}>
                <defs>
                  <linearGradient id="searchGradient" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0%" stopColor="hsl(var(--primary))" stopOpacity={0.2} />
                    <stop offset="100%" stopColor="hsl(var(--primary))" stopOpacity={0} />
                  </linearGradient>
                </defs>
                <CartesianGrid strokeDasharray="3 3" className="stroke-border" vertical={false} />
                <XAxis
                  dataKey="date"
                  className="text-xs"
                  tickFormatter={(d: string) => formatDateShort(d)}
                  tick={{ fill: 'hsl(var(--muted-foreground))' }}
                />
                <YAxis
                  className="text-xs"
                  tick={{ fill: 'hsl(var(--muted-foreground))' }}
                  width={48}
                />
                <Tooltip
                  contentStyle={{
                    background: 'hsl(var(--card))',
                    border: '1px solid hsl(var(--border))',
                    borderRadius: '8px',
                    fontSize: '13px',
                  }}
                  labelFormatter={(d: any) => formatDateLong(String(d))}
                  formatter={(v: any) => [Number(v).toLocaleString(), 'Searches']}
                />
                <Area
                  type="monotone"
                  dataKey="count"
                  stroke="hsl(var(--primary))"
                  strokeWidth={2}
                  fill="url(#searchGradient)"
                />
              </AreaChart>
            </ResponsiveContainer>
          ) : (
            <EmptyState
              icon={Search}
              title="No search data yet"
              description="Searches will appear here once users start querying your index. Send your first search request to get started."
            />
          )}
        </CardContent>
      </Card>

      {/* Two columns: No-Result Rate chart + Top 10 Searches */}
      <div className="grid gap-4 grid-cols-1 lg:grid-cols-2">
        {/* No-Result Rate over time */}
        <Card data-testid="no-result-rate-chart">
          <CardHeader className="pb-2">
            <CardTitle className="text-base font-medium">No-Result Rate Over Time</CardTitle>
          </CardHeader>
          <CardContent>
            {nrrLoading ? (
              <Skeleton className="h-44 w-full" />
            ) : noResultRate?.dates?.length ? (
              <ResponsiveContainer width="100%" height={180}>
                <AreaChart data={noResultRate.dates}>
                  <defs>
                    <linearGradient id="nrrGradient" x1="0" y1="0" x2="0" y2="1">
                      <stop offset="0%" stopColor="#f59e0b" stopOpacity={0.2} />
                      <stop offset="100%" stopColor="#f59e0b" stopOpacity={0} />
                    </linearGradient>
                  </defs>
                  <CartesianGrid strokeDasharray="3 3" className="stroke-border" vertical={false} />
                  <XAxis dataKey="date" className="text-xs" tickFormatter={(d: string) => formatDateShort(d)} tick={{ fill: 'hsl(var(--muted-foreground))' }} />
                  <YAxis className="text-xs" tickFormatter={(v: number) => `${(v * 100).toFixed(0)}%`} tick={{ fill: 'hsl(var(--muted-foreground))' }} width={40} />
                  <Tooltip
                    contentStyle={{ background: 'hsl(var(--card))', border: '1px solid hsl(var(--border))', borderRadius: '8px', fontSize: '13px' }}
                    labelFormatter={(d: any) => formatDateLong(String(d))}
                    formatter={(v: any) => [`${((v as number) * 100).toFixed(1)}%`, 'No-Result Rate']}
                  />
                  <Area type="monotone" dataKey="rate" stroke="#f59e0b" strokeWidth={2} fill="url(#nrrGradient)" />
                </AreaChart>
              </ResponsiveContainer>
            ) : (
              <div className="h-44 flex items-center justify-center text-sm text-muted-foreground">No data available</div>
            )}
          </CardContent>
        </Card>

        {/* Top 10 Searches */}
        <Card data-testid="top-searches-overview">
          <CardHeader className="pb-2">
            <CardTitle className="text-base font-medium">Top 10 Searches</CardTitle>
          </CardHeader>
          <CardContent>
            {topSearchesLoading ? (
              <TableSkeleton rows={5} />
            ) : topSearches?.searches?.length ? (
              <div className="overflow-x-auto">
                <table className="w-full text-sm">
                  <thead>
                    <tr className="border-b border-border text-left text-muted-foreground">
                      <th className="py-2 pr-3 font-medium w-6">#</th>
                      <th className="py-2 pr-3 font-medium">Query</th>
                      <th className="py-2 font-medium text-right">Count</th>
                    </tr>
                  </thead>
                  <tbody>
                    {topSearches.searches.slice(0, 10).map((s: any, i: number) => (
                      <tr key={i} className="border-b border-border/50">
                        <td className="py-1.5 pr-3 text-muted-foreground text-xs">{i + 1}</td>
                        <td className="py-1.5 pr-3 font-mono text-sm truncate max-w-[200px]">
                          {s.search || <span className="text-muted-foreground italic">(empty)</span>}
                        </td>
                        <td className="py-1.5 text-right tabular-nums">{s.count?.toLocaleString()}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            ) : (
              <div className="h-44 flex items-center justify-center text-sm text-muted-foreground">No search data yet</div>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}

// ─── Searches Tab ──────────────────────────────────────────────

function SearchesTab({ index, range }: TabProps) {
  const [countryFilter, setCountryFilter] = useState('');
  const [deviceFilter, setDeviceFilter] = useState('');
  const tagsParam = deviceFilter ? `platform:${deviceFilter}` : undefined;
  const { data, isLoading, error } = useTopSearches(index, range, 100, false, countryFilter || undefined, tagsParam);
  const { data: geoData } = useGeoBreakdown(index, range);
  const { data: deviceData } = useDeviceBreakdown(index, range);
  const [sortCol, setSortCol] = useState<string>('count');
  const [sortDir, setSortDir] = useState<'asc' | 'desc'>('desc');
  const [filter, setFilter] = useState('');

  const countries: any[] = geoData?.countries || [];
  const platforms: any[] = (deviceData?.platforms || []).filter((p: any) => p.platform !== 'unknown');

  const maxCount = useMemo(() => {
    if (!data?.searches?.length) return 1;
    return Math.max(...data.searches.map((s: any) => s.count || 0));
  }, [data]);

  const sorted = useMemo(() => {
    if (!data?.searches) return [];
    let list = [...data.searches];
    if (filter) {
      list = list.filter((s: any) => (s.search || '').toLowerCase().includes(filter.toLowerCase()));
    }
    list.sort((a: any, b: any) => {
      const av = a[sortCol] ?? 0;
      const bv = b[sortCol] ?? 0;
      return sortDir === 'desc' ? bv - av : av - bv;
    });
    return list;
  }, [data, sortCol, sortDir, filter]);

  function toggleSort(col: string) {
    if (sortCol === col) {
      setSortDir(sortDir === 'desc' ? 'asc' : 'desc');
    } else {
      setSortCol(col);
      setSortDir('desc');
    }
  }

  function SortHeader({ col, label, align }: { col: string; label: string; align?: string }) {
    const active = sortCol === col;
    return (
      <th
        className={`py-2.5 pr-4 font-medium cursor-pointer select-none hover:text-foreground transition-colors ${align || 'text-left'}`}
        onClick={() => toggleSort(col)}
      >
        <span className="inline-flex items-center gap-1">
          {label}
          {active && <span className="text-xs">{sortDir === 'desc' ? '\u2193' : '\u2191'}</span>}
        </span>
      </th>
    );
  }

  return (
    <div className="mt-4 space-y-4">
      {/* Search filter + geo/device dropdowns */}
      <div className="flex items-center gap-2 flex-wrap" data-testid="searches-filter">
        <Search className="h-4 w-4 text-muted-foreground" />
        <input
          type="text"
          placeholder="Filter queries..."
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          className="h-8 rounded-md border border-input bg-background px-3 text-sm flex-1 max-w-xs"
          data-testid="searches-filter-input"
        />
        {countries.length > 0 && (
          <select
            value={countryFilter}
            onChange={(e) => setCountryFilter(e.target.value)}
            className="h-8 rounded-md border border-input bg-background px-2 text-sm"
            data-testid="searches-country-filter"
          >
            <option value="">All Countries</option>
            {countries.map((c: any) => (
              <option key={c.country} value={c.country}>
                {COUNTRY_NAMES[c.country as string] || c.country} ({c.count?.toLocaleString()})
              </option>
            ))}
          </select>
        )}
        {platforms.length > 0 && (
          <select
            value={deviceFilter}
            onChange={(e) => setDeviceFilter(e.target.value)}
            className="h-8 rounded-md border border-input bg-background px-2 text-sm"
            data-testid="searches-device-filter"
          >
            <option value="">All Devices</option>
            {platforms.map((p: any) => (
              <option key={p.platform} value={p.platform}>
                {p.platform.charAt(0).toUpperCase() + p.platform.slice(1)} ({p.count?.toLocaleString()})
              </option>
            ))}
          </select>
        )}
      </div>

      <Card data-testid="top-searches-table">
        <CardHeader className="pb-2">
          <div className="flex items-center justify-between">
            <CardTitle className="text-base font-medium">Top Searches</CardTitle>
            {data?.searches?.length > 0 && (
              <span className="text-xs text-muted-foreground">{sorted.length} queries</span>
            )}
          </div>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <TableSkeleton rows={8} />
          ) : error ? (
            <ErrorState message={error instanceof Error ? error.message : 'Failed to load'} />
          ) : sorted.length > 0 ? (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-border text-muted-foreground">
                    <th className="py-2.5 pr-4 font-medium text-left w-8">#</th>
                    <SortHeader col="search" label="Query" />
                    <SortHeader col="count" label="Count" align="text-right" />
                    <th className="py-2.5 pr-4 font-medium text-right w-32">Volume</th>
                    <SortHeader col="nbHits" label="Avg Hits" align="text-right" />
                  </tr>
                </thead>
                <tbody>
                  {sorted.map((s: any, i: number) => (
                    <tr key={i} className="border-b border-border/50 hover:bg-accent/30 transition-colors">
                      <td className="py-2.5 pr-4 text-muted-foreground text-xs">{i + 1}</td>
                      <td className="py-2.5 pr-4">
                        <span className="font-mono text-sm">{s.search || <span className="text-muted-foreground italic">(empty query)</span>}</span>
                      </td>
                      <td className="py-2.5 pr-4 text-right tabular-nums">{s.count?.toLocaleString()}</td>
                      <td className="py-2.5 pr-4">
                        <div className="flex items-center justify-end gap-2">
                          <div className="w-24 h-2 bg-muted rounded-full overflow-hidden">
                            <div
                              className="h-full bg-primary/60 rounded-full transition-all"
                              style={{ width: `${((s.count || 0) / maxCount) * 100}%` }}
                            />
                          </div>
                        </div>
                      </td>
                      <td className="py-2.5 text-right tabular-nums">{s.nbHits ?? '-'}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : (
            <EmptyState
              icon={Search}
              title="No searches recorded yet"
              description="Top search queries will appear here as users search your index."
            />
          )}
        </CardContent>
      </Card>
    </div>
  );
}

// ─── No Results Tab ────────────────────────────────────────────

function NoResultsTab({ index, range, prevRange }: TabProps) {
  const { data, isLoading, error } = useNoResults(index, range, 100);
  const { data: rateData, isLoading: rateLoading } = useNoResultRate(index, range);
  const { data: prevRateData } = useNoResultRate(index, prevRange!);

  const isClean = !rateLoading && rateData?.rate != null && rateData.rate === 0;

  return (
    <div className="space-y-6 mt-4">
      {/* Rate banner */}
      {rateData?.rate != null && (
        <Card data-testid="no-result-rate-banner">
          <CardContent className="py-5">
            <div className="flex items-center gap-4">
              {isClean ? (
                <CheckCircle2 className="h-10 w-10 text-green-500 shrink-0" />
              ) : (
                <AlertCircle className={`h-10 w-10 shrink-0 ${rateData.rate > 0.1 ? 'text-red-500' : 'text-amber-500'}`} />
              )}
              <div className="flex-1">
                <div className="flex items-baseline gap-3">
                  <span className={`text-3xl font-bold ${rateData.rate > 0.1 ? 'text-red-500' : isClean ? 'text-green-500' : ''}`}>
                    {(rateData.rate * 100).toFixed(1)}%
                  </span>
                  <DeltaBadge current={rateData.rate} previous={prevRateData?.rate} invertColor />
                </div>
                <div className="text-sm text-muted-foreground mt-0.5">
                  {isClean
                    ? 'All searches return results. Your content coverage is excellent.'
                    : rateData.rate > 0.1
                      ? 'of searches return no results. Consider adding synonyms or content for the queries below.'
                      : 'of searches return no results'}
                </div>
              </div>
            </div>
          </CardContent>
        </Card>
      )}

      <Card data-testid="no-results-table">
        <CardHeader className="pb-2">
          <CardTitle className="text-base font-medium">Searches With No Results</CardTitle>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <TableSkeleton rows={5} />
          ) : error ? (
            <ErrorState message={error instanceof Error ? error.message : 'Failed to load'} />
          ) : data?.searches?.length ? (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-border text-left text-muted-foreground">
                    <th className="py-2.5 pr-4 font-medium w-8">#</th>
                    <th className="py-2.5 pr-4 font-medium">Query</th>
                    <th className="py-2.5 font-medium text-right">Count</th>
                  </tr>
                </thead>
                <tbody>
                  {data.searches.map((s: any, i: number) => (
                    <tr key={i} className="border-b border-border/50 hover:bg-accent/30 transition-colors">
                      <td className="py-2.5 pr-4 text-muted-foreground text-xs">{i + 1}</td>
                      <td className="py-2.5 pr-4">
                        <span className="font-mono">{s.search || <span className="text-muted-foreground italic">(empty query)</span>}</span>
                      </td>
                      <td className="py-2.5 text-right tabular-nums">{s.count?.toLocaleString()}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : isClean ? (
            <EmptyState
              icon={CheckCircle2}
              title="No zero-result searches"
              description="All queries are returning results. Your content coverage is excellent."
              positive
            />
          ) : (
            <EmptyState
              icon={Search}
              title="No search data yet"
              description="Zero-result searches will appear here once users start querying your index."
            />
          )}
        </CardContent>
      </Card>
    </div>
  );
}

// ─── Filters Tab ───────────────────────────────────────────────

function FilterValueRow({ index, attribute, range }: { index: string; attribute: string; range: DateRange }) {
  const { data, isLoading } = useFilterValues(index, attribute, range, 10);
  if (isLoading) return <tr><td colSpan={4} className="py-2 pl-12 text-muted-foreground text-xs">Loading values...</td></tr>;
  if (!data?.values?.length) return <tr><td colSpan={4} className="py-2 pl-12 text-muted-foreground text-xs">No values found</td></tr>;
  return (
    <>
      {data.values.map((v: any, j: number) => (
        <tr key={j} className="bg-accent/10">
          <td className="py-1.5 pr-4" />
          <td className="py-1.5 pr-4 pl-8 text-xs text-muted-foreground font-mono">{v.value}</td>
          <td className="py-1.5 pr-4 text-right tabular-nums text-xs">{v.count?.toLocaleString()}</td>
          <td className="py-1.5" />
        </tr>
      ))}
    </>
  );
}

function FiltersTab({ index, range }: TabProps) {
  const { data, isLoading, error } = useTopFilters(index, range, 100);
  const { data: noResultFilters } = useFiltersNoResults(index, range, 20);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());

  const maxCount = useMemo(() => {
    if (!data?.filters?.length) return 1;
    return Math.max(...data.filters.map((f: any) => f.count || 0));
  }, [data]);

  const toggleExpand = (attr: string) => {
    setExpanded(prev => {
      const next = new Set(prev);
      if (next.has(attr)) next.delete(attr);
      else next.add(attr);
      return next;
    });
  };

  // Extract attribute name from filter string like "brand:Apple" -> "brand"
  const extractAttr = (filterStr: string) => {
    const idx = filterStr.indexOf(':');
    return idx >= 0 ? filterStr.substring(0, idx) : filterStr;
  };

  return (
    <div className="mt-4 space-y-4">
      <Card data-testid="filters-table">
        <CardHeader className="pb-2">
          <CardTitle className="text-base font-medium">Top Filter Attributes</CardTitle>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <TableSkeleton rows={5} />
          ) : error ? (
            <ErrorState message={error instanceof Error ? error.message : 'Failed to load'} />
          ) : data?.filters?.length ? (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-border text-left text-muted-foreground">
                    <th className="py-2.5 pr-4 font-medium w-8" />
                    <th className="py-2.5 pr-4 font-medium">Attribute</th>
                    <th className="py-2.5 pr-4 font-medium text-right">Count</th>
                    <th className="py-2.5 font-medium text-right w-32">Usage</th>
                  </tr>
                </thead>
                <tbody>
                  {data.filters.map((f: any, i: number) => {
                    const attr = extractAttr(f.attribute);
                    const isExpanded = expanded.has(attr);
                    return (
                      <React.Fragment key={i}>
                        <tr
                          className="border-b border-border/50 hover:bg-accent/30 transition-colors cursor-pointer"
                          onClick={() => toggleExpand(attr)}
                        >
                          <td className="py-2.5 pr-4 text-muted-foreground text-xs">
                            {isExpanded ? <ChevronDown className="w-3 h-3 inline" /> : <ChevronRight className="w-3 h-3 inline" />}
                          </td>
                          <td className="py-2.5 pr-4 font-mono">{f.attribute}</td>
                          <td className="py-2.5 pr-4 text-right tabular-nums">{f.count?.toLocaleString()}</td>
                          <td className="py-2.5">
                            <div className="flex items-center justify-end gap-2">
                              <div className="w-24 h-2 bg-muted rounded-full overflow-hidden">
                                <div
                                  className="h-full bg-primary/60 rounded-full transition-all"
                                  style={{ width: `${((f.count || 0) / maxCount) * 100}%` }}
                                />
                              </div>
                            </div>
                          </td>
                        </tr>
                        {isExpanded && (
                          <FilterValueRow index={index} attribute={attr} range={range} />
                        )}
                      </React.Fragment>
                    );
                  })}
                </tbody>
              </table>
            </div>
          ) : (
            <EmptyState
              icon={Filter}
              title="No filter usage recorded"
              description="Filter analytics will appear here when users apply facet filters in their searches. Make sure you have attributesForFaceting configured."
            />
          )}
        </CardContent>
      </Card>

      {/* Filters causing no results */}
      {noResultFilters?.filters?.length > 0 && (
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-base font-medium flex items-center gap-2">
              <AlertCircle className="w-4 h-4 text-amber-500" />
              Filters Causing No Results
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-border text-left text-muted-foreground">
                    <th className="py-2.5 pr-4 font-medium">Filter</th>
                    <th className="py-2.5 font-medium text-right">Times Used</th>
                  </tr>
                </thead>
                <tbody>
                  {noResultFilters.filters.map((f: any, i: number) => (
                    <tr key={i} className="border-b border-border/50">
                      <td className="py-2.5 pr-4 font-mono text-amber-600 dark:text-amber-400">{f.attribute}</td>
                      <td className="py-2.5 text-right tabular-nums">{f.count?.toLocaleString()}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </CardContent>
        </Card>
      )}
    </div>
  );
}

// ─── Devices Tab ──────────────────────────────────────────────

const PLATFORM_META: Record<string, { label: string; icon: any; color: string }> = {
  desktop: { label: 'Desktop', icon: Monitor, color: 'hsl(var(--primary))' },
  mobile: { label: 'Mobile', icon: Smartphone, color: 'hsl(210, 80%, 55%)' },
  tablet: { label: 'Tablet', icon: Tablet, color: 'hsl(150, 60%, 45%)' },
  unknown: { label: 'Unknown', icon: Search, color: 'hsl(var(--muted-foreground))' },
};

function DevicesTab({ index, range }: TabProps) {
  const { data, isLoading } = useDeviceBreakdown(index, range);

  const platforms: any[] = data?.platforms || [];
  const dailyData: any[] = data?.dates || [];

  const total = platforms.reduce((sum: number, p: any) => sum + (p.count || 0), 0);

  // Pivot daily data for stacked area chart: { date, desktop, mobile, tablet }
  const chartData = useMemo(() => {
    const byDate: Record<string, Record<string, number>> = {};
    for (const row of dailyData) {
      if (!byDate[row.date]) byDate[row.date] = {};
      byDate[row.date][row.platform] = row.count;
    }
    return Object.entries(byDate)
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([date, counts]) => ({
        date: formatDateShort(date),
        desktop: counts.desktop || 0,
        mobile: counts.mobile || 0,
        tablet: counts.tablet || 0,
      }));
  }, [dailyData]);

  if (isLoading) {
    return (
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        {[1, 2, 3].map((i) => (
          <Card key={i}><CardContent className="py-6"><Skeleton className="h-16 w-full" /></CardContent></Card>
        ))}
      </div>
    );
  }

  if (!platforms.length || total === 0) {
    return (
      <Card>
        <CardContent className="py-12">
          <EmptyState
            icon={Smartphone}
            title="No device data"
            description="Device breakdown requires analytics_tags with platform:desktop, platform:mobile, or platform:tablet. Create a demo index to get sample data, or add analyticsTags to your search requests."
          />
        </CardContent>
      </Card>
    );
  }

  return (
    <div className="space-y-4">
      {/* Platform KPI cards */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        {platforms
          .filter((p: any) => p.platform !== 'unknown')
          .map((p: any) => {
            const meta = PLATFORM_META[p.platform] || PLATFORM_META.unknown;
            const PIcon = meta.icon;
            const pct = total > 0 ? ((p.count / total) * 100).toFixed(1) : '0';
            return (
              <Card key={p.platform} data-testid={`device-${p.platform}`}>
                <CardContent className="py-5">
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-3">
                      <div className="p-2 rounded-lg bg-muted">
                        <PIcon className="h-5 w-5" />
                      </div>
                      <div>
                        <div className="text-sm font-medium text-muted-foreground">{meta.label}</div>
                        <div className="text-2xl font-bold tabular-nums">{(p.count as number).toLocaleString()}</div>
                      </div>
                    </div>
                    <div className="text-lg font-semibold text-muted-foreground">{pct}%</div>
                  </div>
                  {/* Mini progress bar */}
                  <div className="mt-3 h-1.5 bg-muted rounded-full overflow-hidden">
                    <div
                      className="h-full rounded-full transition-all"
                      style={{ width: `${pct}%`, backgroundColor: meta.color }}
                    />
                  </div>
                </CardContent>
              </Card>
            );
          })}
      </div>

      {/* Stacked area chart */}
      {chartData.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle className="text-sm font-medium">Searches by Device Over Time</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="h-64">
              <ResponsiveContainer width="100%" height="100%">
                <AreaChart data={chartData}>
                  <CartesianGrid strokeDasharray="3 3" className="stroke-border" />
                  <XAxis dataKey="date" className="text-xs" tick={{ fontSize: 11 }} />
                  <YAxis className="text-xs" tick={{ fontSize: 11 }} />
                  <Tooltip
                    contentStyle={{
                      backgroundColor: 'hsl(var(--popover))',
                      border: '1px solid hsl(var(--border))',
                      borderRadius: '6px',
                      fontSize: '12px',
                    }}
                  />
                  <Area type="monotone" dataKey="desktop" stackId="1" stroke={PLATFORM_META.desktop.color} fill={PLATFORM_META.desktop.color} fillOpacity={0.6} />
                  <Area type="monotone" dataKey="mobile" stackId="1" stroke={PLATFORM_META.mobile.color} fill={PLATFORM_META.mobile.color} fillOpacity={0.6} />
                  <Area type="monotone" dataKey="tablet" stackId="1" stroke={PLATFORM_META.tablet.color} fill={PLATFORM_META.tablet.color} fillOpacity={0.6} />
                </AreaChart>
              </ResponsiveContainer>
            </div>
          </CardContent>
        </Card>
      )}
    </div>
  );
}

// ─── Geography Tab ────────────────────────────────────────────

const COUNTRY_NAMES: Record<string, string> = {
  US: 'United States', GB: 'United Kingdom', DE: 'Germany', FR: 'France',
  CA: 'Canada', AU: 'Australia', NL: 'Netherlands', JP: 'Japan',
  BR: 'Brazil', IN: 'India', ES: 'Spain', IT: 'Italy', SE: 'Sweden',
  MX: 'Mexico', KR: 'South Korea', SG: 'Singapore', CN: 'China',
  RU: 'Russia', PL: 'Poland', CH: 'Switzerland', AT: 'Austria',
  BE: 'Belgium', DK: 'Denmark', NO: 'Norway', FI: 'Finland',
  IE: 'Ireland', PT: 'Portugal', NZ: 'New Zealand', AR: 'Argentina',
  CL: 'Chile', CO: 'Colombia', ZA: 'South Africa', IL: 'Israel',
  TH: 'Thailand', MY: 'Malaysia', PH: 'Philippines', ID: 'Indonesia',
  TW: 'Taiwan', HK: 'Hong Kong', AE: 'United Arab Emirates',
};

function countryFlag(code: string): string {
  try {
    return String.fromCodePoint(
      ...code.toUpperCase().split('').map((c) => 0x1f1e6 + c.charCodeAt(0) - 65)
    );
  } catch {
    return code;
  }
}

function GeographyTab({ index, range }: TabProps) {
  const { data, isLoading } = useGeoBreakdown(index, range);
  const [selectedCountry, setSelectedCountry] = useState<string | null>(null);

  const countries: any[] = data?.countries || [];
  const total: number = data?.total || 0;

  if (isLoading) {
    return (
      <Card><CardContent className="py-6"><TableSkeleton rows={8} /></CardContent></Card>
    );
  }

  if (!countries.length) {
    return (
      <Card>
        <CardContent className="py-12">
          <EmptyState
            icon={Globe}
            title="No geographic data"
            description="Geographic breakdown requires country data in search events. Create a demo index to get sample data with geographic distribution."
          />
        </CardContent>
      </Card>
    );
  }

  if (selectedCountry) {
    return (
      <CountryDrillDown
        index={index}
        range={range}
        country={selectedCountry}
        onBack={() => setSelectedCountry(null)}
      />
    );
  }

  return (
    <div className="space-y-4">
      {/* Summary */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        <Card data-testid="geo-countries-count">
          <CardHeader className="pb-1">
            <CardTitle className="text-xs font-medium text-muted-foreground flex items-center gap-1.5 uppercase tracking-wide">
              <Globe className="h-3.5 w-3.5" />
              Countries
            </CardTitle>
          </CardHeader>
          <CardContent className="pb-3">
            <span className="text-2xl font-bold tabular-nums">{countries.length}</span>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-1">
            <CardTitle className="text-xs font-medium text-muted-foreground flex items-center gap-1.5 uppercase tracking-wide">
              <MapPin className="h-3.5 w-3.5" />
              Total Searches
            </CardTitle>
          </CardHeader>
          <CardContent className="pb-3">
            <span className="text-2xl font-bold tabular-nums">{total.toLocaleString()}</span>
          </CardContent>
        </Card>
      </div>

      {/* Country table */}
      <Card>
        <CardHeader>
          <CardTitle className="text-sm font-medium">Searches by Country</CardTitle>
        </CardHeader>
        <CardContent>
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b text-left text-muted-foreground">
                <th className="pb-2 font-medium w-8">#</th>
                <th className="pb-2 font-medium">Country</th>
                <th className="pb-2 font-medium text-right">Searches</th>
                <th className="pb-2 font-medium text-right w-20">Share</th>
                <th className="pb-2 w-32"></th>
              </tr>
            </thead>
            <tbody>
              {countries.map((c: any, i: number) => {
                const code = c.country as string;
                const count = (c.count as number) || 0;
                const pct = total > 0 ? (count / total) * 100 : 0;
                const name = COUNTRY_NAMES[code] || code;
                return (
                  <tr
                    key={code}
                    className="border-b border-border/50 hover:bg-muted/50 cursor-pointer transition-colors"
                    onClick={() => setSelectedCountry(code)}
                  >
                    <td className="py-2.5 text-muted-foreground tabular-nums">{i + 1}</td>
                    <td className="py-2.5">
                      <span className="mr-2">{countryFlag(code)}</span>
                      <span className="font-medium">{name}</span>
                      <span className="text-muted-foreground ml-1.5 text-xs">({code})</span>
                    </td>
                    <td className="py-2.5 text-right tabular-nums font-medium">{count.toLocaleString()}</td>
                    <td className="py-2.5 text-right tabular-nums text-muted-foreground">{pct.toFixed(1)}%</td>
                    <td className="py-2.5 pl-3">
                      <div className="h-1.5 bg-muted rounded-full overflow-hidden">
                        <div
                          className="h-full bg-primary rounded-full"
                          style={{ width: `${Math.min(pct * 2, 100)}%` }}
                        />
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </CardContent>
      </Card>
    </div>
  );
}

function CountryDrillDown({
  index,
  range,
  country,
  onBack,
}: {
  index: string;
  range: DateRange;
  country: string;
  onBack: () => void;
}) {
  const { data, isLoading } = useGeoTopSearches(index, country, range);
  const { data: regionsData, isLoading: regionsLoading } = useGeoRegions(index, country, range);
  const searches: any[] = data?.searches || [];
  const regions: any[] = regionsData?.regions || [];
  const name = COUNTRY_NAMES[country] || country;

  const regionsTotal = regions.reduce((sum: number, r: any) => sum + (r.count || 0), 0);

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-2">
        <Button variant="ghost" size="sm" onClick={onBack}>
          <ChevronLeftIcon className="h-4 w-4 mr-1" />
          All Countries
        </Button>
        <span className="text-lg font-semibold">
          {countryFlag(country)} {name}
        </span>
      </div>

      <div className="grid gap-4 grid-cols-1 lg:grid-cols-2">
        {/* Top Searches */}
        <Card>
          <CardHeader>
            <CardTitle className="text-sm font-medium">Top Searches from {name}</CardTitle>
          </CardHeader>
          <CardContent>
            {isLoading ? (
              <TableSkeleton rows={5} />
            ) : !searches.length ? (
              <EmptyState icon={Search} title="No searches" description={`No search data from ${name} in this period.`} />
            ) : (
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b text-left text-muted-foreground">
                    <th className="pb-2 font-medium w-8">#</th>
                    <th className="pb-2 font-medium">Query</th>
                    <th className="pb-2 font-medium text-right">Count</th>
                  </tr>
                </thead>
                <tbody>
                  {searches.map((s: any, i: number) => (
                    <tr key={i} className="border-b border-border/50">
                      <td className="py-2.5 text-muted-foreground tabular-nums">{i + 1}</td>
                      <td className="py-2.5 font-mono">{s.search || '(empty)'}</td>
                      <td className="py-2.5 text-right tabular-nums font-medium">{(s.count as number)?.toLocaleString()}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </CardContent>
        </Card>

        {/* Region/State Breakdown */}
        <Card>
          <CardHeader>
            <CardTitle className="text-sm font-medium flex items-center gap-2">
              <MapPin className="h-4 w-4" />
              {country === 'US' ? 'States' : 'Regions'}
            </CardTitle>
          </CardHeader>
          <CardContent>
            {regionsLoading ? (
              <TableSkeleton rows={5} />
            ) : !regions.length ? (
              <div className="py-8 text-center text-sm text-muted-foreground">No region data available</div>
            ) : (
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b text-left text-muted-foreground">
                    <th className="pb-2 font-medium w-8">#</th>
                    <th className="pb-2 font-medium">{country === 'US' ? 'State' : 'Region'}</th>
                    <th className="pb-2 font-medium text-right">Searches</th>
                    <th className="pb-2 font-medium text-right w-16">Share</th>
                  </tr>
                </thead>
                <tbody>
                  {regions.map((r: any, i: number) => {
                    const pct = regionsTotal > 0 ? ((r.count || 0) / regionsTotal * 100) : 0;
                    return (
                      <tr key={i} className="border-b border-border/50">
                        <td className="py-2 text-muted-foreground tabular-nums text-xs">{i + 1}</td>
                        <td className="py-2 font-medium">{r.region}</td>
                        <td className="py-2 text-right tabular-nums">{(r.count as number)?.toLocaleString()}</td>
                        <td className="py-2 text-right tabular-nums text-muted-foreground text-xs">{pct.toFixed(1)}%</td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}

// ─── Shared Components ─────────────────────────────────────────

function KpiCard({
  title,
  value,
  prevValue,
  loading,
  icon: Icon,
  sparkData,
  sparkKey,
  format = 'number',
  invertDelta,
  emptyText,
  tooltip,
}: {
  title: string;
  value: any;
  prevValue?: any;
  loading: boolean;
  icon: any;
  sparkData?: any[];
  sparkKey?: string;
  format?: 'number' | 'percent' | 'decimal';
  invertDelta?: boolean;
  emptyText?: string;
  tooltip?: string;
}) {
  const formattedValue = useMemo(() => {
    if (value == null) return null;
    if (format === 'percent') return `${(value * 100).toFixed(1)}%`;
    if (format === 'decimal') return Number(value).toFixed(1);
    return typeof value === 'number' ? value.toLocaleString() : value;
  }, [value, format]);

  return (
    <Card className="relative group" data-testid={`kpi-${title.toLowerCase().replace(/\s+/g, '-')}`}>
      {tooltip && (
        <div className="absolute top-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity">
          <div className="relative">
            <div className="hidden group-hover:block absolute right-0 top-6 w-56 p-2 bg-popover border border-border rounded-md shadow-md text-xs text-muted-foreground z-10">
              {tooltip}
            </div>
          </div>
        </div>
      )}
      <CardHeader className="pb-1">
        <CardTitle className="text-xs font-medium text-muted-foreground flex items-center gap-1.5 uppercase tracking-wide">
          <Icon className="h-3.5 w-3.5" />
          {title}
        </CardTitle>
      </CardHeader>
      <CardContent className="pb-3">
        {loading ? (
          <Skeleton className="h-8 w-20" />
        ) : formattedValue != null ? (
          <div className="space-y-1">
            <div className="flex items-baseline gap-2">
              <span className="text-2xl font-bold tabular-nums">{formattedValue}</span>
              <DeltaBadge current={value} previous={prevValue} invertColor={invertDelta} />
            </div>
            {/* Sparkline */}
            {sparkData?.length ? (
              <div className="h-8 w-full" data-testid="sparkline">
                <ResponsiveContainer width="100%" height="100%">
                  <AreaChart data={sparkData}>
                    <defs>
                      <linearGradient id={`spark-${title}`} x1="0" y1="0" x2="0" y2="1">
                        <stop offset="0%" stopColor="hsl(var(--primary))" stopOpacity={0.3} />
                        <stop offset="100%" stopColor="hsl(var(--primary))" stopOpacity={0} />
                      </linearGradient>
                    </defs>
                    <Area
                      type="monotone"
                      dataKey={sparkKey || 'count'}
                      stroke="hsl(var(--primary))"
                      strokeWidth={1.5}
                      fill={`url(#spark-${title})`}
                      dot={false}
                      isAnimationActive={false}
                    />
                  </AreaChart>
                </ResponsiveContainer>
              </div>
            ) : null}
          </div>
        ) : (
          <div className="text-sm text-muted-foreground">{emptyText || 'No data'}</div>
        )}
      </CardContent>
    </Card>
  );
}

function DeltaBadge({
  current,
  previous,
  invertColor,
}: {
  current: any;
  previous?: any;
  invertColor?: boolean;
}) {
  if (current == null || previous == null || previous === 0) return null;

  const delta = ((current - previous) / Math.abs(previous)) * 100;
  if (!isFinite(delta)) return null;

  const isPositive = delta > 0;
  const isNeutral = Math.abs(delta) < 0.5;

  if (isNeutral) {
    return (
      <span className="inline-flex items-center gap-0.5 text-xs text-muted-foreground" data-testid="delta-badge">
        <Minus className="h-3 w-3" />
        0%
      </span>
    );
  }

  // For metrics where "down is good" (no-result rate, avg click position), invert colors
  const isGood = invertColor ? !isPositive : isPositive;

  return (
    <span
      className={`inline-flex items-center gap-0.5 text-xs font-medium ${
        isGood ? 'text-green-600' : 'text-red-500'
      }`}
      data-testid="delta-badge"
    >
      {isPositive ? (
        <ArrowUpRight className="h-3 w-3" />
      ) : (
        <ArrowDownRight className="h-3 w-3" />
      )}
      {Math.abs(delta).toFixed(1)}%
    </span>
  );
}

function EmptyState({
  icon: Icon,
  title,
  description,
  positive,
}: {
  icon: any;
  title: string;
  description: string;
  positive?: boolean;
}) {
  return (
    <div className="py-12 text-center" data-testid="empty-state">
      <Icon className={`h-12 w-12 mx-auto mb-4 ${positive ? 'text-green-500/60' : 'text-muted-foreground/30'}`} />
      <h3 className="text-base font-medium mb-1">{title}</h3>
      <p className="text-sm text-muted-foreground max-w-sm mx-auto">{description}</p>
    </div>
  );
}

function ErrorState({ message }: { message: string }) {
  return (
    <div className="py-8 text-center" data-testid="error-state">
      <AlertCircle className="h-8 w-8 mx-auto mb-2 text-red-500/60" />
      <p className="text-sm text-red-600">Error: {message}</p>
    </div>
  );
}

function TableSkeleton({ rows }: { rows: number }) {
  return (
    <div className="space-y-3" data-testid="table-skeleton">
      {Array.from({ length: rows }).map((_, i) => (
        <div key={i} className="flex items-center gap-4">
          <Skeleton className="h-4 w-6" />
          <Skeleton className="h-4 flex-1 max-w-48" />
          <Skeleton className="h-4 w-16 ml-auto" />
        </div>
      ))}
    </div>
  );
}

// ─── Date Formatters ───────────────────────────────────────────

function formatDateShort(d: string) {
  const date = new Date(d + 'T00:00:00');
  return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
}

function formatDateLong(d: string) {
  const date = new Date(d + 'T00:00:00');
  return date.toLocaleDateString('en-US', { weekday: 'short', month: 'long', day: 'numeric', year: 'numeric' });
}
