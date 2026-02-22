import { memo } from 'react';
import { X } from 'lucide-react';
import { Card } from '@/components/ui/card';
import { Label } from '@/components/ui/label';
import { Badge } from '@/components/ui/badge';
import { Skeleton } from '@/components/ui/skeleton';
import { cn } from '@/lib/utils';
import type { FieldInfo } from '@/hooks/useIndexFields';

export interface SettingSectionProps {
  title: string;
  description?: string;
  warning?: string;
  warningDetail?: string;
  warningAction?: React.ReactNode;
  children: React.ReactNode;
}

export const SettingSection = memo(function SettingSection({
  title,
  description,
  warning,
  warningDetail,
  warningAction,
  children,
}: SettingSectionProps) {
  return (
    <Card className="p-6 space-y-4">
      <div>
        <div className="flex items-center gap-2 flex-wrap">
          <h3 className="text-lg font-semibold">{title}</h3>
          {warning && (
            <Badge variant="destructive" className="text-xs">
              {warning}
            </Badge>
          )}
          {warningAction}
        </div>
        {description && (
          <p className="text-sm text-muted-foreground mt-1">{description}</p>
        )}
        {warningDetail && (
          <p className="text-xs text-muted-foreground mt-2">{warningDetail}</p>
        )}
      </div>
      <div className="space-y-4">{children}</div>
    </Card>
  );
});

export interface FieldProps {
  label: string;
  description?: string;
  children: React.ReactNode;
}

export const Field = memo(function Field({ label, description, children }: FieldProps) {
  return (
    <div className="space-y-2">
      <Label>{label}</Label>
      {description && (
        <p className="text-xs text-muted-foreground">{description}</p>
      )}
      {children}
    </div>
  );
});

export interface FieldChipsProps {
  availableFields: FieldInfo[];
  selectedValues: string[];
  onToggle: (fieldName: string) => void;
  isLoading?: boolean;
}

export const FieldChips = memo(function FieldChips({
  availableFields,
  selectedValues,
  onToggle,
  isLoading,
}: FieldChipsProps) {
  if (isLoading) {
    return (
      <div className="flex gap-1.5">
        <Skeleton className="h-6 w-16 rounded-full" />
        <Skeleton className="h-6 w-20 rounded-full" />
        <Skeleton className="h-6 w-14 rounded-full" />
      </div>
    );
  }
  if (!availableFields.length) return null;

  return (
    <div className="flex flex-wrap gap-1.5">
      {availableFields.map((field) => {
        const isSelected = selectedValues.includes(field.name);
        return (
          <button
            key={field.name}
            type="button"
            onClick={() => onToggle(field.name)}
            data-testid={`attr-chip-${field.name}`}
            className={cn(
              'inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium transition-colors',
              isSelected
                ? 'bg-primary text-primary-foreground'
                : 'bg-muted text-muted-foreground hover:bg-muted/80 border border-border'
            )}
          >
            {field.name}
            {isSelected && <X className="h-3 w-3 ml-1" />}
          </button>
        );
      })}
    </div>
  );
});
