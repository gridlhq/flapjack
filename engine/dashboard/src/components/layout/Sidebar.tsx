import { NavLink, useLocation } from 'react-router-dom';
import { Home, Key, Activity, ArrowRightLeft, ScrollText, X, Database, ChevronDown, ChevronRight } from 'lucide-react';
import { cn } from '@/lib/utils';
import { useEffect, useState } from 'react';
import { useIndices } from '@/hooks/useIndices';
import { InfoTooltip } from '@/components/ui/info-tooltip';

interface SidebarProps {
  open?: boolean;
  onClose?: () => void;
}

const navItems = [
  { to: '/overview', icon: Home, label: 'Overview' },
  { to: '/logs', icon: ScrollText, label: 'API Logs' },
  { to: '/migrate', icon: ArrowRightLeft, label: 'Migrate' },
  { to: '/keys', icon: Key, label: 'API Keys' },
  { to: '/system', icon: Activity, label: 'System' },
];

const MAX_VISIBLE_INDICES = 5;

export function Sidebar({ open, onClose }: SidebarProps) {
  const location = useLocation();
  const { data: indices } = useIndices();
  const [showAllIndices, setShowAllIndices] = useState(false);

  // Close sidebar on route change (mobile)
  useEffect(() => {
    onClose?.();
  }, [location.pathname]); // eslint-disable-line react-hooks/exhaustive-deps

  const visibleIndices = showAllIndices
    ? indices
    : indices?.slice(0, MAX_VISIBLE_INDICES);

  const hasMoreIndices = (indices?.length || 0) > MAX_VISIBLE_INDICES;

  return (
    <>
      {/* Mobile overlay */}
      {open && (
        <div
          className="fixed inset-0 z-40 bg-black/50 md:hidden"
          onClick={onClose}
        />
      )}

      {/* Sidebar */}
      <aside
        className={cn(
          'border-r border-border bg-muted/20 p-4 z-50',
          // Desktop: always visible, static
          'hidden md:block w-64',
          // Mobile: slide-in overlay
          open && 'fixed inset-y-0 left-0 block w-64 md:relative md:inset-auto'
        )}
      >
        {/* Mobile close button */}
        <div className="flex items-center justify-between mb-4 md:hidden">
          <span className="text-sm font-semibold text-muted-foreground">Navigation</span>
          <button
            onClick={onClose}
            className="p-1 rounded-md hover:bg-accent"
            aria-label="Close navigation"
          >
            <X className="h-5 w-5" />
          </button>
        </div>

        <nav className="space-y-2">
          {navItems.map((item) => (
            <NavLink
              key={item.to}
              to={item.to}
              className={({ isActive }) =>
                cn(
                  'flex items-center gap-3 px-4 py-2 rounded-md text-sm font-medium transition-colors',
                  isActive
                    ? 'bg-primary/15 text-primary font-semibold'
                    : 'text-muted-foreground hover:bg-accent hover:text-accent-foreground'
                )
              }
            >
              <item.icon className="h-5 w-5" />
              {item.label}
            </NavLink>
          ))}
        </nav>

        {/* Indices section */}
        {indices && indices.length > 0 && (
          <div className="mt-6" data-testid="sidebar-indices">
            <div className="px-4 mb-2 text-xs font-semibold text-muted-foreground uppercase tracking-wider flex items-center gap-1.5" data-testid="sidebar-indices-header">
              Indices
              <InfoTooltip content="Each index is an isolated search collection with its own data, settings, and access controls." side="right" />
            </div>
            <div className="space-y-1">
              {visibleIndices?.map((index) => {
                const indexPath = `/index/${encodeURIComponent(index.uid)}`;
                const isActive = location.pathname.startsWith(indexPath);
                return (
                  <NavLink
                    key={index.uid}
                    to={indexPath}
                    className={cn(
                      'flex items-center gap-3 px-4 py-1.5 rounded-md text-sm transition-colors',
                      isActive
                        ? 'bg-primary/15 text-primary font-semibold'
                        : 'text-muted-foreground hover:bg-accent hover:text-accent-foreground'
                    )}
                    data-testid={`sidebar-index-${index.uid}`}
                  >
                    <Database className="h-4 w-4 shrink-0" />
                    <span className="truncate">{index.uid}</span>
                  </NavLink>
                );
              })}
              {hasMoreIndices && (
                <button
                  onClick={() => setShowAllIndices(!showAllIndices)}
                  className="flex items-center gap-3 px-4 py-1.5 rounded-md text-xs text-muted-foreground hover:bg-accent hover:text-accent-foreground transition-colors w-full"
                  data-testid="sidebar-show-all-indices"
                >
                  {showAllIndices ? (
                    <>
                      <ChevronDown className="h-3 w-3" />
                      Show less
                    </>
                  ) : (
                    <>
                      <ChevronRight className="h-3 w-3" />
                      Show all ({indices.length})
                    </>
                  )}
                </button>
              )}
            </div>
          </div>
        )}
      </aside>
    </>
  );
}
