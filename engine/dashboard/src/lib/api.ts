import axios, { InternalAxiosRequestConfig } from 'axios';
import { useApiLogger } from '@/hooks/useApiLogger';

// Extend Axios config to include metadata
declare module 'axios' {
  export interface InternalAxiosRequestConfig {
    metadata?: {
      startTime: number;
      entryId: string;
    };
  }
}

const api = axios.create({
  // In dev, use empty baseURL so requests go through Vite proxy (avoids CORS).
  // In production, empty baseURL means relative to the serving origin.
  baseURL: '',
  headers: {
    'Content-Type': 'application/json',
  },
});

// Request interceptor - add auth headers and log request
api.interceptors.request.use((config: InternalAxiosRequestConfig) => {
  const apiKey = localStorage.getItem('flapjack-api-key');
  const appId = localStorage.getItem('flapjack-app-id') || 'flapjack';

  // Always send app-id; send api-key if configured
  config.headers['x-algolia-application-id'] = appId;
  if (apiKey) {
    config.headers['x-algolia-api-key'] = apiKey;
  }

  // Add to logger
  const logger = useApiLogger.getState();
  const entryId = crypto.randomUUID();
  config.headers['x-request-id'] = entryId;

  logger.addEntry({
    method: config.method?.toUpperCase() || 'GET',
    url: config.url || '',
    headers: config.headers as Record<string, string>,
    body: config.data,
    status: 'pending',
    duration: 0,
  });

  config.metadata = { startTime: Date.now(), entryId };
  return config;
});

// Response interceptor - log success/error
api.interceptors.response.use(
  (response) => {
    const { startTime, entryId } = response.config.metadata || {};
    if (startTime && entryId) {
      const duration = Date.now() - startTime;

      useApiLogger.getState().updateEntry(entryId, {
        status: 'success',
        duration,
        response: response.data,
      });
    }

    return response;
  },
  (error) => {
    const { startTime, entryId } = error.config?.metadata || {};
    if (startTime && entryId) {
      const duration = Date.now() - startTime;

      useApiLogger.getState().updateEntry(entryId, {
        status: 'error',
        duration,
        response: error.response?.data || { error: error.message },
      });
    }

    return Promise.reject(error);
  }
);

export default api;
