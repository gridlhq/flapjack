// Index types
export interface Index {
  uid: string;
  name?: string;
  createdAt?: string;
  updatedAt?: string;
  primaryKey?: string;
  entries?: number;
  dataSize?: number;
  fileSize?: number;
  numberOfPendingTasks?: number;
}

// Search types
export interface SearchParams {
  query?: string;
  filters?: string;
  facets?: string[];
  facetFilters?: any[];
  numericFilters?: string[];
  page?: number;
  hitsPerPage?: number;
  attributesToRetrieve?: string[];
  attributesToHighlight?: string[];
  highlightPreTag?: string;
  highlightPostTag?: string;
  getRankingInfo?: boolean;
  aroundLatLng?: string;
  aroundRadius?: number | "all";
  sort?: string[];
  distinct?: boolean | number;
  analytics?: boolean;
  clickAnalytics?: boolean;
  analyticsTags?: string[];
}

export interface SearchResponse<T = any> {
  hits: T[];
  nbHits: number;
  page: number;
  nbPages: number;
  hitsPerPage: number;
  processingTimeMS: number;
  facets?: Record<string, Record<string, number>>;
  query: string;
  queryID?: string;
  index?: string;
  exhaustiveNbHits?: boolean;
}

// Document types
export interface Document {
  objectID: string;
  [key: string]: any;
}

// Settings types
export interface IndexSettings {
  searchableAttributes?: string[];
  attributesForFaceting?: string[];
  ranking?: string[];
  customRanking?: string[];
  attributesToRetrieve?: string[];
  unretrievableAttributes?: string[];
  attributesToHighlight?: string[];
  highlightPreTag?: string;
  highlightPostTag?: string;
  hitsPerPage?: number;
  removeStopWords?: boolean | string[];
  ignorePlurals?: boolean | string[];
  queryLanguages?: string[];
  queryType?: "prefixLast" | "prefixAll" | "prefixNone";
  minWordSizefor1Typo?: number;
  minWordSizefor2Typos?: number;
  distinct?: boolean | number;
  attributeForDistinct?: string;
}

// API Key types
export interface ApiKey {
  value: string;
  description?: string;
  acl: string[];
  indexes?: string[];
  expiresAt?: number;
  createdAt: number;
  updatedAt?: number;
  maxHitsPerQuery?: number;
  maxQueriesPerIPPerHour?: number;
  referers?: string[];
  queryParameters?: string;
  validity?: number;
}

// Task types
export interface Task {
  task_uid: number;
  status: "notPublished" | "published" | "error";
  type: string;
  indexUid?: string;
  received_documents?: number;
  indexed_documents?: number;
  rejected_documents?: any[];
  rejected_count?: number;
  error?: string;
  enqueuedAt?: string;
  startedAt?: string;
  finishedAt?: string;
  duration?: string;
}

// Health types
export interface HealthStatus {
  status: string;
  [key: string]: any;
}

// Synonym types (tagged union matching Algolia API)
export type SynonymType = 'synonym' | 'onewaysynonym' | 'altcorrection1' | 'altcorrection2' | 'placeholder';

export type Synonym =
  | { type: 'synonym'; objectID: string; synonyms: string[] }
  | { type: 'onewaysynonym'; objectID: string; input: string; synonyms: string[] }
  | { type: 'altcorrection1'; objectID: string; word: string; corrections: string[] }
  | { type: 'altcorrection2'; objectID: string; word: string; corrections: string[] }
  | { type: 'placeholder'; objectID: string; placeholder: string; replacements: string[] };

export interface SynonymSearchResponse {
  hits: Synonym[];
  nbHits: number;
}

// Rule types (matching Algolia Rules API)
export interface Rule {
  objectID: string;
  conditions: RuleCondition[];
  consequence: RuleConsequence;
  description?: string;
  enabled?: boolean;
  validity?: TimeRange[];
}

export interface RuleCondition {
  pattern: string;
  anchoring: 'is' | 'startsWith' | 'endsWith' | 'contains';
  alternatives?: boolean;
  context?: string;
  filters?: string;
}

export interface RuleConsequence {
  promote?: RulePromote[];
  hide?: RuleHide[];
  filterPromotes?: boolean;
  userData?: any;
  params?: { query?: string };
}

export type RulePromote =
  | { objectID: string; position: number }
  | { objectIDs: string[]; position: number };

export interface RuleHide {
  objectID: string;
}

export interface TimeRange {
  from: number;
  until: number;
}

export interface RuleSearchResponse {
  hits: Rule[];
  nbHits: number;
  page: number;
  nbPages: number;
}
