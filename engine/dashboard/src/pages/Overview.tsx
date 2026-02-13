import { useState, useMemo, useCallback, useRef } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { useIndices, useDeleteIndex } from '@/hooks/useIndices';
import { useHealth } from '@/hooks/useHealth';
import { useAnalyticsOverview, defaultRange, type DateRange } from '@/hooks/useAnalytics';
import { useExportIndex, useImportIndex } from '@/hooks/useSnapshots';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Plus, ChevronLeft, ChevronRight, Search, Users, AlertCircle, BarChart3, Trash2, Database, Download, Upload } from 'lucide-react';
import { Skeleton } from '@/components/ui/skeleton';
import { CreateIndexDialog } from '@/components/indices/CreateIndexDialog';
import { ConfirmDialog } from '@/components/ui/confirm-dialog';
import { InfoTooltip } from '@/components/ui/info-tooltip';
import { formatBytes, formatDate } from '@/lib/utils';
import { AreaChart, Area, ResponsiveContainer, XAxis, YAxis, CartesianGrid, Tooltip } from 'recharts';

const ITEMS_PER_PAGE = 10;

export function Overview() {
  const { data: indices, isLoading, error } = useIndices();
  const { data: health, isLoading: healthLoading, error: healthError } = useHealth();
  const [currentPage, setCurrentPage] = useState(1);
  const [showCreateDialog, setShowCreateDialog] = useState(false);
  const deleteMutation = useDeleteIndex();
  const exportIndex = useExportIndex();
  const importIndex = useImportIndex();
  const [pendingDeleteIndex, setPendingDeleteIndex] = useState<string | null>(null);
  const navigate = useNavigate();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const importTargetRef = useRef<string>('');

  const confirmDelete = useCallback(() => {
    if (pendingDeleteIndex) {
      deleteMutation.mutate(pendingDeleteIndex, {
        onSettled: () => setPendingDeleteIndex(null),
      });
    }
  }, [pendingDeleteIndex, deleteMutation]);

  const handleImport = useCallback((indexName: string) => {
    importTargetRef.current = indexName;
    fileInputRef.current?.click();
  }, []);

  const onFileSelected = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file && importTargetRef.current) {
      importIndex.mutate({ indexName: importTargetRef.current, file });
    }
    e.target.value = '';
  }, [importIndex]);

  const handleExportAll = useCallback(() => {
    indices?.forEach((idx) => exportIndex.mutate(idx.uid));
  }, [indices, exportIndex]);

  const analyticsRange: DateRange = useMemo(() => defaultRange(7), []);
  const { data: overview, isLoading: overviewLoading } = useAnalyticsOverview(analyticsRange);

  const totalDocs = indices?.reduce((sum, idx) => sum + (idx.entries || 0), 0) || 0;
  const totalSize = indices?.reduce((sum, idx) => sum + (idx.dataSize || 0), 0) || 0;

  // Pagination
  const totalPages = Math.ceil((indices?.length || 0) / ITEMS_PER_PAGE);
  const paginatedIndices = useMemo(() => {
    if (!indices) return [];
    const start = (currentPage - 1) * ITEMS_PER_PAGE;
    return indices.slice(start, start + ITEMS_PER_PAGE);
  }, [indices, currentPage]);

  return (
    <div className="space-y-6">
      <input
        ref={fileInputRef}
        type="file"
        accept=".tar.gz,.tgz"
        className="hidden"
        onChange={onFileSelected}
        data-testid="overview-file-input"
      />
      <div className="flex items-center justify-between">
        <h1 className="text-3xl font-bold">Overview</h1>
        <div className="flex gap-2">
          {indices && indices.length > 0 && (
            <Button
              variant="outline"
              onClick={handleExportAll}
              disabled={exportIndex.isPending}
              data-testid="overview-export-all-btn"
            >
              <Download className="mr-2 h-4 w-4" /> Export All
            </Button>
          )}
          <Button onClick={() => setShowCreateDialog(true)}>
            <Plus className="mr-2 h-4 w-4" /> Create Index
          </Button>
        </div>
      </div>

      {/* Stats Cards */}
      <div className="grid gap-4 grid-cols-2 md:grid-cols-4">
        <Card data-testid="stat-card-indices">
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground flex items-center gap-1.5">
              Indices
              <InfoTooltip content="Each index is an isolated data container with its own documents, settings, and search configuration." />
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{indices?.length || 0}</div>
          </CardContent>
        </Card>
        <Card data-testid="stat-card-documents">
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">
              Documents
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{totalDocs.toLocaleString()}</div>
          </CardContent>
        </Card>
        <Card data-testid="stat-card-storage">
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">
              Storage
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{formatBytes(totalSize)}</div>
          </CardContent>
        </Card>
        <Card data-testid="stat-card-status">
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground flex items-center gap-1.5">
              Status
              <InfoTooltip content="Overall health of your Flapjack server. 'Healthy' means all systems are operational with no errors." />
            </CardTitle>
          </CardHeader>
          <CardContent>
            {healthLoading ? (
              <Skeleton className="h-8 w-28" />
            ) : healthError ? (
              <div className="text-2xl font-bold text-red-600">Disconnected</div>
            ) : health?.status === 'ok' ? (
              <div className="text-2xl font-bold text-green-600">Healthy</div>
            ) : (
              <div className="text-2xl font-bold text-yellow-600">Unknown</div>
            )}
          </CardContent>
        </Card>
      </div>

      {/* Index Health Cards — shown when multiple indices exist */}
      {indices && indices.length > 1 && (
        <Card data-testid="index-health-section">
          <CardHeader className="pb-2">
            <CardTitle className="text-base font-medium flex items-center gap-2">
              <Database className="h-4 w-4" />
              Index Health
              <InfoTooltip content="A quick summary of each index's data and health status." />
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
              {indices.map((index) => {
                const pending = index.numberOfPendingTasks ?? 0;
                const isHealthy = pending === 0;
                return (
                  <div
                    key={index.uid}
                    className="flex items-center gap-3 p-3 rounded-md border border-border hover:bg-accent/50 transition-colors cursor-pointer"
                    onClick={() => navigate(`/index/${encodeURIComponent(index.uid)}`)}
                    data-testid={`index-card-${index.uid}`}
                  >
                    <span
                      className={`inline-block h-2.5 w-2.5 rounded-full shrink-0 ${
                        isHealthy ? 'bg-green-500' : 'bg-amber-500 animate-pulse'
                      }`}
                      data-testid={`index-status-${index.uid}`}
                    />
                    <div className="min-w-0 flex-1">
                      <div className="font-medium text-sm truncate">{index.uid}</div>
                      <div className="text-xs text-muted-foreground">
                        {(index.entries || 0).toLocaleString()} docs · {formatBytes(index.dataSize || 0)}
                        {pending > 0 && (
                          <span className="text-amber-600 dark:text-amber-400"> · {pending} pending</span>
                        )}
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          </CardContent>
        </Card>
      )}

      {/* Server-Wide Analytics */}
      {(overviewLoading || overview?.totalSearches > 0) && (
        <Card data-testid="overview-analytics">
          <CardHeader className="pb-2">
            <div className="flex items-center justify-between">
              <CardTitle className="text-base font-medium flex items-center gap-2">
                <BarChart3 className="h-4 w-4" />
                Search Analytics (Last 7 Days)
              </CardTitle>
              {indices?.[0] && (
                <Link to={`/index/${encodeURIComponent(indices[0].uid)}/analytics`} className="text-xs text-primary hover:underline">View Details</Link>
              )}
            </div>
          </CardHeader>
          <CardContent>
            {overviewLoading ? (
              <div className="grid gap-4 grid-cols-2 md:grid-cols-4">
                {[1,2,3,4].map(i => <Skeleton key={i} className="h-16" />)}
              </div>
            ) : (
              <div className="space-y-4">
                <div className="grid gap-4 grid-cols-3">
                  <OverviewKpi icon={Search} label="Total Searches" value={overview?.totalSearches?.toLocaleString() || '0'} />
                  <OverviewKpi icon={Users} label="Unique Users" value={overview?.uniqueUsers?.toLocaleString() || '0'} />
                  <OverviewKpi
                    icon={AlertCircle}
                    label="No-Result Rate"
                    value={overview?.noResultRate != null ? `${(overview.noResultRate * 100).toFixed(1)}%` : '-'}
                    warn={overview?.noResultRate > 0.1}
                  />
                </div>
                {overview?.dates?.length > 0 && (
                  <div className="h-32">
                    <ResponsiveContainer width="100%" height="100%">
                      <AreaChart data={overview.dates}>
                        <defs>
                          <linearGradient id="overviewGradient" x1="0" y1="0" x2="0" y2="1">
                            <stop offset="0%" stopColor="hsl(var(--primary))" stopOpacity={0.2} />
                            <stop offset="100%" stopColor="hsl(var(--primary))" stopOpacity={0} />
                          </linearGradient>
                        </defs>
                        <CartesianGrid strokeDasharray="3 3" className="stroke-border" vertical={false} />
                        <XAxis
                          dataKey="date"
                          className="text-xs"
                          tickFormatter={(d: string) => {
                            const date = new Date(d + 'T00:00:00');
                            return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
                          }}
                          tick={{ fill: 'hsl(var(--muted-foreground))' }}
                        />
                        <YAxis className="text-xs" tick={{ fill: 'hsl(var(--muted-foreground))' }} width={40} />
                        <Tooltip
                          contentStyle={{
                            background: 'hsl(var(--card))',
                            border: '1px solid hsl(var(--border))',
                            borderRadius: '8px',
                            fontSize: '13px',
                          }}
                          formatter={(v: any) => [Number(v).toLocaleString(), 'Searches']}
                        />
                        <Area type="monotone" dataKey="count" stroke="hsl(var(--primary))" strokeWidth={2} fill="url(#overviewGradient)" />
                      </AreaChart>
                    </ResponsiveContainer>
                  </div>
                )}
                {overview?.indices?.length > 1 && (
                  <div className="text-xs text-muted-foreground">
                    Across {overview.indices.length} indices: {overview.indices.slice(0, 5).map((idx: any) => `${idx.index} (${idx.searches})`).join(', ')}
                    {overview.indices.length > 5 && ` and ${overview.indices.length - 5} more`}
                  </div>
                )}
              </div>
            )}
          </CardContent>
        </Card>
      )}

      {/* Index List */}
      <Card>
        <CardHeader>
          <CardTitle>Indices</CardTitle>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <div className="space-y-2 py-2">
              {[1, 2, 3].map((i) => (
                <div key={i} className="flex items-center justify-between p-4 rounded-md border border-border">
                  <div className="space-y-2 flex-1">
                    <Skeleton className="h-5 w-40" />
                    <Skeleton className="h-4 w-64" />
                  </div>
                  <div className="flex gap-2">
                    <Skeleton className="h-8 w-20 rounded-md" />
                    <Skeleton className="h-8 w-20 rounded-md" />
                  </div>
                </div>
              ))}
            </div>
          ) : error ? (
            <div className="text-center py-8 text-red-600">
              Error loading indices: {error instanceof Error ? error.message : 'Unknown error'}
            </div>
          ) : indices && indices.length > 0 ? (
            <div className="space-y-4">
              <div className="space-y-2">
                {paginatedIndices.map((index) => (
                  <div
                    key={index.uid}
                    className="flex items-center justify-between p-4 rounded-md border border-border hover:bg-accent/50 transition-colors cursor-pointer"
                    onClick={() => navigate(`/index/${encodeURIComponent(index.uid)}`)}
                  >
                    <div>
                      <h3 className="font-medium">{index.uid}</h3>
                      <p className="text-sm text-muted-foreground">
                        {index.entries?.toLocaleString() || 0} documents · {formatBytes(index.dataSize || 0)}
                        {index.updatedAt && ` · Updated ${formatDate(index.updatedAt)}`}
                      </p>
                    </div>
                    <div className="flex gap-2 items-center" onClick={(e) => e.stopPropagation()}>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => exportIndex.mutate(index.uid)}
                        disabled={exportIndex.isPending}
                        title={`Export index "${index.uid}"`}
                        data-testid={`overview-export-${index.uid}`}
                      >
                        <Download className="h-4 w-4" />
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => handleImport(index.uid)}
                        disabled={importIndex.isPending}
                        title={`Import into index "${index.uid}"`}
                        data-testid={`overview-import-${index.uid}`}
                      >
                        <Upload className="h-4 w-4" />
                      </Button>
                      <Link to={`/index/${encodeURIComponent(index.uid)}/settings`}>
                        <Button variant="outline" size="sm">
                          Settings
                        </Button>
                      </Link>
                      <Button
                        variant="ghost"
                        size="sm"
                        className="text-muted-foreground hover:text-destructive"
                        onClick={() => setPendingDeleteIndex(index.uid)}
                        disabled={deleteMutation.isPending}
                        title={`Delete index "${index.uid}"`}
                      >
                        <Trash2 className="h-4 w-4" />
                      </Button>
                    </div>
                  </div>
                ))}
              </div>

              {/* Pagination */}
              {totalPages > 1 && (
                <div className="flex items-center justify-between pt-4 border-t">
                  <div className="text-sm text-muted-foreground">
                    Showing {((currentPage - 1) * ITEMS_PER_PAGE) + 1}-{Math.min(currentPage * ITEMS_PER_PAGE, indices.length)} of {indices.length} indices
                  </div>
                  <div className="flex gap-2">
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => setCurrentPage(p => Math.max(1, p - 1))}
                      disabled={currentPage === 1}
                    >
                      <ChevronLeft className="h-4 w-4" />
                      Previous
                    </Button>
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => setCurrentPage(p => Math.min(totalPages, p + 1))}
                      disabled={currentPage === totalPages}
                    >
                      Next
                      <ChevronRight className="h-4 w-4" />
                    </Button>
                  </div>
                </div>
              )}
            </div>
          ) : (
            <div className="text-center py-8 text-muted-foreground">
              No indices yet. Create your first index to get started.
            </div>
          )}
        </CardContent>
      </Card>

      <CreateIndexDialog
        open={showCreateDialog}
        onOpenChange={setShowCreateDialog}
      />

      <ConfirmDialog
        open={pendingDeleteIndex !== null}
        onOpenChange={(open) => { if (!open) setPendingDeleteIndex(null); }}
        title="Delete Index"
        description={
          <>
            Are you sure you want to delete{' '}
            <code className="font-mono text-sm bg-muted px-1 py-0.5 rounded">
              {pendingDeleteIndex}
            </code>
            ? This action cannot be undone.
          </>
        }
        confirmLabel="Delete"
        variant="destructive"
        onConfirm={confirmDelete}
        isPending={deleteMutation.isPending}
      />
    </div>
  );
}

function OverviewKpi({ icon: Icon, label, value, warn }: { icon: any; label: string; value: string; warn?: boolean }) {
  return (
    <div className="flex items-center gap-3 p-3 rounded-md bg-muted/30">
      <Icon className={`h-5 w-5 shrink-0 ${warn ? 'text-amber-500' : 'text-muted-foreground'}`} />
      <div>
        <div className={`text-lg font-bold tabular-nums ${warn ? 'text-amber-500' : ''}`}>{value}</div>
        <div className="text-xs text-muted-foreground">{label}</div>
      </div>
    </div>
  );
}
