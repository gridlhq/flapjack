import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export interface ApiLogEntry {
  id: string;
  timestamp: number;
  method: string;
  url: string;
  headers: Record<string, string>;
  body?: any;
  response?: any;
  duration: number;
  status: 'pending' | 'success' | 'error';
}

interface ApiLoggerStore {
  entries: ApiLogEntry[];
  maxEntries: number;
  isExpanded: boolean;
  addEntry: (entry: Omit<ApiLogEntry, 'id' | 'timestamp'>) => string;
  updateEntry: (id: string, updates: Partial<ApiLogEntry>) => void;
  clear: () => void;
  toggleExpanded: () => void;
  exportAsBash: () => string;
  exportAsFile: () => void;
}

export const useApiLogger = create<ApiLoggerStore>()(
  persist(
    (set, get) => ({
      entries: [],
      maxEntries: 20,
      isExpanded: false,

      addEntry: (entry) => {
        const id = crypto.randomUUID();
        const timestamp = Date.now();
        set((state) => ({
          entries: [
            { ...entry, id, timestamp },
            ...state.entries.slice(0, state.maxEntries - 1),
          ],
        }));
        return id;
      },

      updateEntry: (id, updates) => {
        set((state) => ({
          entries: state.entries.map((e) =>
            e.id === id ? { ...e, ...updates } : e
          ),
        }));
      },

      clear: () => set({ entries: [] }),

      toggleExpanded: () => set((state) => ({ isExpanded: !state.isExpanded })),

      exportAsBash: () => {
        const { entries } = get();
        const timestamp = new Date().toISOString();
        const header = `#!/bin/bash\n# Flapjack API Requests - ${timestamp}\n\n`;

        const commands = entries
          .slice()
          .reverse()
          .map((e, i) => {
            const headers = Object.entries(e.headers)
              .filter(([k]) => k !== 'x-request-id') // Exclude internal header
              .map(([k, v]) => `  -H "${k}: ${v}"`)
              .join(' \\\n');
            const body = e.body ? ` \\\n  -d '${JSON.stringify(e.body)}'` : '';
            const fullUrl = e.url.startsWith('http') ? e.url : `http://localhost:7700${e.url}`;
            return `# ${i + 1}. ${e.method} ${e.url}\ncurl -X ${e.method} ${fullUrl} \\\n${headers}${body}\n`;
          })
          .join('\n');

        return header + commands;
      },

      exportAsFile: () => {
        const bash = get().exportAsBash();
        const blob = new Blob([bash], { type: 'text/plain' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = `flapjack-api-log-${Date.now()}.sh`;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(url);
      },
    }),
    {
      name: 'flapjack-api-log',
      storage: {
        getItem: (name) => {
          const value = sessionStorage.getItem(name);
          return value ? JSON.parse(value) : null;
        },
        setItem: (name, value) => {
          sessionStorage.setItem(name, JSON.stringify(value));
        },
        removeItem: (name) => {
          sessionStorage.removeItem(name);
        },
      },
    }
  )
);
