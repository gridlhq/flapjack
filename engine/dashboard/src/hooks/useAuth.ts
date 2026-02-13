import { create } from 'zustand';
import { persist } from 'zustand/middleware';

interface AuthStore {
  apiKey: string | null;
  appId: string;
  setApiKey: (key: string) => void;
  setAppId: (id: string) => void;
  clearAuth: () => void;
}

export const useAuth = create<AuthStore>()(
  persist(
    (set) => ({
      apiKey: null,
      appId: 'flapjack',
      setApiKey: (key: string) => {
        localStorage.setItem('flapjack-api-key', key);
        set({ apiKey: key });
      },
      setAppId: (id: string) => {
        localStorage.setItem('flapjack-app-id', id);
        set({ appId: id });
      },
      clearAuth: () => {
        localStorage.removeItem('flapjack-api-key');
        localStorage.removeItem('flapjack-app-id');
        set({ apiKey: null, appId: 'flapjack' });
      },
    }),
    {
      name: 'flapjack-auth',
    }
  )
);
