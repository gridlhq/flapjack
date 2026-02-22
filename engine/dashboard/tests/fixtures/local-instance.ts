import { getLocalInstanceConfig } from '../../local-instance-config';

const instance = getLocalInstanceConfig();

export const API_BASE = instance.backendBaseUrl;
export const DASHBOARD_BASE = instance.dashboardBaseUrl;
export const TEST_ADMIN_KEY = instance.adminKey;

export const API_HEADERS = {
  'x-algolia-application-id': 'flapjack',
  'x-algolia-api-key': TEST_ADMIN_KEY,
  'Content-Type': 'application/json',
} as const;
