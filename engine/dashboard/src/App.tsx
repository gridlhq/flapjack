import { lazy, Suspense } from 'react';
import { Routes, Route } from 'react-router-dom';
import { useTheme } from './hooks/useTheme';
import { Layout } from './components/layout/Layout';
import { AuthGate } from './components/layout/AuthGate';
import { ErrorBoundary } from './components/ErrorBoundary';
import { Toaster } from './components/ui/toaster';

// Lazy-load all route pages to keep initial bundle small.
// Each page (+ its deps like recharts) loads on demand.
const Overview = lazy(() => import('./pages/Overview').then(m => ({ default: m.Overview })));
const SearchBrowse = lazy(() => import('./pages/SearchBrowse').then(m => ({ default: m.SearchBrowse })));
const Settings = lazy(() => import('./pages/Settings').then(m => ({ default: m.Settings })));
const Analytics = lazy(() => import('./pages/Analytics').then(m => ({ default: m.Analytics })));
const Synonyms = lazy(() => import('./pages/Synonyms').then(m => ({ default: m.Synonyms })));
const Rules = lazy(() => import('./pages/Rules').then(m => ({ default: m.Rules })));
const MerchandisingStudio = lazy(() => import('./pages/MerchandisingStudio').then(m => ({ default: m.MerchandisingStudio })));
const ApiKeys = lazy(() => import('./pages/ApiKeys').then(m => ({ default: m.ApiKeys })));
const SearchLogs = lazy(() => import('./pages/SearchLogs').then(m => ({ default: m.SearchLogs })));
const System = lazy(() => import('./pages/System').then(m => ({ default: m.System })));
const Metrics = lazy(() => import('./pages/Metrics').then(m => ({ default: m.Metrics })));
const Migrate = lazy(() => import('./pages/Migrate').then(m => ({ default: m.Migrate })));
const QuerySuggestions = lazy(() => import('./pages/QuerySuggestions').then(m => ({ default: m.QuerySuggestions })));
const Experiments = lazy(() => import('./pages/Experiments').then(m => ({ default: m.Experiments })));
const ExperimentDetail = lazy(() => import('./pages/ExperimentDetail').then(m => ({ default: m.ExperimentDetail })));

function LazyPage({ children }: { children: React.ReactNode }) {
  return (
    <ErrorBoundary>
      <Suspense fallback={<div className="p-6 animate-pulse">Loading...</div>}>
        {children}
      </Suspense>
    </ErrorBoundary>
  );
}

function App() {
  // Initialize theme
  useTheme();

  return (
    <AuthGate>
      <Routes>
        <Route path="/" element={<Layout />}>
          <Route index element={<LazyPage><Overview /></LazyPage>} />
          <Route path="overview" element={<LazyPage><Overview /></LazyPage>} />
          <Route path="index/:indexName" element={<LazyPage><SearchBrowse /></LazyPage>} />
          <Route path="index/:indexName/settings" element={<LazyPage><Settings /></LazyPage>} />
          <Route path="index/:indexName/analytics" element={<LazyPage><Analytics /></LazyPage>} />
          <Route path="index/:indexName/synonyms" element={<LazyPage><Synonyms /></LazyPage>} />
          <Route path="index/:indexName/rules" element={<LazyPage><Rules /></LazyPage>} />
          <Route path="index/:indexName/merchandising" element={<LazyPage><MerchandisingStudio /></LazyPage>} />
          <Route path="keys" element={<LazyPage><ApiKeys /></LazyPage>} />
          <Route path="logs" element={<LazyPage><SearchLogs /></LazyPage>} />
          <Route path="migrate" element={<LazyPage><Migrate /></LazyPage>} />
          <Route path="metrics" element={<LazyPage><Metrics /></LazyPage>} />
          <Route path="system" element={<LazyPage><System /></LazyPage>} />
          <Route path="query-suggestions" element={<LazyPage><QuerySuggestions /></LazyPage>} />
          <Route path="experiments" element={<LazyPage><Experiments /></LazyPage>} />
          <Route path="experiments/:experimentId" element={<LazyPage><ExperimentDetail /></LazyPage>} />
          <Route path="*" element={<div className="p-6">Page not found</div>} />
        </Route>
      </Routes>
      <Toaster />
    </AuthGate>
  );
}

export default App;
