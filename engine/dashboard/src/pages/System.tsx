import { memo, useRef } from 'react';
import { Link } from 'react-router-dom';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Skeleton } from '@/components/ui/skeleton';
import { Button } from '@/components/ui/button';
import { useHealthDetail, useInternalStatus } from '@/hooks/useSystemStatus';
import { useIndices } from '@/hooks/useIndices';
import {
  useExportIndex,
  useImportIndex,
  useSnapshotToS3,
  useRestoreFromS3,
  useListSnapshots,
} from '@/hooks/useSnapshots';
import {
  Activity,
  Server,
  Database,
  RefreshCw,
  CheckCircle,
  XCircle,
  Layers,
  Download,
  Upload,
  HardDrive,
  Cloud,
  CloudOff,
} from 'lucide-react';
import { formatBytes } from '@/lib/utils';
import { InfoTooltip } from '@/components/ui/info-tooltip';

function IndexHealthSummary() {
  const { data: indices, isLoading } = useIndices();

  if (isLoading || !indices || indices.length === 0) return null;

  const healthyCount = indices.filter((idx) => (idx.numberOfPendingTasks ?? 0) === 0).length;
  const totalPending = indices.reduce((sum, idx) => sum + (idx.numberOfPendingTasks ?? 0), 0);

  return (
    <Card data-testid="index-health-summary">
      <CardHeader className="pb-2">
        <CardTitle className="text-base flex items-center gap-1.5">
          Index Health
          <InfoTooltip content="Shows the health status of each index. Green means healthy with no pending operations. Amber means tasks are still processing." />
        </CardTitle>
      </CardHeader>
      <CardContent>
        <div className="flex flex-wrap items-center gap-3 mb-2">
          {indices.map((idx) => {
            const pending = idx.numberOfPendingTasks ?? 0;
            const isHealthy = pending === 0;
            return (
              <Link
                key={idx.uid}
                to={`/index/${encodeURIComponent(idx.uid)}`}
                className="flex items-center gap-1.5 hover:bg-accent/50 rounded-md px-1.5 py-0.5 transition-colors"
                data-testid={`index-dot-${idx.uid}`}
              >
                <span
                  className={`inline-block h-2.5 w-2.5 rounded-full ${
                    isHealthy
                      ? 'bg-green-500'
                      : 'bg-amber-500 animate-pulse'
                  }`}
                />
                <span className="text-sm">{idx.uid}</span>
              </Link>
            );
          })}
        </div>
        <p className="text-sm text-muted-foreground">
          {healthyCount} of {indices.length} indices healthy{totalPending > 0 ? ` · ${totalPending} pending task(s)` : ''}
        </p>
      </CardContent>
    </Card>
  );
}

function HealthTab() {
  const { data, isLoading, isError, error } = useHealthDetail();

  if (isLoading) {
    return (
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {Array.from({ length: 5 }).map((_, i) => (
          <Card key={i}><CardContent className="pt-6"><Skeleton className="h-16" /></CardContent></Card>
        ))}
      </div>
    );
  }

  if (isError) {
    return (
      <Card>
        <CardContent className="pt-6">
          <div className="flex items-center gap-3 text-destructive">
            <XCircle className="h-5 w-5" />
            <div>
              <p className="font-medium">Failed to fetch health status</p>
              <p className="text-sm text-muted-foreground">{(error as Error)?.message}</p>
            </div>
          </div>
        </CardContent>
      </Card>
    );
  }

  const stats = [
    {
      label: 'Status',
      value: data?.status || 'unknown',
      icon: data?.status === 'ok' ? CheckCircle : XCircle,
      color: data?.status === 'ok' ? 'text-green-600 dark:text-green-400' : 'text-destructive',
    },
    {
      label: 'Active Writers',
      value: `${data?.active_writers ?? 0} / ${data?.max_concurrent_writers ?? 0}`,
      icon: Database,
      color: 'text-blue-600 dark:text-blue-400',
    },
    {
      label: 'Facet Cache',
      value: `${data?.facet_cache_entries ?? 0} / ${data?.facet_cache_cap ?? 0}`,
      icon: Layers,
      color: 'text-purple-600 dark:text-purple-400',
    },
  ];

  return (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground">Auto-refreshes every 5 seconds</p>
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {stats.map((stat) => (
          <Card key={stat.label} data-testid={`health-${stat.label.toLowerCase().replace(/\s+/g, '-')}`}>
            <CardHeader className="flex flex-row items-center justify-between pb-2">
              <CardTitle className="text-sm font-medium text-muted-foreground">
                {stat.label}
              </CardTitle>
              <stat.icon className={`h-5 w-5 ${stat.color}`} />
            </CardHeader>
            <CardContent>
              <p className="text-2xl font-bold">{stat.value}</p>
            </CardContent>
          </Card>
        ))}
      </div>
      <IndexHealthSummary />
    </div>
  );
}

function IndicesTab() {
  const { data: indices, isLoading, isError } = useIndices();

  if (isLoading) {
    return (
      <div className="space-y-2">
        {Array.from({ length: 5 }).map((_, i) => (
          <Skeleton key={i} className="h-12 w-full" />
        ))}
      </div>
    );
  }

  if (isError || !indices) {
    return (
      <Card>
        <CardContent className="pt-6 text-center text-muted-foreground">
          Unable to load indices.
        </CardContent>
      </Card>
    );
  }

  const totalDocs = indices.reduce((sum, idx) => sum + (idx.entries ?? 0), 0);
  const totalSize = indices.reduce((sum, idx) => sum + (idx.dataSize ?? 0), 0);
  const pendingTasks = indices.reduce((sum, idx) => sum + (idx.numberOfPendingTasks ?? 0), 0);

  return (
    <div className="space-y-4">
      <div className="grid gap-4 sm:grid-cols-3">
        <Card data-testid="indices-total-count">
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">Total Indices</CardTitle>
          </CardHeader>
          <CardContent><p className="text-2xl font-bold">{indices.length}</p></CardContent>
        </Card>
        <Card data-testid="indices-total-docs">
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">Total Documents</CardTitle>
          </CardHeader>
          <CardContent><p className="text-2xl font-bold">{totalDocs.toLocaleString()}</p></CardContent>
        </Card>
        <Card data-testid="indices-total-storage">
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">Total Storage</CardTitle>
          </CardHeader>
          <CardContent><p className="text-2xl font-bold">{formatBytes(totalSize)}</p></CardContent>
        </Card>
      </div>

      {pendingTasks > 0 && (
        <Card>
          <CardContent className="pt-6">
            <div className="flex items-center gap-2 text-amber-600 dark:text-amber-400">
              <RefreshCw className="h-4 w-4 animate-spin" />
              <span className="text-sm font-medium">{pendingTasks} pending task(s) across indices</span>
            </div>
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader>
          <CardTitle className="text-base flex items-center gap-1.5">
            Index Details
            <InfoTooltip content="Each index is an isolated search collection with its own data, settings, and access controls." />
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b text-left text-muted-foreground">
                  <th className="pb-2 pr-4 font-medium">Name</th>
                  <th className="pb-2 pr-4 font-medium">Status</th>
                  <th className="pb-2 pr-4 font-medium text-right">Documents</th>
                  <th className="pb-2 pr-4 font-medium text-right">Size</th>
                  <th className="pb-2 font-medium text-right">Pending</th>
                </tr>
              </thead>
              <tbody>
                {indices.map((idx) => {
                  const pending = idx.numberOfPendingTasks ?? 0;
                  return (
                    <tr key={idx.uid} className="border-b last:border-0">
                      <td className="py-2 pr-4 font-medium">
                        <Link
                          to={`/index/${idx.uid}`}
                          className="text-primary hover:underline"
                          data-testid={`index-link-${idx.uid}`}
                        >
                          {idx.uid}
                        </Link>
                      </td>
                      <td className="py-2 pr-4" data-testid={`index-status-${idx.uid}`}>
                        {pending === 0 ? (
                          <span className="inline-flex items-center gap-1 text-green-600 dark:text-green-400">
                            <CheckCircle className="h-4 w-4" />
                            Healthy
                          </span>
                        ) : (
                          <span className="inline-flex items-center gap-1 text-amber-600 dark:text-amber-400">
                            <RefreshCw className="h-4 w-4 animate-spin" />
                            Processing ({pending})
                          </span>
                        )}
                      </td>
                      <td className="py-2 pr-4 text-right">{(idx.entries ?? 0).toLocaleString()}</td>
                      <td className="py-2 pr-4 text-right">{formatBytes(idx.dataSize ?? 0)}</td>
                      <td className="py-2 text-right">
                        {pending > 0 ? (
                          <span className="text-amber-600 dark:text-amber-400">
                            {pending}
                          </span>
                        ) : (
                          <span className="text-muted-foreground">0</span>
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

function ReplicationTab() {
  const { data, isLoading, isError, error } = useInternalStatus();

  if (isLoading) {
    return (
      <div className="space-y-4">
        <Skeleton className="h-24" />
        <Skeleton className="h-24" />
      </div>
    );
  }

  if (isError) {
    return (
      <Card>
        <CardContent className="pt-6">
          <div className="flex items-center gap-3 text-muted-foreground">
            <Server className="h-5 w-5" />
            <div>
              <p className="font-medium">Replication status unavailable</p>
              <p className="text-sm">{(error as Error)?.message || 'Could not reach internal status endpoint.'}</p>
            </div>
          </div>
        </CardContent>
      </Card>
    );
  }

  return (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground">Auto-refreshes every 10 seconds</p>
      <div className="grid gap-4 sm:grid-cols-2">
        <Card>
          <CardHeader className="flex flex-row items-center justify-between pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">Node ID</CardTitle>
            <Server className="h-5 w-5 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <p className="text-sm font-mono break-all">{data?.node_id || 'N/A'}</p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="flex flex-row items-center justify-between pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">Replication</CardTitle>
            {data?.replication_enabled ? (
              <CheckCircle className="h-5 w-5 text-green-600 dark:text-green-400" />
            ) : (
              <XCircle className="h-5 w-5 text-muted-foreground" />
            )}
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold">
              {data?.replication_enabled ? 'Enabled' : 'Disabled'}
            </p>
            {data?.replication_enabled && (
              <p className="text-sm text-muted-foreground mt-1">
                {data.peer_count} peer(s) connected
              </p>
            )}
          </CardContent>
        </Card>
      </div>

      {data?.ssl_renewal && (
        <Card>
          <CardHeader>
            <CardTitle className="text-base">SSL / TLS</CardTitle>
          </CardHeader>
          <CardContent className="space-y-2 text-sm">
            {data.ssl_renewal.certificate_expiry && (
              <p><span className="text-muted-foreground">Certificate expires:</span> {data.ssl_renewal.certificate_expiry}</p>
            )}
            {data.ssl_renewal.next_renewal && (
              <p><span className="text-muted-foreground">Next renewal:</span> {data.ssl_renewal.next_renewal}</p>
            )}
          </CardContent>
        </Card>
      )}
    </div>
  );
}

function SnapshotsTab() {
  const { data: indices, isLoading: indicesLoading } = useIndices();
  const exportIndex = useExportIndex();
  const importIndex = useImportIndex();
  const snapshotToS3 = useSnapshotToS3();
  const restoreFromS3 = useRestoreFromS3();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const importTargetRef = useRef<string>('');

  // Probe S3 availability by listing snapshots for the first index
  const firstIndex = indices?.[0]?.uid || '';
  const { data: snapshots, isError: s3Error } = useListSnapshots(firstIndex);
  const s3Available = !s3Error && !!firstIndex;

  const handleImport = (indexName: string) => {
    importTargetRef.current = indexName;
    fileInputRef.current?.click();
  };

  const onFileSelected = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file && importTargetRef.current) {
      importIndex.mutate({ indexName: importTargetRef.current, file });
    }
    e.target.value = '';
  };

  const handleExportAll = () => {
    indices?.forEach((idx) => exportIndex.mutate(idx.uid));
  };

  const handleBackupAll = () => {
    indices?.forEach((idx) => snapshotToS3.mutate(idx.uid));
  };

  if (indicesLoading) {
    return (
      <div className="space-y-4">
        <Skeleton className="h-24" />
        <Skeleton className="h-24" />
      </div>
    );
  }

  if (!indices || indices.length === 0) {
    return (
      <Card>
        <CardContent className="pt-6 text-center text-muted-foreground">
          No indices available. Create an index first to use snapshots.
        </CardContent>
      </Card>
    );
  }

  return (
    <div className="space-y-6" data-testid="snapshots-tab">
      <input
        ref={fileInputRef}
        type="file"
        accept=".tar.gz,.tgz"
        className="hidden"
        onChange={onFileSelected}
        data-testid="snapshot-file-input"
      />

      {/* Local Export / Import */}
      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <CardTitle className="text-base flex items-center gap-2">
              <HardDrive className="h-4 w-4" />
              Local Export / Import
              <InfoTooltip content="Export an index as a tar.gz file to your local machine, or import a tar.gz file into an existing index." />
            </CardTitle>
            <Button
              variant="outline"
              size="sm"
              onClick={handleExportAll}
              disabled={exportIndex.isPending}
              data-testid="export-all-btn"
            >
              <Download className="h-4 w-4 mr-1" />
              Export All
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          <div className="space-y-2">
            {indices.map((idx) => (
              <div
                key={idx.uid}
                className="flex items-center justify-between p-3 rounded-md border border-border"
                data-testid={`snapshot-index-${idx.uid}`}
              >
                <div>
                  <span className="font-medium text-sm">{idx.uid}</span>
                  <span className="text-xs text-muted-foreground ml-2">
                    {(idx.entries ?? 0).toLocaleString()} docs · {formatBytes(idx.dataSize ?? 0)}
                  </span>
                </div>
                <div className="flex gap-2">
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => exportIndex.mutate(idx.uid)}
                    disabled={exportIndex.isPending}
                    data-testid={`export-btn-${idx.uid}`}
                  >
                    <Download className="h-3 w-3 mr-1" />
                    Export
                  </Button>
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => handleImport(idx.uid)}
                    disabled={importIndex.isPending}
                    data-testid={`import-btn-${idx.uid}`}
                  >
                    <Upload className="h-3 w-3 mr-1" />
                    Import
                  </Button>
                </div>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>

      {/* S3 Backups */}
      <Card data-testid="s3-section">
        <CardHeader>
          <div className="flex items-center justify-between">
            <CardTitle className="text-base flex items-center gap-2">
              {s3Available ? (
                <Cloud className="h-4 w-4 text-blue-500" />
              ) : (
                <CloudOff className="h-4 w-4 text-muted-foreground" />
              )}
              S3 Backups
              <InfoTooltip content="Back up indices to S3-compatible storage. Requires FLAPJACK_S3_BUCKET and FLAPJACK_S3_REGION environment variables." />
            </CardTitle>
            {s3Available && (
              <Button
                variant="outline"
                size="sm"
                onClick={handleBackupAll}
                disabled={snapshotToS3.isPending}
                data-testid="backup-all-s3-btn"
              >
                <Cloud className="h-4 w-4 mr-1" />
                Backup All to S3
              </Button>
            )}
          </div>
        </CardHeader>
        <CardContent>
          {!s3Available ? (
            <div className="text-sm text-muted-foreground space-y-2" data-testid="s3-not-configured">
              <p>S3 backups are not configured. To enable, set these environment variables:</p>
              <code className="block bg-muted px-3 py-2 rounded text-xs">
                FLAPJACK_S3_BUCKET=your-bucket-name<br />
                FLAPJACK_S3_REGION=us-east-1<br />
                FLAPJACK_S3_ENDPOINT=https://s3.amazonaws.com  (optional)
              </code>
            </div>
          ) : (
            <div className="space-y-3">
              {snapshots && snapshots.length > 0 && (
                <div className="text-sm text-muted-foreground mb-2">
                  {snapshots.length} snapshot(s) available for {firstIndex}
                </div>
              )}
              {indices.map((idx) => (
                <div
                  key={idx.uid}
                  className="flex items-center justify-between p-3 rounded-md border border-border"
                  data-testid={`s3-index-${idx.uid}`}
                >
                  <span className="font-medium text-sm">{idx.uid}</span>
                  <div className="flex gap-2">
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => snapshotToS3.mutate(idx.uid)}
                      disabled={snapshotToS3.isPending}
                      data-testid={`backup-btn-${idx.uid}`}
                    >
                      <Cloud className="h-3 w-3 mr-1" />
                      Backup
                    </Button>
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => restoreFromS3.mutate(idx.uid)}
                      disabled={restoreFromS3.isPending}
                      data-testid={`restore-btn-${idx.uid}`}
                    >
                      <RefreshCw className="h-3 w-3 mr-1" />
                      Restore
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

export const System = memo(function System() {
  return (
    <div className="space-y-6">
      <div className="flex items-center gap-3">
        <Activity className="h-6 w-6" />
        <h1 className="text-2xl font-bold">System</h1>
      </div>

      <Tabs defaultValue="health">
        <TabsList>
          <TabsTrigger value="health">Health</TabsTrigger>
          <TabsTrigger value="indices">Indices</TabsTrigger>
          <TabsTrigger value="replication">Replication</TabsTrigger>
          <TabsTrigger value="snapshots">Snapshots</TabsTrigger>
        </TabsList>

        <TabsContent value="health">
          <HealthTab />
        </TabsContent>

        <TabsContent value="indices">
          <IndicesTab />
        </TabsContent>

        <TabsContent value="replication">
          <ReplicationTab />
        </TabsContent>

        <TabsContent value="snapshots">
          <SnapshotsTab />
        </TabsContent>
      </Tabs>
    </div>
  );
});
