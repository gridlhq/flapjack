import { useState, useCallback, useMemo } from 'react';
import { useParams, Link } from 'react-router-dom';
import { ChevronLeft, Plus, HardDrive, Settings, BarChart3, Circle, BookA, Wand2 } from 'lucide-react';
import { Card } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Switch } from '@/components/ui/switch';
import { Label } from '@/components/ui/label';
import { SearchBox } from '@/components/search/SearchBox';
import { ResultsPanel } from '@/components/search/ResultsPanel';
import { FacetsPanel } from '@/components/search/FacetsPanel';
import { AddDocumentsDialog } from '@/components/documents/AddDocumentsDialog';
import { useIndices } from '@/hooks/useIndices';
import { formatBytes } from '@/lib/utils';
import api from '@/lib/api';
import type { SearchParams } from '@/lib/types';

export function SearchBrowse() {
  const { indexName } = useParams<{ indexName: string }>();
  const { data: indices } = useIndices();
  const [trackAnalytics, setTrackAnalytics] = useState(false);
  const [searchParams, setSearchParams] = useState<SearchParams>({
    query: '',
    hitsPerPage: 20,
    page: 0,
    attributesToHighlight: ['*'],
  });
  const [showAddDocs, setShowAddDocs] = useState(false);

  const currentIndex = indices?.find((idx) => idx.uid === indexName);

  // Generate a stable user token for the dashboard session
  const dashboardUserToken = useMemo(() => `dashboard-${crypto.randomUUID().slice(0, 8)}`, []);

  // Merge analytics params into search params when tracking is on
  const effectiveParams = useMemo<SearchParams>(() => {
    if (!trackAnalytics) return searchParams;
    return {
      ...searchParams,
      analytics: true,
      clickAnalytics: true,
      analyticsTags: ['source:dashboard'],
    };
  }, [searchParams, trackAnalytics]);

  const handleParamsChange = useCallback((updates: Partial<SearchParams>) => {
    setSearchParams((prev) => ({
      ...prev,
      ...updates,
      // Reset to page 0 when query/filters change
      page: updates.query !== undefined || updates.filters !== undefined || updates.facetFilters !== undefined ? 0 : prev.page,
    }));
  }, []);

  // Fire a click event when analytics tracking is on and user clicks a result
  const handleResultClick = useCallback(
    (objectID: string, position: number, queryID?: string) => {
      if (!trackAnalytics || !queryID || !indexName) return;
      api.post('/1/events', {
        events: [
          {
            eventType: 'click',
            eventName: 'Result Clicked',
            index: indexName,
            userToken: dashboardUserToken,
            queryID,
            objectIDs: [objectID],
            positions: [position],
            timestamp: Date.now(),
          },
        ],
      }).catch(() => {
        // Fire-and-forget - don't interrupt the user
      });
    },
    [trackAnalytics, indexName, dashboardUserToken]
  );

  if (!indexName) {
    return (
      <Card className="p-8 text-center">
        <h3 className="text-lg font-semibold mb-2">No index selected</h3>
        <p className="text-muted-foreground mb-4">
          Select an index from the Overview page to start searching
        </p>
        <Link to="/overview">
          <Button>Go to Overview</Button>
        </Link>
      </Card>
    );
  }

  return (
    <div className="h-full flex flex-col gap-4">
      {/* Breadcrumb + Index stats + Add Documents */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Link to="/overview">
            <Button variant="ghost" size="sm">
              <ChevronLeft className="h-4 w-4 mr-1" />
              Overview
            </Button>
          </Link>
          <span className="text-muted-foreground">/</span>
          <h2 className="text-xl font-semibold">{indexName}</h2>
          {currentIndex && (
            <span className="flex items-center gap-1 text-sm text-muted-foreground ml-2">
              <HardDrive className="h-3.5 w-3.5" />
              {formatBytes(currentIndex.dataSize || 0)}
              <span className="mx-1">·</span>
              {(currentIndex.entries || 0).toLocaleString()} docs
            </span>
          )}
        </div>
        <div className="flex items-center gap-3">
          {/* Analytics tracking toggle */}
          <div className="flex items-center gap-2">
            <Switch
              id="track-analytics"
              checked={trackAnalytics}
              onCheckedChange={setTrackAnalytics}
            />
            <Label htmlFor="track-analytics" className="text-sm cursor-pointer select-none flex items-center gap-1.5">
              {trackAnalytics && (
                <Circle className="h-2 w-2 fill-red-500 text-red-500 animate-pulse" />
              )}
              Track Analytics
            </Label>
          </div>

          <div className="h-4 w-px bg-border" />

          <Link to={`/index/${encodeURIComponent(indexName)}/synonyms`}>
            <Button variant="outline" size="sm">
              <BookA className="h-4 w-4 mr-1" />
              Synonyms
            </Button>
          </Link>
          <Link to={`/index/${encodeURIComponent(indexName)}/merchandising`}>
            <Button variant="outline" size="sm">
              <Wand2 className="h-4 w-4 mr-1" />
              Merchandising
            </Button>
          </Link>
          <Link to={`/index/${encodeURIComponent(indexName)}/analytics`}>
            <Button variant="outline" size="sm">
              <BarChart3 className="h-4 w-4 mr-1" />
              Analytics
            </Button>
          </Link>
          <Link to={`/index/${encodeURIComponent(indexName)}/settings`}>
            <Button variant="outline" size="sm" title={`Settings for ${indexName}`}>
              <Settings className="h-4 w-4 mr-1" />
              Settings
            </Button>
          </Link>
          <Button size="sm" onClick={() => setShowAddDocs(true)}>
            <Plus className="h-4 w-4 mr-1" />
            Add Documents
          </Button>
        </div>
      </div>

      <SearchBox
        indexName={indexName}
        params={searchParams}
        onParamsChange={handleParamsChange}
      />

      <div className="flex-1 grid grid-cols-1 lg:grid-cols-[1fr_300px] gap-4 min-h-0">
        <ResultsPanel
          indexName={indexName}
          params={effectiveParams}
          onParamsChange={handleParamsChange}
          onResultClick={trackAnalytics ? handleResultClick : undefined}
          userToken={trackAnalytics ? dashboardUserToken : undefined}
        />

        <FacetsPanel
          indexName={indexName}
          params={effectiveParams}
          onParamsChange={handleParamsChange}
        />
      </div>

      <AddDocumentsDialog
        open={showAddDocs}
        onOpenChange={setShowAddDocs}
        indexName={indexName}
      />
    </div>
  );
}
