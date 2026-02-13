import { useState, useCallback, lazy, Suspense } from 'react';
import { useParams, Link } from 'react-router-dom';
import { ChevronLeft, Plus, Trash2, Search, Power, PowerOff, Wand2 } from 'lucide-react';
import { Card } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import { Skeleton } from '@/components/ui/skeleton';
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter, DialogDescription } from '@/components/ui/dialog';
import { useRules, useSaveRule, useDeleteRule, useClearRules } from '@/hooks/useRules';
import type { Rule } from '@/lib/types';

const Editor = lazy(() =>
  import('@monaco-editor/react').then((module) => ({ default: module.default }))
);

function getRuleDescription(rule: Rule): string {
  const parts: string[] = [];
  if (rule.conditions.length > 0) {
    const cond = rule.conditions[0];
    parts.push(`When query ${cond.anchoring} "${cond.pattern}"`);
  }
  const promotes = rule.consequence.promote?.length || 0;
  const hides = rule.consequence.hide?.length || 0;
  if (promotes) parts.push(`pin ${promotes} result${promotes > 1 ? 's' : ''}`);
  if (hides) parts.push(`hide ${hides} result${hides > 1 ? 's' : ''}`);
  if (rule.consequence.params?.query !== undefined) parts.push('modify query');
  return parts.join(', ') || 'No conditions or consequences';
}

function getEmptyRule(): Rule {
  return {
    objectID: `rule-${Date.now()}`,
    conditions: [{ pattern: '', anchoring: 'contains' }],
    consequence: {},
    description: '',
    enabled: true,
  };
}

export function Rules() {
  const { indexName } = useParams<{ indexName: string }>();
  const [searchQuery, setSearchQuery] = useState('');
  const [editingRule, setEditingRule] = useState<Rule | null>(null);
  const [isCreating, setIsCreating] = useState(false);

  const { data, isLoading } = useRules({
    indexName: indexName || '',
    query: searchQuery,
  });

  const saveRule = useSaveRule(indexName || '');
  const deleteRule = useDeleteRule(indexName || '');
  const clearRules = useClearRules(indexName || '');

  const handleSave = useCallback(async (rule: Rule) => {
    await saveRule.mutateAsync(rule);
    setEditingRule(null);
    setIsCreating(false);
  }, [saveRule]);

  const handleDelete = useCallback(async (objectID: string) => {
    if (!confirm(`Delete rule "${objectID}"?`)) return;
    await deleteRule.mutateAsync(objectID);
  }, [deleteRule]);

  const handleClearAll = useCallback(async () => {
    if (!confirm('Delete ALL rules for this index? This cannot be undone.')) return;
    await clearRules.mutateAsync();
  }, [clearRules]);

  if (!indexName) {
    return (
      <Card className="p-8 text-center">
        <h3 className="text-lg font-semibold mb-2">No index selected</h3>
        <Link to="/overview"><Button>Go to Overview</Button></Link>
      </Card>
    );
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Link to={`/index/${encodeURIComponent(indexName)}`}>
            <Button variant="ghost" size="sm">
              <ChevronLeft className="h-4 w-4 mr-1" />
              {indexName}
            </Button>
          </Link>
          <span className="text-muted-foreground">/</span>
          <h2 className="text-xl font-semibold">Rules</h2>
          {data && (
            <Badge variant="secondary" className="ml-2" data-testid="rules-count-badge">{data.nbHits}</Badge>
          )}
        </div>
        <div className="flex items-center gap-2">
          <Link to={`/index/${encodeURIComponent(indexName)}/merchandising`}>
            <Button variant="outline" size="sm">
              <Wand2 className="h-4 w-4 mr-1" />
              Merchandising Studio
            </Button>
          </Link>
          {data && data.nbHits > 0 && (
            <Button variant="outline" size="sm" onClick={handleClearAll}>
              <Trash2 className="h-4 w-4 mr-1" />
              Clear All
            </Button>
          )}
          <Button onClick={() => { setEditingRule(getEmptyRule()); setIsCreating(true); }}>
            <Plus className="h-4 w-4 mr-1" />
            Add Rule
          </Button>
        </div>
      </div>

      {/* Search */}
      <div className="relative">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
        <Input
          placeholder="Search rules..."
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          className="pl-9"
        />
      </div>

      {/* Rules list */}
      {isLoading ? (
        <div className="space-y-3">
          {[1, 2, 3].map((i) => (
            <Card key={i} className="p-4">
              <Skeleton className="h-5 w-full" />
            </Card>
          ))}
        </div>
      ) : !data || data.nbHits === 0 ? (
        <Card className="p-8 text-center">
          <h3 className="text-lg font-semibold mb-2">No rules</h3>
          <p className="text-sm text-muted-foreground mb-4">
            Rules let you customize search results for specific queries.
            Pin products to the top, hide irrelevant results, or modify queries.
          </p>
          <div className="flex items-center justify-center gap-2">
            <Button onClick={() => { setEditingRule(getEmptyRule()); setIsCreating(true); }}>
              <Plus className="h-4 w-4 mr-1" /> Create a Rule
            </Button>
            <Link to={`/index/${encodeURIComponent(indexName)}/merchandising`}>
              <Button variant="outline">
                <Wand2 className="h-4 w-4 mr-1" /> Open Merchandising Studio
              </Button>
            </Link>
          </div>
        </Card>
      ) : (
        <div className="space-y-2" data-testid="rules-list">
          {data.hits.map((rule) => (
            <Card key={rule.objectID} data-testid="rule-card" className="p-4 hover:bg-accent/50 transition-colors">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-3 min-w-0">
                  {rule.enabled !== false ? (
                    <Power className="h-4 w-4 text-green-500 shrink-0" />
                  ) : (
                    <PowerOff className="h-4 w-4 text-muted-foreground shrink-0" />
                  )}
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="font-mono text-sm font-medium truncate">
                        {rule.objectID}
                      </span>
                      {rule.consequence.promote && (
                        <Badge variant="secondary" className="text-xs">
                          {rule.consequence.promote.length} pinned
                        </Badge>
                      )}
                      {rule.consequence.hide && (
                        <Badge variant="outline" className="text-xs">
                          {rule.consequence.hide.length} hidden
                        </Badge>
                      )}
                    </div>
                    {rule.description && (
                      <p className="text-sm text-muted-foreground truncate">{rule.description}</p>
                    )}
                    <p className="text-xs text-muted-foreground mt-0.5">
                      {getRuleDescription(rule)}
                    </p>
                  </div>
                </div>
                <div className="flex items-center gap-1 shrink-0">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => { setEditingRule({ ...rule }); setIsCreating(false); }}
                  >
                    Edit
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => handleDelete(rule.objectID)}
                    disabled={deleteRule.isPending}
                    aria-label="Delete"
                  >
                    <Trash2 className="h-4 w-4 text-destructive" />
                  </Button>
                </div>
              </div>
            </Card>
          ))}
        </div>
      )}

      {/* Edit / Create Dialog */}
      {editingRule && (
        <RuleEditor
          rule={editingRule}
          isCreating={isCreating}
          onSave={handleSave}
          onCancel={() => { setEditingRule(null); setIsCreating(false); }}
          isPending={saveRule.isPending}
        />
      )}
    </div>
  );
}

interface RuleEditorProps {
  rule: Rule;
  isCreating: boolean;
  onSave: (rule: Rule) => void;
  onCancel: () => void;
  isPending: boolean;
}

function RuleEditor({ rule: initial, isCreating, onSave, onCancel, isPending }: RuleEditorProps) {
  const [json, setJson] = useState(JSON.stringify(initial, null, 2));
  const [parseError, setParseError] = useState<string | null>(null);

  const handleSave = () => {
    try {
      const parsed = JSON.parse(json) as Rule;
      if (!parsed.objectID) {
        setParseError('objectID is required');
        return;
      }
      if (!parsed.consequence) {
        setParseError('consequence is required');
        return;
      }
      setParseError(null);
      onSave(parsed);
    } catch (e: any) {
      setParseError(e.message);
    }
  };

  return (
    <Dialog open onOpenChange={() => onCancel()}>
      <DialogContent className="max-w-2xl max-h-[80vh] flex flex-col">
        <DialogHeader>
          <DialogTitle>{isCreating ? 'Create Rule' : `Edit Rule: ${initial.objectID}`}</DialogTitle>
          <DialogDescription>
            Edit the rule JSON directly. The rule must have an objectID and consequence.
          </DialogDescription>
        </DialogHeader>

        <div className="flex-1 min-h-0">
          <Suspense
            fallback={
              <div className="h-64 flex items-center justify-center text-muted-foreground">
                Loading editor...
              </div>
            }
          >
            <div className="border rounded-md overflow-hidden">
              <Editor
                height="400px"
                defaultLanguage="json"
                value={json}
                onChange={(value) => setJson(value || '')}
                options={{
                  minimap: { enabled: false },
                  scrollBeyondLastLine: false,
                  lineNumbers: 'on',
                  folding: true,
                  fontSize: 13,
                  tabSize: 2,
                }}
                theme="vs-dark"
              />
            </div>
          </Suspense>
          {parseError && (
            <p className="text-sm text-destructive mt-2">{parseError}</p>
          )}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={onCancel}>Cancel</Button>
          <Button onClick={handleSave} disabled={isPending}>
            {isPending ? 'Saving...' : isCreating ? 'Create' : 'Save'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
