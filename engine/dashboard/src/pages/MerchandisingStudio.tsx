import { useState, useCallback, useMemo } from 'react';
import { useParams, Link } from 'react-router-dom';
import {
  ChevronLeft, Search, Pin, EyeOff, Eye, Undo2, Save,
  GripVertical, ArrowUp, ArrowDown,
} from 'lucide-react';
import { Card } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import { Label } from '@/components/ui/label';
import { useSearch } from '@/hooks/useSearch';
import { useSaveRule, useRules } from '@/hooks/useRules';
import { useToast } from '@/hooks/use-toast';
import type { Rule, RulePromote, RuleHide } from '@/lib/types';

interface PinAction {
  objectID: string;
  position: number;
}

interface HideAction {
  objectID: string;
}

export function MerchandisingStudio() {
  const { indexName } = useParams<{ indexName: string }>();
  const { toast } = useToast();

  const [query, setQuery] = useState('');
  const [submittedQuery, setSubmittedQuery] = useState('');
  const [pins, setPins] = useState<PinAction[]>([]);
  const [hides, setHides] = useState<HideAction[]>([]);
  const [ruleDescription, setRuleDescription] = useState('');

  const saveRule = useSaveRule(indexName || '');

  // Load existing rules for this query
  const { data: existingRules } = useRules({
    indexName: indexName || '',
    query: submittedQuery,
    hitsPerPage: 100,
  });

  // Search results
  const { data: searchData, isLoading } = useSearch({
    indexName: indexName || '',
    params: {
      query: submittedQuery,
      hitsPerPage: 50,
      page: 0,
      attributesToHighlight: ['*'],
    },
    enabled: !!submittedQuery,
  });

  const handleSearch = useCallback(() => {
    setSubmittedQuery(query);
    setPins([]);
    setHides([]);
    setRuleDescription('');
  }, [query]);

  // Apply pins and hides to the result list for preview
  const previewResults = useMemo(() => {
    if (!searchData?.hits) return [];

    // Filter out hidden results
    const hiddenIds = new Set(hides.map((h) => h.objectID));
    let results = searchData.hits.filter((hit) => !hiddenIds.has(hit.objectID));

    // Remove pinned items from their natural positions
    const pinnedIds = new Set(pins.map((p) => p.objectID));
    const unpinned = results.filter((hit) => !pinnedIds.has(hit.objectID));

    // Insert pinned items at their target positions
    const final = [...unpinned];
    const sortedPins = [...pins].sort((a, b) => a.position - b.position);
    for (const pin of sortedPins) {
      const hit = searchData.hits.find((h) => h.objectID === pin.objectID);
      if (hit) {
        final.splice(Math.min(pin.position, final.length), 0, hit);
      }
    }

    return final;
  }, [searchData, pins, hides]);

  const isPinned = (objectID: string) => pins.some((p) => p.objectID === objectID);

  const togglePin = useCallback((objectID: string, currentPosition: number) => {
    setPins((prev) => {
      if (prev.some((p) => p.objectID === objectID)) {
        return prev.filter((p) => p.objectID !== objectID);
      }
      return [...prev, { objectID, position: currentPosition }];
    });
  }, []);

  const toggleHide = useCallback((objectID: string) => {
    setHides((prev) => {
      if (prev.some((h) => h.objectID === objectID)) {
        return prev.filter((h) => h.objectID !== objectID);
      }
      return [...prev, { objectID }];
    });
    // Remove from pins if hidden
    setPins((prev) => prev.filter((p) => p.objectID !== objectID));
  }, []);

  const movePin = useCallback((objectID: string, direction: 'up' | 'down') => {
    setPins((prev) => {
      return prev.map((p) => {
        if (p.objectID !== objectID) return p;
        const newPos = direction === 'up' ? Math.max(0, p.position - 1) : p.position + 1;
        return { ...p, position: newPos };
      });
    });
  }, []);

  const handleReset = useCallback(() => {
    setPins([]);
    setHides([]);
    setRuleDescription('');
  }, []);

  const hasChanges = pins.length > 0 || hides.length > 0;

  const handleSaveRule = useCallback(async () => {
    if (!submittedQuery || !hasChanges) return;

    const promote: RulePromote[] = pins.map((p) => ({
      objectID: p.objectID,
      position: p.position,
    }));

    const hide: RuleHide[] = hides.map((h) => ({
      objectID: h.objectID,
    }));

    const rule: Rule = {
      objectID: `merch-${submittedQuery.replace(/\s+/g, '-').toLowerCase()}-${Date.now()}`,
      conditions: [{
        pattern: submittedQuery,
        anchoring: 'is',
      }],
      consequence: {
        ...(promote.length > 0 ? { promote } : {}),
        ...(hide.length > 0 ? { hide } : {}),
      },
      description: ruleDescription || `Merchandising: "${submittedQuery}"`,
      enabled: true,
    };

    try {
      await saveRule.mutateAsync(rule);
      toast({ title: 'Merchandising rule saved', description: `Rule "${rule.objectID}" created for query "${submittedQuery}"` });
      setPins([]);
      setHides([]);
      setRuleDescription('');
    } catch {
      // Error toast handled by hook
    }
  }, [submittedQuery, hasChanges, pins, hides, ruleDescription, saveRule, toast]);

  if (!indexName) {
    return (
      <Card className="p-8 text-center">
        <h3 className="text-lg font-semibold mb-2">No index selected</h3>
        <Link to="/overview"><Button>Go to Overview</Button></Link>
      </Card>
    );
  }

  return (
    <div className="h-full flex flex-col gap-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Link to={`/index/${encodeURIComponent(indexName)}/rules`}>
            <Button variant="ghost" size="sm">
              <ChevronLeft className="h-4 w-4 mr-1" />
              Rules
            </Button>
          </Link>
          <span className="text-muted-foreground">/</span>
          <h2 className="text-xl font-semibold">Merchandising Studio</h2>
        </div>
        <div className="flex items-center gap-2">
          {hasChanges && (
            <>
              <Badge variant="secondary">
                {pins.length} pinned, {hides.length} hidden
              </Badge>
              <Button variant="outline" size="sm" onClick={handleReset}>
                <Undo2 className="h-4 w-4 mr-1" />
                Reset
              </Button>
              <Button size="sm" onClick={handleSaveRule} disabled={saveRule.isPending}>
                <Save className="h-4 w-4 mr-1" />
                {saveRule.isPending ? 'Saving...' : 'Save as Rule'}
              </Button>
            </>
          )}
        </div>
      </div>

      {/* Search bar */}
      <div className="flex gap-2">
        <div className="relative flex-1">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
          <Input
            placeholder="Enter a search query to merchandise..."
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleSearch()}
            className="pl-9"
          />
        </div>
        <Button onClick={handleSearch}>Search</Button>
      </div>

      {/* Description field (when changes exist) */}
      {hasChanges && (
        <div className="flex items-center gap-2">
          <Label className="shrink-0">Rule description:</Label>
          <Input
            placeholder={`Merchandising: "${submittedQuery}"`}
            value={ruleDescription}
            onChange={(e) => setRuleDescription(e.target.value)}
            className="max-w-md"
          />
        </div>
      )}

      {/* Content area */}
      <div className="flex-1 grid grid-cols-1 lg:grid-cols-[1fr_280px] gap-4 min-h-0">
        {/* Results grid */}
        <div className="overflow-auto space-y-2">
          {!submittedQuery ? (
            <Card className="p-8 text-center">
              <h3 className="text-lg font-semibold mb-2">Enter a search query</h3>
              <p className="text-sm text-muted-foreground">
                Type a query above to see search results, then pin or hide results
                to create a merchandising rule.
              </p>
            </Card>
          ) : isLoading ? (
            <Card className="p-8 text-center text-muted-foreground">Searching...</Card>
          ) : previewResults.length === 0 ? (
            <Card className="p-8 text-center text-muted-foreground">No results</Card>
          ) : (
            <>
              <p className="text-sm text-muted-foreground">
                {searchData?.nbHits} results for "{submittedQuery}" ({searchData?.processingTimeMS}ms)
              </p>
              {previewResults.map((hit, index) => {
                const pinned = isPinned(hit.objectID);
                const { objectID, _highlightResult, ...fields } = hit;
                const primaryField = Object.keys(fields)[0];
                const secondaryField = Object.keys(fields)[1];

                return (
                  <Card
                    key={hit.objectID}
                    className={`p-4 transition-all ${
                      pinned
                        ? 'border-blue-500 bg-blue-50/50 dark:bg-blue-950/20'
                        : ''
                    }`}
                    data-testid="merch-card"
                  >
                    <div className="flex items-center gap-3">
                      {/* Position indicator */}
                      <div className="flex flex-col items-center gap-0.5 shrink-0 w-8">
                        {pinned ? (
                          <>
                            <Button
                              variant="ghost"
                              size="sm"
                              className="h-5 w-5 p-0"
                              title="Move up"
                              onClick={() => movePin(objectID, 'up')}
                            >
                              <ArrowUp className="h-3 w-3" />
                            </Button>
                            <Badge className="text-xs bg-blue-500">#{index + 1}</Badge>
                            <Button
                              variant="ghost"
                              size="sm"
                              className="h-5 w-5 p-0"
                              title="Move down"
                              onClick={() => movePin(objectID, 'down')}
                            >
                              <ArrowDown className="h-3 w-3" />
                            </Button>
                          </>
                        ) : (
                          <span className="text-xs text-muted-foreground">{index + 1}</span>
                        )}
                      </div>

                      {/* Drag handle (visual only for now) */}
                      <GripVertical className="h-4 w-4 text-muted-foreground/40 shrink-0" />

                      {/* Document content */}
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2">
                          <Badge variant="outline" className="font-mono text-xs shrink-0">
                            {objectID}
                          </Badge>
                          {pinned && (
                            <Badge variant="secondary" className="text-xs">
                              <Pin className="h-3 w-3 mr-1" />
                              Pinned #{pins.find((p) => p.objectID === objectID)?.position}
                            </Badge>
                          )}
                        </div>
                        {primaryField && (
                          <p className="text-sm font-medium mt-1 truncate">
                            {String(fields[primaryField])}
                          </p>
                        )}
                        {secondaryField && (
                          <p className="text-xs text-muted-foreground truncate">
                            {String(fields[secondaryField])}
                          </p>
                        )}
                      </div>

                      {/* Actions */}
                      <div className="flex items-center gap-1 shrink-0">
                        <Button
                          variant={pinned ? 'default' : 'outline'}
                          size="sm"
                          onClick={() => togglePin(objectID, index)}
                          title={pinned ? 'Unpin' : 'Pin to this position'}
                        >
                          <Pin className="h-4 w-4" />
                        </Button>
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => toggleHide(objectID)}
                          title="Hide from results"
                        >
                          <EyeOff className="h-4 w-4" />
                        </Button>
                      </div>
                    </div>
                  </Card>
                );
              })}

              {/* Hidden items section */}
              {hides.length > 0 && (
                <div className="mt-6">
                  <h3 className="text-sm font-medium text-muted-foreground mb-2">
                    Hidden Results ({hides.length})
                  </h3>
                  <div className="space-y-1">
                    {hides.map((hide) => {
                      const hit = searchData?.hits.find((h) => h.objectID === hide.objectID);
                      const fields = hit ? Object.keys(hit).filter((k) => k !== 'objectID' && k !== '_highlightResult') : [];
                      return (
                        <Card key={hide.objectID} className="p-3 bg-red-50/50 dark:bg-red-950/20 border-red-200 dark:border-red-900">
                          <div className="flex items-center justify-between">
                            <div className="flex items-center gap-2 min-w-0">
                              <EyeOff className="h-4 w-4 text-red-500 shrink-0" />
                              <Badge variant="outline" className="font-mono text-xs line-through">
                                {hide.objectID}
                              </Badge>
                              {hit && fields[0] && (
                                <span className="text-sm text-muted-foreground truncate line-through">
                                  {String((hit as any)[fields[0]])}
                                </span>
                              )}
                            </div>
                            <Button
                              variant="ghost"
                              size="sm"
                              onClick={() => toggleHide(hide.objectID)}
                              title="Unhide"
                            >
                              <Eye className="h-4 w-4" />
                            </Button>
                          </div>
                        </Card>
                      );
                    })}
                  </div>
                </div>
              )}
            </>
          )}
        </div>

        {/* Sidebar: Existing rules for this query */}
        <div className="space-y-4">
          <Card className="p-4">
            <h3 className="font-semibold text-sm mb-3">How it works</h3>
            <div className="space-y-2 text-xs text-muted-foreground">
              <p>1. Search for a query your users type</p>
              <p>2. <Pin className="inline h-3 w-3" /> <strong>Pin</strong> results to lock them at a position</p>
              <p>3. <EyeOff className="inline h-3 w-3" /> <strong>Hide</strong> irrelevant results</p>
              <p>4. <Save className="inline h-3 w-3" /> <strong>Save</strong> as a rule</p>
            </div>
          </Card>

          {existingRules && existingRules.nbHits > 0 && (
            <Card className="p-4">
              <h3 className="font-semibold text-sm mb-2">Existing Rules</h3>
              <div className="space-y-2">
                {existingRules.hits.map((rule) => (
                  <div key={rule.objectID} className="text-xs p-2 bg-muted rounded-md">
                    <p className="font-mono font-medium truncate">{rule.objectID}</p>
                    {rule.description && (
                      <p className="text-muted-foreground mt-0.5">{rule.description}</p>
                    )}
                    {rule.consequence.promote && (
                      <Badge variant="secondary" className="text-xs mt-1">
                        {rule.consequence.promote.length} pinned
                      </Badge>
                    )}
                    {rule.consequence.hide && (
                      <Badge variant="outline" className="text-xs mt-1 ml-1">
                        {rule.consequence.hide.length} hidden
                      </Badge>
                    )}
                  </div>
                ))}
              </div>
            </Card>
          )}
        </div>
      </div>
    </div>
  );
}
