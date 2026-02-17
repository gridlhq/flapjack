/**
 * Shared test data for Algolia migration E2E tests.
 * Mirrors the data used in sdk_test/test_algolia_migration.js.
 */

export const PRODUCTS = [
  { objectID: 'p01', name: 'MacBook Pro 16"', description: 'Apple M3 Max chip laptop', brand: 'Apple', category: 'Laptops', price: 3499, rating: 4.8, inStock: true, tags: ['laptop', 'professional'] },
  { objectID: 'p02', name: 'ThinkPad X1 Carbon', description: 'Lightweight business laptop', brand: 'Lenovo', category: 'Laptops', price: 1849, rating: 4.6, inStock: true, tags: ['laptop', 'business'] },
  { objectID: 'p03', name: 'Dell XPS 15', description: 'Creative laptop with OLED display', brand: 'Dell', category: 'Laptops', price: 2499, rating: 4.5, inStock: true, tags: ['laptop', 'creative'] },
  { objectID: 'p04', name: 'iPad Pro 12.9"', description: 'M2 chip tablet by Apple', brand: 'Apple', category: 'Tablets', price: 1099, rating: 4.7, inStock: true, tags: ['tablet', 'professional'] },
  { objectID: 'p05', name: 'Galaxy Tab S9', description: 'Samsung premium Android tablet', brand: 'Samsung', category: 'Tablets', price: 1199, rating: 4.4, inStock: false, tags: ['tablet', 'android'] },
  { objectID: 'p06', name: 'Sony WH-1000XM5', description: 'Wireless noise canceling headphones', brand: 'Sony', category: 'Audio', price: 349, rating: 4.7, inStock: true, tags: ['headphones', 'wireless'] },
  { objectID: 'p07', name: 'AirPods Pro 2', description: 'Apple wireless earbuds with ANC', brand: 'Apple', category: 'Audio', price: 249, rating: 4.6, inStock: true, tags: ['earbuds', 'wireless'] },
  { objectID: 'p08', name: 'Samsung 990 Pro 2TB', description: 'NVMe M.2 SSD storage', brand: 'Samsung', category: 'Storage', price: 179, rating: 4.8, inStock: true, tags: ['ssd', 'storage'] },
  { objectID: 'p09', name: 'LG UltraGear 27" 4K', description: '144Hz gaming monitor', brand: 'LG', category: 'Monitors', price: 699, rating: 4.5, inStock: true, tags: ['monitor', 'gaming'] },
  { objectID: 'p10', name: 'Logitech MX Master 3S', description: 'Wireless ergonomic mouse', brand: 'Logitech', category: 'Accessories', price: 99, rating: 4.7, inStock: true, tags: ['mouse', 'wireless'] },
  { objectID: 'p11', name: 'Keychron Q1 Pro', description: 'Wireless mechanical keyboard', brand: 'Keychron', category: 'Accessories', price: 199, rating: 4.6, inStock: true, tags: ['keyboard', 'wireless'] },
  { objectID: 'p12', name: 'CalDigit TS4', description: 'Thunderbolt 4 dock with 18 ports', brand: 'CalDigit', category: 'Accessories', price: 399, rating: 4.8, inStock: false, tags: ['dock', 'thunderbolt'] },
];

export const SYNONYMS = [
  { objectID: 'syn-laptop-notebook', type: 'synonym' as const, synonyms: ['laptop', 'notebook', 'computer'] },
  { objectID: 'syn-phone-mobile', type: 'synonym' as const, synonyms: ['headphones', 'earphones', 'earbuds'] },
  { objectID: 'syn-screen-display', type: 'synonym' as const, synonyms: ['monitor', 'screen', 'display'] },
];

export const RULES = [
  {
    objectID: 'rule-pin-macbook',
    conditions: [{ pattern: 'laptop', anchoring: 'contains' }],
    consequence: { promote: [{ objectID: 'p01', position: 0 }] },
    description: 'Pin MacBook Pro to top when searching laptop',
  },
  {
    objectID: 'rule-hide-galaxy-tab',
    conditions: [{ pattern: 'tablet', anchoring: 'contains' }],
    consequence: { hide: [{ objectID: 'p05' }] },
    description: 'Hide Galaxy Tab S9 when searching tablet',
  },
];

export const SETTINGS = {
  searchableAttributes: ['name', 'description', 'brand', 'category', 'tags'],
  attributesForFaceting: ['category', 'brand', 'filterOnly(price)', 'filterOnly(inStock)'],
  customRanking: ['desc(rating)', 'asc(price)'],
};

/** Expected counts for verifying the migration success card */
export const EXPECTED_COUNTS = {
  documents: PRODUCTS.length,
  synonyms: SYNONYMS.length,
  rules: RULES.length,
};
