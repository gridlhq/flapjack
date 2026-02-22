/**
 * Analytics data seeding helpers for integration tests.
 * 
 * These helpers create real analytics data by:
 * 1. Creating an index
 * 2. Seeding documents
 * 3. Executing searches with analytics enabled
 * 4. Flushing analytics to storage
 */

import type { APIRequestContext } from '@playwright/test';
import { API_BASE as API, API_HEADERS as HEADERS } from './local-instance';

export interface AnalyticsSeedConfig {
  /** Index name to seed */
  indexName: string;
  /** Number of documents to add */
  documentCount: number;
  /** Number of searches to execute */
  searchCount: number;
  /** Percentage of searches that return no results (0-1) */
  noResultRate: number;
  /** Device distribution { desktop, mobile, tablet } percentages (should sum to 1) */
  deviceDistribution: { desktop: number; mobile: number; tablet: number };
  /** Country distribution (should sum to 1) */
  countryDistribution: Record<string, number>;
}

/** Default configuration for analytics seeding */
export const DEFAULT_ANALYTICS_CONFIG: AnalyticsSeedConfig = {
  indexName: 'analytics-test',
  documentCount: 100,
  searchCount: 500,
  noResultRate: 0.05,
  deviceDistribution: { desktop: 0.6, mobile: 0.3, tablet: 0.1 },
  countryDistribution: { US: 0.45, GB: 0.2, DE: 0.15, CA: 0.1, FR: 0.1 },
};

const PRODUCTS = [
  { objectID: 'p01', name: 'MacBook Pro 16"', category: 'Laptops', brand: 'Apple', price: 3499 },
  { objectID: 'p02', name: 'ThinkPad X1 Carbon', category: 'Laptops', brand: 'Lenovo', price: 1849 },
  { objectID: 'p03', name: 'Dell XPS 15', category: 'Laptops', brand: 'Dell', price: 2499 },
  { objectID: 'p04', name: 'iPad Pro 12.9"', category: 'Tablets', brand: 'Apple', price: 1099 },
  { objectID: 'p05', name: 'Galaxy Tab S9', category: 'Tablets', brand: 'Samsung', price: 1199 },
  { objectID: 'p06', name: 'Sony WH-1000XM5', category: 'Audio', brand: 'Sony', price: 349 },
  { objectID: 'p07', name: 'AirPods Pro 2', category: 'Audio', brand: 'Apple', price: 249 },
  { objectID: 'p08', name: 'Samsung 990 Pro 2TB', category: 'Storage', brand: 'Samsung', price: 179 },
  { objectID: 'p09', name: 'LG UltraGear 27" 4K', category: 'Monitors', brand: 'LG', price: 699 },
  { objectID: 'p10', name: 'Logitech MX Master 3S', category: 'Accessories', brand: 'Logitech', price: 99 },
];

const SEARCH_TERMS = [
  { query: 'laptop', hasResults: true, weight: 50 },
  { query: 'macbook', hasResults: true, weight: 40 },
  { query: 'tablet', hasResults: true, weight: 35 },
  { query: 'headphones', hasResults: true, weight: 30 },
  { query: 'dell', hasResults: true, weight: 25 },
  { query: 'apple', hasResults: true, weight: 45 },
  { query: 'samsung', hasResults: true, weight: 20 },
  { query: 'monitor', hasResults: true, weight: 15 },
  { query: 'keyboard', hasResults: true, weight: 12 },
  { query: 'mouse', hasResults: true, weight: 10 },
  // No-result queries
  { query: 'unicorn widget', hasResults: false, weight: 5 },
  { query: 'nonexistent product', hasResults: false, weight: 3 },
  { query: 'xyzzy123', hasResults: false, weight: 2 },
  { query: 'discontinued item', hasResults: false, weight: 4 },
];

/**
 * Seeds analytics data for testing.
 * Uses the backend's built-in seed endpoint which generates realistic data
 * including geography, devices, searches, clicks, and conversions.
 */
export async function seedAnalytics(
  request: APIRequestContext,
  config: AnalyticsSeedConfig = DEFAULT_ANALYTICS_CONFIG,
): Promise<void> {
  const { indexName, documentCount } = config;

  // 1. Create index and add documents (needed for searches to work)
  const documents = PRODUCTS.slice(0, Math.min(documentCount, PRODUCTS.length));
  await request.post(`${API}/1/indexes/${indexName}/batch`, {
    headers: HEADERS,
    data: {
      requests: documents.map((doc) => ({ action: 'addObject', body: doc })),
    },
  });

  // Wait for indexing to complete
  await new Promise((resolve) => setTimeout(resolve, 2000));

  // 2. Seed analytics using backend's built-in generator
  // This creates realistic analytics with geography, devices, searches, clicks
  await request.post(`${API}/2/analytics/seed`, {
    headers: HEADERS,
    data: {
      index: indexName,
      days: 7, // Generate 7 days of analytics data
    },
  });

  // Wait for analytics data to be available
  // Seed creates data for the past 7 days (NOT including today)
  await new Promise((resolve) => setTimeout(resolve, 3000));

  // Verify data was created by checking the /2/overview endpoint
  // Use yesterday as end date since seed doesn't create data for today
  const yesterday = new Date(Date.now() - 24 * 60 * 60 * 1000);
  const eightDaysAgo = new Date(Date.now() - 8 * 24 * 60 * 60 * 1000);

  try {
    const response = await request.get(`${API}/2/overview`, {
      headers: HEADERS,
      params: {
        index: indexName,
        startDate: eightDaysAgo.toISOString().split('T')[0],
        endDate: yesterday.toISOString().split('T')[0],
      },
    });

    if (!response.ok()) {
      console.warn(`Analytics seed verification failed: ${response.status()}`);
    } else {
      const data = await response.json();
      if (!data.totalSearches || data.totalSearches === 0) {
        console.warn('Analytics seed created no data');
      }
    }
  } catch (error) {
    console.warn('Analytics seed verification warning:', error);
  }
}

/**
 * Deletes analytics data for an index.
 */
export async function clearAnalytics(
  request: APIRequestContext,
  indexName: string,
): Promise<void> {
  await request.delete(`${API}/2/analytics/clear`, {
    params: { index: indexName },
    headers: HEADERS,
  });
}

/**
 * Deletes an index (cleanup).
 */
export async function deleteIndex(
  request: APIRequestContext,
  indexName: string,
): Promise<void> {
  await request.delete(`${API}/1/indexes/${indexName}`, {
    headers: HEADERS,
  });
}

/**
 * Generates a list of searches based on config.
 */
function generateSearches(
  count: number,
  config: AnalyticsSeedConfig,
): Array<{
  query: string;
  userToken: string;
  ip: string;
  userAgent: string;
  tags: string[];
}> {
  const searches: ReturnType<typeof generateSearches> = [];
  
  // Calculate how many searches per term
  const totalWeight = SEARCH_TERMS.reduce((sum, term) => sum + term.weight, 0);
  
  for (const term of SEARCH_TERMS) {
    const termCount = Math.round((term.weight / totalWeight) * count);
    
    for (let i = 0; i < termCount; i++) {
      // Randomly assign device
      const deviceRand = Math.random();
      let device = 'desktop';
      if (deviceRand < config.deviceDistribution.mobile + config.deviceDistribution.tablet) {
        device = deviceRand < config.deviceDistribution.mobile ? 'mobile' : 'tablet';
      }
      
      // Randomly assign country
      const countryRand = Math.random();
      let country = 'US';
      let sum = 0;
      for (const [code, prob] of Object.entries(config.countryDistribution)) {
        sum += prob;
        if (countryRand < sum) {
          country = code;
          break;
        }
      }
      
      searches.push({
        query: term.query,
        userToken: `user-${Math.floor(Math.random() * 50) + 1}`, // 50 unique users
        ip: getIPForCountry(country),
        userAgent: getUserAgentForDevice(device),
        tags: [`platform:${device}`, `country:${country}`],
      });
    }
  }
  
  return searches;
}

function getIPForCountry(country: string): string {
  const ips: Record<string, string> = {
    US: '8.8.8.8',
    GB: '81.2.69.142',
    DE: '46.114.0.0',
    CA: '24.200.0.0',
    FR: '2.0.0.0',
  };
  return ips[country] || '8.8.8.8';
}

function getUserAgentForDevice(device: string): string {
  const agents: Record<string, string> = {
    desktop: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36',
    mobile: 'Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X) AppleWebKit/605.1.15',
    tablet: 'Mozilla/5.0 (iPad; CPU OS 16_0 like Mac OS X) AppleWebKit/605.1.15',
  };
  return agents[device] || agents.desktop;
}
