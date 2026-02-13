import { useState, useCallback } from 'react';
import { useAddDocuments } from '@/hooks/useDocuments';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Database, ShoppingBag, Loader2 } from 'lucide-react';
import { cn } from '@/lib/utils';
import moviesData from '@/data/movies.json';
import productsData from '@/data/products.json';

interface SampleDataTabContentProps {
  indexName: string;
  onSuccess: () => void;
}

type DatasetKey = 'movies' | 'products';

interface PreviewColumn {
  key: string;
  label: string;
  align?: 'right';
  hidden?: boolean;
}

interface DatasetConfig {
  key: DatasetKey;
  label: string;
  icon: typeof Database;
  description: string;
  data: Record<string, unknown>[];
  previewColumns: PreviewColumn[];
}

const DATASETS: DatasetConfig[] = [
  {
    key: 'movies',
    label: 'Movies',
    icon: Database,
    description: 'Popular movies with title, year, genre, rating, overview, and director.',
    data: moviesData,
    previewColumns: [
      { key: 'title', label: 'Title' },
      { key: 'year', label: 'Year' },
      { key: 'genre', label: 'Genre', hidden: true },
      { key: 'rating', label: 'Rating', align: 'right' },
    ],
  },
  {
    key: 'products',
    label: 'Products',
    icon: ShoppingBag,
    description: 'E-commerce items with name, brand, category, price, and rating.',
    data: productsData,
    previewColumns: [
      { key: 'name', label: 'Name' },
      { key: 'category', label: 'Category' },
      { key: 'brand', label: 'Brand', hidden: true },
      { key: 'price', label: 'Price', align: 'right' },
    ],
  },
];

function formatCellValue(value: unknown, colKey: string): string {
  if (Array.isArray(value)) return value.join(', ');
  if (typeof value === 'number' && colKey === 'price') return `$${value.toFixed(2)}`;
  return String(value ?? '');
}

export function SampleDataTabContent({ indexName, onSuccess }: SampleDataTabContentProps) {
  const [dataset, setDataset] = useState<DatasetKey>('movies');
  const [countInput, setCountInput] = useState('100');
  const addDocuments = useAddDocuments(indexName);

  const config = DATASETS.find((d) => d.key === dataset)!;
  const maxCount = config.data.length;
  const count = Math.max(1, Math.min(parseInt(countInput, 10) || 1, maxCount));
  const selected = config.data.slice(0, count);
  const preview = selected.slice(0, 5);

  const handleLoad = useCallback(async () => {
    try {
      await addDocuments.mutateAsync(selected);
      onSuccess();
    } catch {
      // error handled by the hook's onError toast
    }
  }, [selected, addDocuments, onSuccess]);

  return (
    <div className="space-y-4">
      {/* Dataset selector */}
      <div className="space-y-2">
        <Label>Dataset</Label>
        <div className="grid grid-cols-2 gap-2">
          {DATASETS.map((ds) => (
            <button
              key={ds.key}
              type="button"
              onClick={() => {
                setDataset(ds.key);
                setCountInput(String(ds.data.length));
              }}
              className={cn(
                'flex items-start gap-2.5 p-3 rounded-md border text-left transition-colors',
                dataset === ds.key
                  ? 'border-primary bg-primary/5'
                  : 'border-border hover:border-muted-foreground/50'
              )}
            >
              <ds.icon className="h-4 w-4 text-muted-foreground mt-0.5 shrink-0" />
              <div className="min-w-0">
                <p className="text-sm font-medium">{ds.label}</p>
                <p className="text-xs text-muted-foreground mt-0.5 line-clamp-2">{ds.description}</p>
              </div>
            </button>
          ))}
        </div>
      </div>

      {/* Count input */}
      <div className="space-y-1.5">
        <Label htmlFor="sample-count">Number of documents</Label>
        <div className="flex items-center gap-2">
          <Input
            id="sample-count"
            type="number"
            min={1}
            max={maxCount}
            value={countInput}
            onChange={(e) => setCountInput(e.target.value)}
            className="w-24"
          />
          <span className="text-xs text-muted-foreground">of {maxCount} available</span>
        </div>
      </div>

      {/* Preview table */}
      <div className="border rounded-md overflow-hidden">
        <table className="w-full text-sm">
          <thead>
            <tr className="bg-muted/50">
              {config.previewColumns.map((col) => (
                <th
                  key={col.key}
                  className={cn(
                    'text-left p-2 font-medium text-muted-foreground',
                    col.align === 'right' && 'text-right w-16',
                    col.hidden && 'hidden sm:table-cell'
                  )}
                >
                  {col.label}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {preview.map((row, i) => (
              <tr key={i} className="border-t">
                {config.previewColumns.map((col) => (
                  <td
                    key={col.key}
                    className={cn(
                      'p-2 truncate max-w-[200px]',
                      col.align === 'right' && 'text-right',
                      col.hidden && 'hidden sm:table-cell text-muted-foreground'
                    )}
                  >
                    {formatCellValue((row as Record<string, unknown>)[col.key], col.key)}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
        {count > 5 && (
          <div className="p-2 text-xs text-muted-foreground bg-muted/30 border-t">
            Showing 5 of {count} selected
          </div>
        )}
      </div>

      {/* Load button */}
      <Button
        onClick={handleLoad}
        disabled={addDocuments.isPending}
        className="w-full"
      >
        {addDocuments.isPending ? (
          <>
            <Loader2 className="h-4 w-4 mr-2 animate-spin" />
            Loading...
          </>
        ) : (
          <>
            <config.icon className="h-4 w-4 mr-2" />
            Load {count} {config.label}
          </>
        )}
      </Button>
    </div>
  );
}
