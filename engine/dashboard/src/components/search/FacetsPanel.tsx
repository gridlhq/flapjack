import { memo, useCallback, useState } from 'react';
import { Link } from 'react-router-dom';
import { Filter, X, Settings } from 'lucide-react';
import { useSearch } from '@/hooks/useSearch';
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

  // Get facets from search response
  const { data } = useSearch({
    indexName,
    params: {
      ...params,
      facets: ['*'], // Request all facets
    },
  });

  const handleToggleFacet = useCallback(
    (facetName: string, value: string) => {
      const currentFilters = params.facetFilters || [];
      const filterString = `${facetName}:${value}`;

      // Check if this facet is already selected
      const isSelected = currentFilters.some((filter) =>
        Array.isArray(filter)
          ? filter.includes(filterString)
          : filter === filterString
      );

      let newFilters;
      if (isSelected) {
        // Remove the filter
        newFilters = currentFilters.filter((filter) =>
          Array.isArray(filter)
            ? !filter.includes(filterString)
            : filter !== filterString
        );
      } else {
        // Add the filter
        newFilters = [...currentFilters, filterString];
      }

      onParamsChange({
        facetFilters: newFilters.length > 0 ? newFilters : undefined,
      });
    },
    [params.facetFilters, onParamsChange]
  );

  const handleClearAll = useCallback(() => {
    onParamsChange({ facetFilters: undefined });
  }, [onParamsChange]);

  const facets = data?.facets || {};
  const facetNames = Object.keys(facets);
  const hasActiveFacets = (params.facetFilters?.length || 0) > 0;

  // Filter facet values by search query
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

  if (facetNames.length === 0) {
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
          const facetValues = filterFacetValues(facets[facetName]);
          const facetEntries = Object.entries(facetValues);

          if (facetEntries.length === 0) return null;

          return (
            <div key={facetName} className="space-y-2">
              <h4 className="text-sm font-medium capitalize">
                {facetName.replace(/_/g, ' ')}
              </h4>
              <div className="space-y-1">
                {facetEntries
                  .sort(([, a], [, b]) => b - a) // Sort by count descending
                  .slice(0, 10) // Limit to top 10
                  .map(([value, count]) => {
                    const filterString = `${facetName}:${value}`;
                    const isSelected =
                      params.facetFilters?.some((filter) =>
                        Array.isArray(filter)
                          ? filter.includes(filterString)
                          : filter === filterString
                      ) || false;

                    return (
                      <FacetValue
                        key={value}
                        facetName={facetName}
                        value={value}
                        count={count}
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
