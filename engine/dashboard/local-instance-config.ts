import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const REPO_ROOT = path.resolve(__dirname, '..', '..');
const LOCAL_CONFIG_PATH = path.join(REPO_ROOT, 'flapjack.local.conf');

const DEFAULTS = {
  host: '127.0.0.1',
  backendPort: 7700,
  dashboardPort: 5177,
  adminKey: 'fj_devtestadminkey000000',
} as const;

export interface LocalInstanceConfig {
  host: string;
  backendPort: number;
  dashboardPort: number;
  adminKey: string;
  backendBaseUrl: string;
  dashboardBaseUrl: string;
  configPath: string;
  loadedFromFile: boolean;
}

export function parseLocalConfigFile(contents: string): Record<string, string> {
  const parsed: Record<string, string> = {};
  for (const rawLine of contents.split('\n')) {
    const line = rawLine.trim();
    if (!line || line.startsWith('#')) {
      continue;
    }
    const assignment = line.startsWith('export ') ? line.slice('export '.length).trim() : line;
    const equalsAt = assignment.indexOf('=');
    if (equalsAt <= 0) {
      continue;
    }
    const key = assignment.slice(0, equalsAt).trim();
    let value = assignment.slice(equalsAt + 1).trim();
    if (!key) {
      continue;
    }
    if (
      (value.startsWith('"') && value.endsWith('"')) ||
      (value.startsWith("'") && value.endsWith("'"))
    ) {
      value = value.slice(1, -1);
    } else {
      // Keep parser behavior aligned with shell `source` for simple KEY=value # comment lines.
      const commentAt = value.indexOf('#');
      if (commentAt >= 0) {
        value = value.slice(0, commentAt).trim();
      }
    }
    parsed[key] = value;
  }
  return parsed;
}

function parsePort(raw: string | undefined, fallback: number): number {
  if (!raw) {
    return fallback;
  }
  const parsed = Number(raw);
  if (!Number.isInteger(parsed) || parsed <= 0 || parsed > 65535) {
    return fallback;
  }
  return parsed;
}

function parseHttpOrigin(raw: string | undefined): string | null {
  if (!raw) {
    return null;
  }
  try {
    const parsed = new URL(raw);
    if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
      return null;
    }
    return parsed.origin;
  } catch {
    return null;
  }
}

export function getLocalInstanceConfig(): LocalInstanceConfig {
  let fileValues: Record<string, string> = {};
  let loadedFromFile = false;

  if (fs.existsSync(LOCAL_CONFIG_PATH)) {
    try {
      const contents = fs.readFileSync(LOCAL_CONFIG_PATH, 'utf8');
      fileValues = parseLocalConfigFile(contents);
      loadedFromFile = true;
    } catch {
      fileValues = {};
      loadedFromFile = false;
    }
  }

  const host = process.env.FJ_HOST || fileValues.FJ_HOST || DEFAULTS.host;
  const backendPort = parsePort(
    process.env.FJ_BACKEND_PORT || fileValues.FJ_BACKEND_PORT,
    DEFAULTS.backendPort,
  );
  const dashboardPort = parsePort(
    process.env.FJ_DASHBOARD_PORT || process.env.FLAPJACK_DASHBOARD_PORT || fileValues.FJ_DASHBOARD_PORT,
    DEFAULTS.dashboardPort,
  );
  const adminKey = process.env.FJ_TEST_ADMIN_KEY || fileValues.FJ_TEST_ADMIN_KEY || DEFAULTS.adminKey;
  const backendBaseUrl =
    parseHttpOrigin(process.env.FLAPJACK_BACKEND_URL) || `http://${host}:${backendPort}`;
  const dashboardBaseUrl = `http://${host}:${dashboardPort}`;

  return {
    host,
    backendPort,
    dashboardPort,
    adminKey,
    backendBaseUrl,
    dashboardBaseUrl,
    configPath: LOCAL_CONFIG_PATH,
    loadedFromFile,
  };
}
