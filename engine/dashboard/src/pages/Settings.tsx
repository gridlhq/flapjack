import { useState, useCallback, lazy, Suspense } from 'react';
import { Link, useParams } from 'react-router-dom';
import { Save, RotateCcw, Code, ChevronLeft, HardDriveDownload } from 'lucide-react';
import { useSettings, useUpdateSettings } from '@/hooks/useSettings';
import { useCompactIndex } from '@/hooks/useIndices';
import { Card } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Skeleton } from '@/components/ui/skeleton';
import { SettingsForm } from '@/components/settings/SettingsForm';
import type { IndexSettings } from '@/lib/types';

const Editor = lazy(() =>
  import('@monaco-editor/react').then((module) => ({
    default: module.default,
  }))
);

export function Settings() {
  const { indexName } = useParams<{ indexName: string }>();
  const [isDirty, setIsDirty] = useState(false);
  const [formData, setFormData] = useState<Partial<IndexSettings>>({});
  const [showJson, setShowJson] = useState(false);

  const { data: settings, isLoading: isLoadingSettings } = useSettings(indexName || '');
  const updateSettings = useUpdateSettings(indexName || '');
  const compactIndex = useCompactIndex();

  const handleFormChange = useCallback(
    (updates: Partial<IndexSettings>) => {
      setFormData((prev) => ({ ...prev, ...updates }));
      setIsDirty(true);
    },
    []
  );

  const handleSave = useCallback(async () => {
    try {
      await updateSettings.mutateAsync(formData);
      setIsDirty(false);
      setFormData({});
    } catch (err) {
      console.error('Failed to save settings:', err);
    }
  }, [formData, updateSettings]);

  const handleReset = useCallback(() => {
    setFormData({});
    setIsDirty(false);
  }, []);

  if (!indexName) {
    return (
      <Card className="p-8 text-center">
        <h3 className="text-lg font-semibold mb-2">No index selected</h3>
        <p className="text-muted-foreground mb-4">
          Select an index from the Overview page to configure its settings
        </p>
        <Link to="/overview">
          <Button>Go to Overview</Button>
        </Link>
      </Card>
    );
  }

  const currentSettings = { ...settings, ...formData };

  return (
    <div className="h-full flex flex-col">
      {/* Breadcrumb header */}
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2">
          <Link to={`/index/${encodeURIComponent(indexName)}`}>
            <Button variant="ghost" size="sm">
              <ChevronLeft className="h-4 w-4 mr-1" />
              {indexName}
            </Button>
          </Link>
          <span className="text-muted-foreground">/</span>
          <h2 className="text-xl font-semibold">Settings</h2>
        </div>

        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() => indexName && compactIndex.mutate(indexName)}
            disabled={compactIndex.isPending}
            title="Merge segments and reclaim disk space from deleted documents"
          >
            <HardDriveDownload className="h-4 w-4 mr-1" />
            {compactIndex.isPending ? 'Compacting...' : 'Compact Index'}
          </Button>

          <Button
            variant={showJson ? 'default' : 'outline'}
            size="sm"
            onClick={() => setShowJson(!showJson)}
            title="View settings as JSON"
          >
            <Code className="h-4 w-4 mr-1" />
            JSON
          </Button>

          {isDirty && (
            <>
              <Button
                variant="outline"
                size="sm"
                onClick={handleReset}
                disabled={updateSettings.isPending}
              >
                <RotateCcw className="h-4 w-4 mr-1" />
                Reset
              </Button>
              <Button
                size="sm"
                onClick={handleSave}
                disabled={updateSettings.isPending}
              >
                <Save className="h-4 w-4 mr-1" />
                {updateSettings.isPending ? 'Saving...' : 'Save Changes'}
              </Button>
            </>
          )}
        </div>
      </div>

      {/* Settings content */}
      <div className="flex-1 overflow-y-auto">
        {isLoadingSettings ? (
          <div className="space-y-6">
            {[1, 2].map((i) => (
              <Card key={i} className="p-6 space-y-4">
                <div className="space-y-2">
                  <Skeleton className="h-6 w-40" />
                  <Skeleton className="h-4 w-64" />
                </div>
                <div className="space-y-3">
                  <Skeleton className="h-4 w-32" />
                  <Skeleton className="h-20 w-full rounded-md" />
                </div>
              </Card>
            ))}
          </div>
        ) : showJson ? (
          <Card className="p-0 overflow-hidden">
            <div className="px-4 py-2 border-b bg-muted/50">
              <p className="text-xs text-muted-foreground">
                Raw settings for <span className="font-medium">{indexName}</span> — this is the format stored in <code className="text-xs">settings.json</code>
              </p>
            </div>
            <Suspense
              fallback={
                <div className="h-96 flex items-center justify-center text-muted-foreground">
                  Loading editor...
                </div>
              }
            >
              <Editor
                height="calc(100vh - 240px)"
                defaultLanguage="json"
                value={JSON.stringify(currentSettings, null, 2)}
                options={{
                  readOnly: true,
                  minimap: { enabled: false },
                  scrollBeyondLastLine: false,
                  lineNumbers: 'on',
                  folding: true,
                  fontSize: 13,
                  wordWrap: 'on',
                }}
                theme="vs-dark"
              />
            </Suspense>
          </Card>
        ) : (
          <SettingsForm
            settings={currentSettings}
            savedSettings={settings}
            onChange={handleFormChange}
            indexName={indexName}
          />
        )}
      </div>
    </div>
  );
}
