import { memo, useCallback, useEffect, useRef, useState } from 'react';
import { Link } from 'react-router-dom';
import { Filter, X, Settings } from 'lucide-react';
import { useSearch } from '@/hooks/useSearch';
import { useDevMode } from '@/hooks/useDevMode';
import { Card } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Input } from '@/components/ui/input';
import type { SearchParams } from '@/lib/types';

interface FacetsPanelProps {
  indexName: string;
  params: SearchParams;
  onParamsChange: (updates: Partial<SearchParams>) => void;
}

interface FacetValueProps {
  facetName: string;
  value: string;
  count: number;
  isSelected: boolean;
  onToggle: () => void;
}

const FacetValue = memo(function FacetValue({
  value,
  count,
  isSelected,
  onToggle,
}: FacetValueProps) {
  return (
    <button
      onClick={onToggle}
      className="w-full flex items-center justify-between p-2 rounded-md hover:bg-accent text-sm group"
    >
      <div className="flex items-center gap-2 min-w-0">
        <div
          className={`h-4 w-4 border rounded flex-shrink-0 ${
            isSelected
              ? 'bg-primary border-primary'
              : 'border-muted-foreground group-hover:border-primary'
          }`}
        >
          {isSelected && (
            <svg className="h-3 w-3 text-primary-foreground" viewBox="0 0 12 12">
              <polyline
                points="2,6 5,9 10,3"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
              />
            </svg>
          )}
        </div>
        <span className="truncate">{value}</span>
      </div>
      <Badge variant="secondary" className="ml-2 shrink-0">
        {count.toLocaleString()}
      </Badge>
    </button>
  );
});

export const FacetsPanel = memo(function FacetsPanel({
  indexName,
  params,
  onParamsChange,
}: FacetsPanelProps) {
  const [searchQuery, setSearchQuery] = useState('');
  const devLog = useDevMode((s) => s.log);
  const renderCount = useRef(0);
  renderCount.current++;

  const hasActiveFacets = (params.facetFilters?.length || 0) > 0;

  // Main query: always runs. Gets facets + counts for the current search state
  // (query + facetFilters). This is the single source of truth for counts.
  const mainQuery = useSearch({
    indexName,
    params: {
      ...params,
      facets: ['*'],
    },
    keepPrevious: true,
  });

  // Base query: ONLY runs when facetFilters are active.
  // Purpose: get the full list of facet values (including those filtered to 0)
  // so users can see and deselect them. Uses hitsPerPage:1 (not 0) because
  // the backend can return empty facets with hitsPerPage:0 on some requests.
  const baseQuery = useSearch({
    indexName,
    params: {
      query: params.query || '',
      hitsPerPage: 1,
      facets: ['*'],
    },
    enabled: hasActiveFacets,
    keepPrevious: true,
  });

  // Dev mode logging
  useEffect(() => {
    const mainFacetKeys = Object.keys(mainQuery.data?.facets || {});
    const baseFacetKeys = Object.keys(baseQuery.data?.facets || {});

    const mainFacetDetail: Record<string, Record<string, number>> = {};
    for (const k of mainFacetKeys) {
      mainFacetDetail[k] = mainQuery.data?.facets?.[k] || {};
    }

    devLog('facets', [
      `render #${renderCount.current}`,
      `query="${params.query || ''}"`,
      `filters=${JSON.stringify(params.facetFilters || [])}`,
      `nbHits=${mainQuery.data?.nbHits ?? '?'}`,
    ].join(' | '), {
      main: {
        status: mainQuery.status,
        isFetching: mainQuery.isFetching,
        isPlaceholder: mainQuery.isPlaceholderData,
        facets: mainFacetDetail,
      },
      ...(hasActiveFacets ? {
        base: {
          status: baseQuery.status,
          isFetching: baseQuery.isFetching,
          isPlaceholder: baseQuery.isPlaceholderData,
          facetNames: baseFacetKeys,
        },
      } : {}),
    });
  }, [mainQuery.data, mainQuery.status, mainQuery.isFetching, mainQuery.isPlaceholderData,
      baseQuery.data, baseQuery.status, baseQuery.isFetching, baseQuery.isPlaceholderData,
      hasActiveFacets, params.query, params.facetFilters, devLog]);

  const handleToggleFacet = useCallback(
    (facetName: string, value: string) => {
      const currentFilters = params.facetFilters || [];
      const filterString = `${facetName}:${value}`;

      const isSelected = currentFilters.some((filter) =>
        Array.isArray(filter)
          ? filter.includes(filterString)
          : filter === filterString
      );

      let newFilters;
      if (isSelected) {
        newFilters = currentFilters.filter((filter) =>
          Array.isArray(filter)
            ? !filter.includes(filterString)
            : filter !== filterString
        );
      } else {
        newFilters = [...currentFilters, filterString];
      }

      devLog('facets', `toggle ${filterString} → ${isSelected ? 'OFF' : 'ON'} | newFilters=${JSON.stringify(newFilters)}`);

      onParamsChange({
        facetFilters: newFilters.length > 0 ? newFilters : undefined,
      });
    },
    [params.facetFilters, onParamsChange, devLog]
  );

  const handleClearAll = useCallback(() => {
    devLog('facets', 'clear all facet filters');
    onParamsChange({ facetFilters: undefined });
  }, [onParamsChange, devLog]);

  // When no filters active: main query is sole source for facet names + counts.
  // When filters active: base query provides all available facet values (so filtered-out
  // options still appear), main query provides accurate counts.
  const mainFacets = mainQuery.data?.facets || {};
  const baseFacets = baseQuery.data?.facets || {};
  const facetSource = hasActiveFacets ? baseFacets : mainFacets;
  const countSource = mainFacets;

  const facetNames = Object.keys(facetSource);

  // Filter facet values by local search query
  const filterFacetValues = (values: Record<string, number>) => {
    if (!searchQuery) return values;
    return Object.entries(values).reduce(
      (acc, [key, value]) => {
        if (key.toLowerCase().includes(searchQuery.toLowerCase())) {
          acc[key] = value;
        }
        return acc;
      },
      {} as Record<string, number>
    );
  };

  // Only show "No facets configured" when main query has settled AND returned no facets.
  // Never flash this during loading, fetching, or placeholder transitions.
  const mainSettled = mainQuery.status === 'success' && !mainQuery.isFetching && !mainQuery.isPlaceholderData;
  if (facetNames.length === 0 && !hasActiveFacets && mainSettled) {
    return (
      <Card className="p-6 text-center">
        <Filter className="h-8 w-8 mx-auto mb-2 text-muted-foreground" />
        <h4 className="text-sm font-medium mb-1">No facets configured</h4>
        <p className="text-xs text-muted-foreground mb-3">
          Add attributes to <span className="font-medium">Faceting</span> in settings to enable filtering.
        </p>
        <Link to={`/index/${encodeURIComponent(indexName)}/settings`}>
          <Button variant="outline" size="sm">
            <Settings className="h-3.5 w-3.5 mr-1" />
            Configure Facets
          </Button>
        </Link>
      </Card>
    );
  }

  return (
    <Card className="flex flex-col h-full" data-testid="facets-panel">
      <div className="p-4 border-b space-y-3">
        <div className="flex items-center justify-between">
          <h3 className="font-semibold flex items-center gap-2">
            <Filter className="h-4 w-4" />
            Facets
            {mainQuery.isFetching && (
              <span className="inline-block h-2 w-2 rounded-full bg-blue-500 animate-pulse" title="Updating..." />
            )}
          </h3>
          {hasActiveFacets && (
            <Button variant="ghost" size="sm" onClick={handleClearAll}>
              <X className="h-3 w-3 mr-1" />
              Clear
            </Button>
          )}
        </div>
        <Input
          placeholder="Filter facets..."
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          className="h-8"
        />
      </div>

      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {facetNames.map((facetName) => {
          const sourceValues = filterFacetValues(facetSource[facetName]);
          const counts = countSource[facetName] || {};
          const facetEntries = Object.entries(sourceValues);

          if (facetEntries.length === 0) return null;

          return (
            <div key={facetName} className="space-y-2">
              <h4 className="text-sm font-medium capitalize">
                {facetName.replace(/_/g, ' ')}
              </h4>
              <div className="space-y-1">
                {facetEntries
                  .map(([value]) => {
                    const filterString = `${facetName}:${value}`;
                    const isSelected =
                      params.facetFilters?.some((filter) =>
                        Array.isArray(filter)
                          ? filter.includes(filterString)
                          : filter === filterString
                      ) || false;

                    // Always use main query counts — they reflect current query + filters
                    const displayCount = counts[value] ?? 0;

                    return { value, isSelected, displayCount, filterString };
                  })
                  // Hide 0-count values unless they're actively selected
                  .filter((entry) => entry.displayCount > 0 || entry.isSelected)
                  // Sort by count descending (selected items with 0 count sort to bottom)
                  .sort((a, b) => b.displayCount - a.displayCount)
                  .slice(0, 10)
                  .map(({ value, isSelected, displayCount }) => {

                    return (
                      <FacetValue
                        key={value}
                        facetName={facetName}
                        value={value}
                        count={displayCount}
                        isSelected={isSelected}
                        onToggle={() => handleToggleFacet(facetName, value)}
                      />
                    );
                  })}
              </div>
            </div>
          );
        })}
      </div>
    </Card>
  );
});
