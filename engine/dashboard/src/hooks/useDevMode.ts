import { create } from 'zustand';

export interface DevLogEntry {
  id: string;
  timestamp: number;
  category: string;
  message: string;
  data?: any;
}

interface DevModeStore {
  enabled: boolean;
  logs: DevLogEntry[];
  maxLogs: number;
  setEnabled: (enabled: boolean) => void;
  toggle: () => void;
  log: (category: string, message: string, data?: any) => void;
  clear: () => void;
}

export const useDevMode = create<DevModeStore>()((set, get) => ({
  enabled: new URLSearchParams(window.location.search).has('dev'),
  logs: [],
  maxLogs: 200,

  setEnabled: (enabled) => set({ enabled }),
  toggle: () => set((s) => ({ enabled: !s.enabled })),

  log: (category, message, data) => {
    if (!get().enabled) return;
    const entry: DevLogEntry = {
      id: crypto.randomUUID(),
      timestamp: Date.now(),
      category,
      message,
      data,
    };
    set((s) => ({
      logs: [entry, ...s.logs.slice(0, s.maxLogs - 1)],
    }));
  },

  clear: () => set({ logs: [] }),
}));
