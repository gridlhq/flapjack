/// <reference types="vite/client" />

/** Injected by vite.config.ts `define` — the backend server URL (e.g. "http://localhost:7701") */
declare const __BACKEND_URL__: string;

interface ImportMetaEnv {
  readonly DEV: boolean;
  readonly PROD: boolean;
  readonly MODE: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
