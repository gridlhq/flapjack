import { memo, useState, useCallback } from 'react';
import { useCreateIndex, useIndexes } from '@/hooks/useIndexes';
import { useAddDocuments } from '@/hooks/useDocuments';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Database, ShoppingBag } from 'lucide-react';
import { cn } from '@/lib/utils';
import api from '@/lib/api';
import moviesData from '@/data/movies.json';
import productsData from '@/data/products.json';
import type { IndexSettings, Synonym, Rule } from '@/lib/types';

type IndexTemplate = 'empty' | 'movies' | 'products';

interface TemplateConfig {
  value: IndexTemplate;
  label: string;
  desc: string;
  icon: typeof Database | null;
  defaultName: string;
  settings: Partial<IndexSettings>;
  synonyms: Synonym[];
  rules: Rule[];
  getData: () => Record<string, unknown>[];
}

const MOVIES_SYNONYMS: Synonym[] = [
  { type: 'synonym', objectID: 'syn-film-movie', synonyms: ['film', 'movie', 'picture', 'flick'] },
  { type: 'synonym', objectID: 'syn-scary-horror', synonyms: ['scary', 'horror', 'frightening', 'terrifying'] },
  { type: 'synonym', objectID: 'syn-funny-comedy', synonyms: ['funny', 'comedy', 'humorous', 'hilarious'] },
  { type: 'onewaysynonym', objectID: 'syn-scifi', input: 'sci-fi', synonyms: ['science fiction', 'futuristic', 'space'] },
  { type: 'onewaysynonym', objectID: 'syn-animated', input: 'cartoon', synonyms: ['animated', 'animation'] },
  { type: 'synonym', objectID: 'syn-war-battle', synonyms: ['war', 'battle', 'combat', 'military'] },
  { type: 'synonym', objectID: 'syn-love-romance', synonyms: ['love', 'romance', 'romantic'] },
  { type: 'synonym', objectID: 'syn-detective-mystery', synonyms: ['detective', 'mystery', 'whodunit', 'investigation'] },
];

const MOVIES_RULES: Rule[] = [
  {
    objectID: 'rule-best-movies',
    conditions: [{ pattern: 'best', anchoring: 'contains' }],
    consequence: {
      params: { query: '' },
    },
    description: 'When searching "best", show highest-rated movies',
  },
  {
    objectID: 'rule-nolan',
    conditions: [{ pattern: 'nolan', anchoring: 'contains' }],
    consequence: {
      promote: [
        { objectID: 'movie_3', position: 0 },
        { objectID: 'movie_9', position: 1 },
        { objectID: 'movie_13', position: 2 },
      ],
    },
    description: 'Promote top Nolan films when searching for "nolan"',
  },
];

const PRODUCTS_SYNONYMS: Synonym[] = [
  { type: 'synonym', objectID: 'syn-laptop-notebook', synonyms: ['laptop', 'notebook', 'computer'] },
  { type: 'synonym', objectID: 'syn-headphones-earbuds', synonyms: ['headphones', 'earbuds', 'earphones', 'headset'] },
  { type: 'synonym', objectID: 'syn-shirt-tee', synonyms: ['shirt', 'tee', 't-shirt', 'top'] },
  { type: 'synonym', objectID: 'syn-sneakers-shoes', synonyms: ['sneakers', 'shoes', 'trainers', 'kicks'] },
  { type: 'onewaysynonym', objectID: 'syn-cheap', input: 'cheap', synonyms: ['affordable', 'budget', 'value'] },
  { type: 'onewaysynonym', objectID: 'syn-premium', input: 'premium', synonyms: ['luxury', 'high-end', 'pro'] },
  { type: 'synonym', objectID: 'syn-bottle-flask', synonyms: ['bottle', 'flask', 'tumbler', 'canteen'] },
  { type: 'synonym', objectID: 'syn-bag-backpack', synonyms: ['bag', 'backpack', 'pack', 'rucksack'] },
];

const PRODUCTS_RULES: Rule[] = [
  {
    objectID: 'rule-featured-electronics',
    conditions: [{ pattern: 'electronics', anchoring: 'contains' }],
    consequence: {
      promote: [
        { objectID: 'prod_1', position: 0 },
        { objectID: 'prod_7', position: 1 },
      ],
    },
    description: 'Promote headphones and keyboard for electronics searches',
  },
  {
    objectID: 'rule-gift-ideas',
    conditions: [{ pattern: 'gift', anchoring: 'contains' }],
    consequence: {
      promote: [
        { objectID: 'prod_9', position: 0 },
        { objectID: 'prod_5', position: 1 },
      ],
    },
    description: 'Promote giftable items when searching for "gift"',
  },
];

const TEMPLATE_CONFIGS: TemplateConfig[] = [
  {
    value: 'empty',
    label: 'Empty index',
    desc: 'Start from scratch — add your own documents later',
    icon: null,
    defaultName: '',
    settings: {},
    synonyms: [],
    rules: [],
    getData: () => [],
  },
  {
    value: 'movies',
    label: 'Movies — 1,000 docs',
    desc: 'Search by title/director, filter by genre, includes synonyms & merchandising rules',
    icon: Database,
    defaultName: 'movies',
    settings: {
      searchableAttributes: ['title', 'overview', 'director'],
      attributesForFaceting: ['genre', 'director', 'year'],
      attributesToHighlight: ['title', 'overview', 'director'],
    },
    synonyms: MOVIES_SYNONYMS,
    rules: MOVIES_RULES,
    getData: () => moviesData.slice(0, 1000),
  },
  {
    value: 'products',
    label: 'Products — 1,000 docs',
    desc: 'E-commerce demo with facets, synonyms & merchandising rules',
    icon: ShoppingBag,
    defaultName: 'products',
    settings: {
      searchableAttributes: ['name', 'description', 'brand', 'category'],
      attributesForFaceting: ['category', 'brand', 'inStock'],
      attributesToHighlight: ['name', 'description'],
    },
    synonyms: PRODUCTS_SYNONYMS,
    rules: PRODUCTS_RULES,
    getData: () => productsData.slice(0, 1000),
  },
];

interface CreateIndexDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export const CreateIndexDialog = memo(function CreateIndexDialog({
  open,
  onOpenChange,
}: CreateIndexDialogProps) {
  const createIndex = useCreateIndex();
  const { data: existingIndexes } = useIndexes();
  const [uid, setUid] = useState('');
  const [error, setError] = useState('');
  const [template, setTemplate] = useState<IndexTemplate>('empty');
  const [isLoadingData, setIsLoadingData] = useState(false);

  const addDocuments = useAddDocuments(uid.trim());

  const selectedConfig = TEMPLATE_CONFIGS.find((t) => t.value === template)!;

  const handleTemplateChange = useCallback((newTemplate: IndexTemplate) => {
    setTemplate(newTemplate);
    const config = TEMPLATE_CONFIGS.find((t) => t.value === newTemplate)!;
    if (config.defaultName) {
      setUid(config.defaultName);
    } else {
      setUid('');
    }
    setError('');
  }, []);

  const handleCreate = useCallback(async () => {
    const trimmed = uid.trim();
    if (!trimmed) {
      setError('Index name is required');
      return;
    }
    if (!/^[a-zA-Z0-9_-]+$/.test(trimmed)) {
      setError('Only letters, numbers, hyphens, and underscores allowed');
      return;
    }
    if (existingIndexes?.some((idx) => idx.uid === trimmed)) {
      setError(`An index named "${trimmed}" already exists`);
      return;
    }

    try {
      await createIndex.mutateAsync({ uid: trimmed });

      const data = selectedConfig.getData();
      if (data.length > 0) {
        setIsLoadingData(true);

        // Configure settings BEFORE adding documents so facets are indexed correctly
        if (Object.keys(selectedConfig.settings).length > 0) {
          await api.put(`/1/indexes/${trimmed}/settings`, selectedConfig.settings);
        }

        await addDocuments.mutateAsync(data);

        // Seed synonyms
        for (const synonym of selectedConfig.synonyms) {
          try {
            await api.put(`/1/indexes/${trimmed}/synonyms/${synonym.objectID}`, synonym);
          } catch { /* non-critical */ }
        }

        // Seed merchandising rules
        for (const rule of selectedConfig.rules) {
          try {
            await api.put(`/1/indexes/${trimmed}/rules/${rule.objectID}`, rule);
          } catch { /* non-critical */ }
        }

        // Auto-seed analytics data for demo datasets so the Analytics page isn't empty
        try {
          await api.post('/2/analytics/seed', { index: trimmed, days: 30 });
          // Flush so seeded data is immediately queryable
          await api.post('/2/analytics/flush');
        } catch {
          // Non-critical — don't block index creation if seed fails
        }
        setIsLoadingData(false);
      }

      setUid('');
      setTemplate('empty');
      setError('');
      onOpenChange(false);
    } catch (err) {
      setIsLoadingData(false);
      console.error('Failed to create index:', err);
    }
  }, [uid, template, selectedConfig, createIndex, addDocuments, onOpenChange]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Enter' && !createIndex.isPending && !isLoadingData) {
        handleCreate();
      }
    },
    [handleCreate, createIndex.isPending, isLoadingData]
  );

  const handleOpenChange = useCallback(
    (open: boolean) => {
      if (!open) {
        setUid('');
        setTemplate('empty');
        setError('');
      }
      onOpenChange(open);
    },
    [onOpenChange]
  );

  const isPending = createIndex.isPending || isLoadingData;

  const buttonText = isPending
    ? createIndex.isPending
      ? 'Creating...'
      : 'Configuring & loading...'
    : template === 'empty'
    ? 'Create Index'
    : `Create & Load ${selectedConfig.defaultName}`;

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>Create Index</DialogTitle>
          <DialogDescription>
            Create a new search index to start adding documents
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="index-uid">Index Name</Label>
            <Input
              id="index-uid"
              value={uid}
              onChange={(e) => {
                setUid(e.target.value);
                setError('');
              }}
              onKeyDown={handleKeyDown}
              placeholder="e.g., products, articles, users"
              autoFocus
            />
            {error && (
              <p className="text-sm text-destructive">{error}</p>
            )}
          </div>

          <div className="space-y-2">
            <Label>Starting data</Label>
            <div className="space-y-2">
              {TEMPLATE_CONFIGS.map((opt) => (
                <label
                  key={opt.value}
                  className={cn(
                    'flex items-start gap-3 p-3 rounded-md border cursor-pointer transition-colors',
                    template === opt.value
                      ? 'border-primary bg-primary/5'
                      : 'border-border hover:border-muted-foreground/50'
                  )}
                >
                  <input
                    type="radio"
                    name="template"
                    value={opt.value}
                    checked={template === opt.value}
                    onChange={() => handleTemplateChange(opt.value)}
                    className="mt-0.5"
                  />
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-1.5">
                      {opt.icon && <opt.icon className="h-3.5 w-3.5 text-muted-foreground" />}
                      <span className="text-sm font-medium">{opt.label}</span>
                    </div>
                    <div className="text-xs text-muted-foreground mt-0.5">{opt.desc}</div>
                  </div>
                </label>
              ))}
            </div>
          </div>
        </div>

        <DialogFooter>
          <Button
            variant="outline"
            onClick={() => handleOpenChange(false)}
            disabled={isPending}
          >
            Cancel
          </Button>
          <Button onClick={handleCreate} disabled={isPending}>
            {buttonText}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
});
