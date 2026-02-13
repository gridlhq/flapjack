import { memo, useCallback, useState } from 'react';
import { Search, SlidersHorizontal, X } from 'lucide-react';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import type { SearchParams } from '@/lib/types';

interface SearchBoxProps {
  indexName: string;
  params: SearchParams;
  onParamsChange: (updates: Partial<SearchParams>) => void;
}

export const SearchBox = memo(function SearchBox({
  params,
  onParamsChange,
}: SearchBoxProps) {
  const [query, setQuery] = useState(params.query || '');
  const [showFilters, setShowFilters] = useState(false);
  const [filterInput, setFilterInput] = useState(params.filters || '');

  const handleSearch = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      onParamsChange({ query });
    },
    [query, onParamsChange]
  );

  const handleApplyFilters = useCallback(() => {
    onParamsChange({ filters: filterInput || undefined });
    setShowFilters(false);
  }, [filterInput, onParamsChange]);

  const handleClearFilters = useCallback(() => {
    setFilterInput('');
    onParamsChange({ filters: undefined });
  }, [onParamsChange]);

  const hasFilters = !!params.filters;

  return (
    <Card className="p-4">
      <form onSubmit={handleSearch} className="space-y-4">
        <div className="flex gap-2">
          <div className="relative flex-1">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
            <Input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search documents..."
              className="pl-9"
            />
          </div>
          <Button type="submit" size="default">
            Search
          </Button>
          <Button
            type="button"
            variant={hasFilters ? 'default' : 'outline'}
            size="icon"
            onClick={() => setShowFilters(!showFilters)}
          >
            <SlidersHorizontal className="h-4 w-4" />
          </Button>
        </div>

        {showFilters && (
          <div className="space-y-3 pt-2 border-t">
            <div className="flex items-center justify-between">
              <label className="text-sm font-medium">Filters</label>
              {hasFilters && (
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  onClick={handleClearFilters}
                >
                  <X className="h-3 w-3 mr-1" />
                  Clear
                </Button>
              )}
            </div>
            <Input
              value={filterInput}
              onChange={(e) => setFilterInput(e.target.value)}
              placeholder="e.g., category:books AND price > 10"
              className="font-mono text-sm"
            />
            <div className="flex justify-end gap-2">
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={() => setShowFilters(false)}
              >
                Cancel
              </Button>
              <Button type="button" size="sm" onClick={handleApplyFilters}>
                Apply Filters
              </Button>
            </div>
          </div>
        )}

        {hasFilters && !showFilters && (
          <div className="flex items-center gap-2">
            <span className="text-sm text-muted-foreground">Active filter:</span>
            <Badge variant="secondary" className="font-mono text-xs">
              {params.filters}
            </Badge>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={handleClearFilters}
            >
              <X className="h-3 w-3" />
            </Button>
          </div>
        )}
      </form>
    </Card>
  );
});
