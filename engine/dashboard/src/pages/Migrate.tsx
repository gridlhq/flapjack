import { useState, useCallback } from 'react';
import { Link } from 'react-router-dom';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import {
  ArrowRightLeft,
  Loader2,
  CheckCircle2,
  XCircle,
  Eye,
  EyeOff,
  AlertTriangle,
} from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import api from '@/lib/api';

interface MigrationResult {
  status: string;
  settings: boolean;
  synonyms: { imported: number };
  rules: { imported: number };
  objects: { imported: number };
  taskID: number;
}

export function Migrate() {
  const queryClient = useQueryClient();

  const [appId, setAppId] = useState('');
  const [apiKey, setApiKey] = useState('');
  const [sourceIndex, setSourceIndex] = useState('');
  const [targetIndex, setTargetIndex] = useState('');
  const [overwrite, setOverwrite] = useState(false);
  const [showKey, setShowKey] = useState(false);

  const migration = useMutation({
    mutationFn: async () => {
      const body: Record<string, unknown> = {
        appId: appId.trim(),
        apiKey: apiKey.trim(),
        sourceIndex: sourceIndex.trim(),
      };
      const target = targetIndex.trim();
      if (target) body.targetIndex = target;
      if (overwrite) body.overwrite = true;

      const response = await api.post<MigrationResult>(
        '/1/migrate-from-algolia',
        body
      );
      return response.data;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['indices'] });
    },
  });

  const handleMigrate = useCallback(() => {
    migration.mutate();
  }, [migration]);

  const canSubmit =
    appId.trim() && apiKey.trim() && sourceIndex.trim() && !migration.isPending;

  const effectiveTarget = targetIndex.trim() || sourceIndex.trim();

  return (
    <div className="space-y-6 max-w-2xl">
      <div>
        <h1 className="text-3xl font-bold">Migrate from Algolia</h1>
        <p className="text-muted-foreground mt-1">
          Import an index from Algolia — settings, documents, synonyms, and rules.
        </p>
      </div>

      {/* Credentials */}
      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Algolia Credentials</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <div className="space-y-2">
              <Label htmlFor="app-id">Application ID</Label>
              <Input
                id="app-id"
                value={appId}
                onChange={(e) => setAppId(e.target.value)}
                placeholder="YourAlgoliaAppId"
                disabled={migration.isPending}
                autoComplete="off"
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="api-key">Admin API Key</Label>
              <div className="relative">
                <Input
                  id="api-key"
                  type={showKey ? 'text' : 'password'}
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                  placeholder="Your admin API key"
                  disabled={migration.isPending}
                  autoComplete="off"
                  className="pr-10"
                />
                <button
                  type="button"
                  onClick={() => setShowKey(!showKey)}
                  className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
                  tabIndex={-1}
                >
                  {showKey ? (
                    <EyeOff className="h-4 w-4" />
                  ) : (
                    <Eye className="h-4 w-4" />
                  )}
                </button>
              </div>
              <p className="text-xs text-muted-foreground">
                Needs read access. Not stored anywhere.
              </p>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Index names */}
      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Index</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <div className="space-y-2">
              <Label htmlFor="source-index">Source Index (Algolia)</Label>
              <Input
                id="source-index"
                value={sourceIndex}
                onChange={(e) => setSourceIndex(e.target.value)}
                placeholder="e.g., products, articles"
                disabled={migration.isPending}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="target-index">
                Target Index (Flapjack)
                <span className="text-muted-foreground font-normal ml-1">— optional</span>
              </Label>
              <Input
                id="target-index"
                value={targetIndex}
                onChange={(e) => setTargetIndex(e.target.value)}
                placeholder={sourceIndex.trim() || 'Same as source'}
                disabled={migration.isPending}
              />
              <p className="text-xs text-muted-foreground">
                Defaults to the source index name if left blank.
              </p>
            </div>
          </div>

          <div className="flex items-center gap-3 pt-2">
            <Switch
              id="overwrite"
              checked={overwrite}
              onCheckedChange={setOverwrite}
              disabled={migration.isPending}
            />
            <div>
              <Label htmlFor="overwrite" className="cursor-pointer">
                Overwrite if exists
              </Label>
              <p className="text-xs text-muted-foreground">
                If the target index already exists, delete it first and re-import.
              </p>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Action */}
      <Button
        size="lg"
        onClick={handleMigrate}
        disabled={!canSubmit}
        className="w-full"
      >
        {migration.isPending ? (
          <>
            <Loader2 className="h-5 w-5 mr-2 animate-spin" />
            Migrating from Algolia...
          </>
        ) : (
          <>
            <ArrowRightLeft className="h-5 w-5 mr-2" />
            Migrate{effectiveTarget ? ` "${effectiveTarget}"` : ''}
          </>
        )}
      </Button>

      {/* Results */}
      {migration.isSuccess && (
        <Card className="border-green-500/50">
          <CardContent className="pt-6">
            <div className="flex items-start gap-3">
              <CheckCircle2 className="h-6 w-6 text-green-500 shrink-0 mt-0.5" />
              <div className="space-y-3 flex-1">
                <div>
                  <h3 className="font-semibold text-lg">Migration complete</h3>
                  <p className="text-sm text-muted-foreground">
                    Index <span className="font-medium">{effectiveTarget}</span> is
                    ready.
                  </p>
                </div>

                <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
                  <ResultStat
                    label="Documents"
                    value={migration.data.objects.imported}
                  />
                  <ResultStat
                    label="Settings"
                    value={migration.data.settings ? 'Applied' : 'None'}
                  />
                  <ResultStat
                    label="Synonyms"
                    value={migration.data.synonyms.imported}
                  />
                  <ResultStat
                    label="Rules"
                    value={migration.data.rules.imported}
                  />
                </div>

                <div className="flex gap-2 pt-1">
                  <Link to={`/index/${encodeURIComponent(effectiveTarget)}`}>
                    <Button size="sm">Browse Index</Button>
                  </Link>
                  <Link to={`/index/${encodeURIComponent(effectiveTarget)}/settings`}>
                    <Button variant="outline" size="sm">
                      View Settings
                    </Button>
                  </Link>
                </div>
              </div>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Error */}
      {migration.isError && (
        <Card className="border-destructive/50">
          <CardContent className="pt-6">
            <div className="flex items-start gap-3">
              <XCircle className="h-6 w-6 text-destructive shrink-0 mt-0.5" />
              <div className="space-y-1">
                <h3 className="font-semibold">Migration failed</h3>
                <p className="text-sm text-muted-foreground">
                  {getErrorMessage(migration.error)}
                </p>
              </div>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Info */}
      <Card className="bg-muted/30">
        <CardContent className="pt-6">
          <div className="flex items-start gap-3">
            <AlertTriangle className="h-5 w-5 text-muted-foreground shrink-0 mt-0.5" />
            <div className="space-y-2 text-sm text-muted-foreground">
              <p>
                <span className="font-medium text-foreground">What gets migrated:</span>{' '}
                Settings (searchable attributes, facets, ranking), all documents,
                synonyms, and query rules.
              </p>
              <p>
                <span className="font-medium text-foreground">Credentials:</span>{' '}
                Your Algolia API key is sent directly to the Flapjack server to fetch
                data from Algolia's API. It is not stored or logged.
              </p>
              <p>
                <span className="font-medium text-foreground">Large indexes:</span>{' '}
                Documents are fetched in batches. Migration may take a few minutes for
                indexes with millions of records.
              </p>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

function ResultStat({
  label,
  value,
}: {
  label: string;
  value: number | string;
}) {
  return (
    <div className="rounded-md border p-3 text-center">
      <div className="text-xl font-bold">
        {typeof value === 'number' ? value.toLocaleString() : value}
      </div>
      <div className="text-xs text-muted-foreground">{label}</div>
    </div>
  );
}

function getErrorMessage(error: unknown): string {
  if (!error) return 'Unknown error';

  // Axios error with response data
  if (
    typeof error === 'object' &&
    'response' in error &&
    (error as Record<string, unknown>).response
  ) {
    const resp = (error as { response: { data?: { message?: string }; status?: number } })
      .response;
    if (resp.data?.message) return resp.data.message;
    if (resp.status === 409)
      return 'Target index already exists. Enable "Overwrite if exists" to replace it.';
    if (resp.status === 502) return 'Could not connect to Algolia. Check your App ID and API Key.';
    return `Server returned ${resp.status}`;
  }

  if (error instanceof Error) return error.message;
  return String(error);
}
