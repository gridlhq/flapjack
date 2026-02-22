import { useState } from 'react';
import { Outlet } from 'react-router-dom';
import { Header } from './Header';
import { Sidebar } from './Sidebar';
import { ApiLogger } from './ApiLogger';
import { DevModePanel } from './DevModePanel';
import { useHealth } from '@/hooks/useHealth';
import { AlertTriangle } from 'lucide-react';

export function Layout() {
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const health = useHealth();
  const isDisconnected = health.isError || (health.data?.status !== 'ok' && !health.isLoading);

  return (
    <div className="flex h-screen flex-col">
      <Header onMenuToggle={() => setSidebarOpen((o) => !o)} />
      {isDisconnected && (
        <div className="bg-red-600 text-white px-4 py-2 flex items-center justify-center gap-2 text-sm font-medium" data-testid="disconnected-banner">
          <AlertTriangle className="h-4 w-4 shrink-0" />
          <span>Server disconnected — check that Flapjack is running on {import.meta.env.DEV ? new URL(__BACKEND_URL__).host : window.location.host}</span>
        </div>
      )}
      <DevModePanel />
      <div className="flex flex-1 overflow-hidden">
        <Sidebar open={sidebarOpen} onClose={() => setSidebarOpen(false)} />
        <main className="flex-1 overflow-auto p-6 bg-muted/30">
          <Outlet />
        </main>
      </div>
      <ApiLogger />
    </div>
  );
}
