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
        set({ apiKey: key });
      },
      setAppId: (id: string) => {
        set({ appId: id });
      },
      clearAuth: () => {
        set({ apiKey: null, appId: 'flapjack' });
      },
    }),
    {
      name: 'flapjack-auth',
    }
  )
);
