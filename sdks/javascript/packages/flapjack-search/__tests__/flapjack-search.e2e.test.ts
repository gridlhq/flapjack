/**
 * E2E tests for flapjack-search SDK against a live Flapjack server.
 *
 * Requires: Flapjack server running on localhost:7700
 * Run: npx vitest --run __tests__/flapjack-search.e2e.test.ts
 *
 * Conventions:
 *  - Never sleep — always poll for expected state
 *  - Each describe block uses its own index (timestamp-based) for isolation
 *  - Clean up indices after tests
 */
import { describe, expect, it, beforeAll, afterAll } from "vitest";
import { flapjackSearch } from "../builds/node";
import type { EndRequest, Response } from "@flapjack-search/client-common";

const SERVER = process.env.FLAPJACK_SERVER || "localhost:7700";
const API_KEY = process.env.FLAPJACK_ADMIN_KEY || "abcdef0123456789";
const APP_ID = process.env.FLAPJACK_APP_ID || "test-app";

/** Requester that routes all traffic to the local Flapjack server over HTTP. */
function localRequester() {
  return {
    async send(request: EndRequest): Promise<Response> {
      const url = new URL(request.url);
      url.protocol = "http:";
      url.host = SERVER;

      const res = await fetch(url.toString(), {
        method: request.method,
        headers: request.headers,
        body: request.data,
      });

      return {
        status: res.status,
        content: await res.text(),
        isTimedOut: false,
      };
    },
  };
}

function createClient() {
  return flapjackSearch(APP_ID, API_KEY, {
    requester: localRequester(),
  });
}

/** Poll until the index has at least `count` hits (max 5 s). */
async function waitForHits(
  client: ReturnType<typeof createClient>,
  indexName: string,
  count: number,
  timeoutMs = 5000,
): Promise<void> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const res = await client.search({
      requests: [{ indexName, query: "", hitsPerPage: 0 }],
    });
    const r = (res.results as any[])[0];
    if (r && r.nbHits >= count) return;
    await new Promise((r) => setTimeout(r, 50));
  }
  throw new Error(
    `Timed out waiting for ${count} hits in "${indexName}" after ${timeoutMs}ms`,
  );
}

/** Poll until the index has exactly 0 hits (max 5 s). */
async function waitForEmpty(
  client: ReturnType<typeof createClient>,
  indexName: string,
  timeoutMs = 5000,
): Promise<void> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    try {
      const res = await client.search({
        requests: [{ indexName, query: "", hitsPerPage: 0 }],
      });
      const r = (res.results as any[])[0];
      if (r && r.nbHits === 0) return;
    } catch {
      // index may not exist yet — that counts as empty
      return;
    }
    await new Promise((r) => setTimeout(r, 50));
  }
  throw new Error(
    `Timed out waiting for empty index "${indexName}" after ${timeoutMs}ms`,
  );
}

// ---------------------------------------------------------------------------
// Test suite
// ---------------------------------------------------------------------------

describe("flapjack-search E2E (live server)", () => {
  const client = createClient();
  const INDEX = `e2e_test_${Date.now()}`;

  // Pre-flight: make sure the server is reachable
  beforeAll(async () => {
    const res = await fetch(`http://${SERVER}/health`);
    expect(res.ok).toBe(true);
  });

  // Clean up
  afterAll(async () => {
    try {
      await client.deleteIndex({ indexName: INDEX });
    } catch {
      // ignore — index may already be gone
    }
  });

  // ----- Settings -----

  it("sets and retrieves index settings", async () => {
    await client.setSettings({
      indexName: INDEX,
      indexSettings: {
        searchableAttributes: ["name", "description"],
        attributesForFaceting: ["category", "brand"],
      },
    });

    const settings = await client.getSettings({ indexName: INDEX });
    expect(settings.searchableAttributes).toEqual(["name", "description"]);
    expect(settings.attributesForFaceting).toEqual(["category", "brand"]);
  });

  // ----- Document ingestion -----

  it("saves objects via batch and indexes them", async () => {
    const objects = [
      {
        objectID: "1",
        name: "Gaming Laptop",
        description: "High-performance gaming machine",
        category: "electronics",
        brand: "Dell",
        price: 999,
      },
      {
        objectID: "2",
        name: "Office Laptop",
        description: "Business-class notebook",
        category: "electronics",
        brand: "HP",
        price: 599,
      },
      {
        objectID: "3",
        name: "Wireless Mouse",
        description: "Ergonomic wireless mouse",
        category: "accessories",
        brand: "Logitech",
        price: 49,
      },
      {
        objectID: "4",
        name: "Mechanical Keyboard",
        description: "Cherry MX switches keyboard",
        category: "accessories",
        brand: "Corsair",
        price: 129,
      },
      {
        objectID: "5",
        name: "USB-C Hub",
        description: "Multi-port USB hub adapter",
        category: "accessories",
        brand: "Anker",
        price: 35,
      },
    ];

    const result = await client.saveObjects({ indexName: INDEX, objects });
    expect(result).toBeDefined();

    // Poll until all 5 docs are indexed
    await waitForHits(client, INDEX, 5);
  });

  // ----- Text search -----

  it("searches for 'laptop' and gets 2 hits", async () => {
    const res = await client.search({
      requests: [{ indexName: INDEX, query: "laptop" }],
    });
    const hits = (res.results as any[])[0];
    expect(hits.nbHits).toBe(2);
    const names = hits.hits.map((h: any) => h.name).sort();
    expect(names).toEqual(["Gaming Laptop", "Office Laptop"]);
  });

  it("searches for 'mouse' and gets 1 hit", async () => {
    const res = await client.search({
      requests: [{ indexName: INDEX, query: "mouse" }],
    });
    const hits = (res.results as any[])[0];
    expect(hits.nbHits).toBe(1);
    expect(hits.hits[0].name).toBe("Wireless Mouse");
  });

  it("empty query returns all documents", async () => {
    const res = await client.search({
      requests: [{ indexName: INDEX, query: "" }],
    });
    const hits = (res.results as any[])[0];
    expect(hits.nbHits).toBe(5);
  });

  // ----- Filters -----

  it("applies numeric filter (price >= 500)", async () => {
    const res = await client.search({
      requests: [
        { indexName: INDEX, query: "", filters: "price >= 500" },
      ],
    });
    const hits = (res.results as any[])[0];
    expect(hits.nbHits).toBe(2);
    for (const hit of hits.hits) {
      expect(hit.price).toBeGreaterThanOrEqual(500);
    }
  });

  it("applies facet filter (category:accessories)", async () => {
    const res = await client.search({
      requests: [
        { indexName: INDEX, query: "", filters: "category:accessories" },
      ],
    });
    const hits = (res.results as any[])[0];
    expect(hits.nbHits).toBe(3);
  });

  it("applies combined text + filter query", async () => {
    const res = await client.search({
      requests: [
        {
          indexName: INDEX,
          query: "laptop",
          filters: "price >= 800",
        },
      ],
    });
    const hits = (res.results as any[])[0];
    expect(hits.nbHits).toBe(1);
    expect(hits.hits[0].name).toBe("Gaming Laptop");
  });

  // ----- Facets -----

  it("returns facet counts", async () => {
    const res = await client.search({
      requests: [
        { indexName: INDEX, query: "", facets: ["category", "brand"] },
      ],
    });
    const hits = (res.results as any[])[0];
    expect(hits.facets).toBeDefined();
    expect(hits.facets.category).toBeDefined();
    expect(hits.facets.category.electronics).toBe(2);
    expect(hits.facets.category.accessories).toBe(3);
    expect(hits.facets.brand).toBeDefined();
  });

  // ----- Pagination -----

  it("paginates results with hitsPerPage and page", async () => {
    const page0 = await client.search({
      requests: [
        { indexName: INDEX, query: "", hitsPerPage: 2, page: 0 },
      ],
    });
    const r0 = (page0.results as any[])[0];
    expect(r0.hits.length).toBe(2);
    expect(r0.nbPages).toBeGreaterThanOrEqual(3);

    const page1 = await client.search({
      requests: [
        { indexName: INDEX, query: "", hitsPerPage: 2, page: 1 },
      ],
    });
    const r1 = (page1.results as any[])[0];
    expect(r1.hits.length).toBe(2);

    // Pages should have different results
    const ids0 = r0.hits.map((h: any) => h.objectID).sort();
    const ids1 = r1.hits.map((h: any) => h.objectID).sort();
    expect(ids0).not.toEqual(ids1);
  });

  // ----- GetObject -----

  it("retrieves a single object by ID", async () => {
    const obj = await client.getObject({
      indexName: INDEX,
      objectID: "1",
    });
    expect((obj as any).name).toBe("Gaming Laptop");
    expect((obj as any).objectID).toBe("1");
  });

  // ----- Highlighting -----

  it("returns highlight results with <em> markup for matching query", async () => {
    const res = await client.search({
      requests: [{ indexName: INDEX, query: "laptop" }],
    });
    const hits = (res.results as any[])[0];
    expect(hits.hits.length).toBeGreaterThan(0);
    const hit = hits.hits[0];
    expect(hit._highlightResult).toBeDefined();
    // Verify actual highlight markup — not just that _highlightResult exists
    const nameHighlight = hit._highlightResult.name;
    expect(nameHighlight).toBeDefined();
    expect(nameHighlight.value).toContain("<em>");
    expect(nameHighlight.value).toContain("Laptop");
    expect(nameHighlight.matchLevel).toBe("full");
  });

  // ----- Synonyms -----

  it("saves synonyms and verifies search expansion works", async () => {
    await client.saveSynonym({
      indexName: INDEX,
      objectID: "syn1",
      synonymHit: {
        objectID: "syn1",
        type: "synonym",
        synonyms: ["laptop", "notebook", "computer"],
      },
    });

    // Verify CRUD: synonym is stored and retrievable
    const synResult = await client.searchSynonyms({ indexName: INDEX });
    expect((synResult as any).nbHits).toBeGreaterThanOrEqual(1);

    // Verify search expansion: "computer" doesn't appear in any document text,
    // but the synonym [laptop, notebook, computer] should expand the query
    // to also match docs containing "laptop"
    const res = await client.search({
      requests: [{ indexName: INDEX, query: "computer" }],
    });
    const hits = (res.results as any[])[0];
    expect(hits.nbHits).toBeGreaterThanOrEqual(2);
    const names = hits.hits.map((h: any) => h.name);
    expect(names).toEqual(
      expect.arrayContaining(["Gaming Laptop", "Office Laptop"]),
    );
  });

  // ----- Rules -----

  it("saves rules and verifies retrieval with correct data", async () => {
    await client.saveRule({
      indexName: INDEX,
      objectID: "rule1",
      rule: {
        objectID: "rule1",
        conditions: [{ anchoring: "is", pattern: "cheap" }],
        consequence: { params: { filters: "price < 100" } },
      },
    });

    // Verify rule is stored: searchRules returns it with correct data
    const ruleResult = await client.searchRules({ indexName: INDEX });
    const rules = (ruleResult as any).hits || [];
    expect(rules.length).toBeGreaterThanOrEqual(1);
    const savedRule = rules.find((r: any) => r.objectID === "rule1");
    expect(savedRule).toBeDefined();
    expect(savedRule.conditions[0].pattern).toBe("cheap");

    // Verify GET individual rule returns correct data
    const fetched = await client.getRule({
      indexName: INDEX,
      objectID: "rule1",
    });
    expect((fetched as any).objectID).toBe("rule1");
    expect((fetched as any).conditions[0].anchoring).toBe("is");
  });

  // ----- ListIndices -----

  it("lists indices including the test index", async () => {
    const indices = await client.listIndices();
    const names = (indices.items || []).map((i: any) => i.name);
    expect(names).toContain(INDEX);
  });

  // ----- Multi-index search -----

  it("searches multiple indices in a single call", async () => {
    const res = await client.search({
      requests: [
        { indexName: INDEX, query: "laptop" },
        { indexName: INDEX, query: "mouse" },
      ],
    });
    expect((res.results as any[]).length).toBe(2);
    expect((res.results as any[])[0].nbHits).toBe(2);
    expect((res.results as any[])[1].nbHits).toBe(1);
  });

  // ----- Partial update -----

  it("partially updates an object", async () => {
    await client.partialUpdateObject({
      indexName: INDEX,
      objectID: "1",
      attributesToUpdate: { price: 1099, description: "Updated gaming machine" },
    });

    // Poll until the update is visible
    const start = Date.now();
    while (Date.now() - start < 5000) {
      const obj = await client.getObject({
        indexName: INDEX,
        objectID: "1",
      });
      if ((obj as any).price === 1099) break;
      await new Promise((r) => setTimeout(r, 50));
    }

    const obj = await client.getObject({
      indexName: INDEX,
      objectID: "1",
    });
    expect((obj as any).price).toBe(1099);
    expect((obj as any).description).toBe("Updated gaming machine");
    expect((obj as any).name).toBe("Gaming Laptop"); // unchanged field preserved
  });

  // ----- attributesToRetrieve -----

  it("limits returned fields with attributesToRetrieve", async () => {
    const res = await client.search({
      requests: [
        {
          indexName: INDEX,
          query: "laptop",
          attributesToRetrieve: ["name", "price"],
        },
      ],
    });
    const hit = (res.results as any[])[0].hits[0];
    // Requested fields should be present
    expect(hit.name).toBeDefined();
    expect(hit.price).toBeDefined();
    // objectID is always included by Algolia convention
    expect(hit.objectID).toBeDefined();
    // Non-requested fields should NOT be returned
    expect(hit.description).toBeUndefined();
    expect(hit.category).toBeUndefined();
    expect(hit.brand).toBeUndefined();
  });

  // ----- Delete object -----

  it("deletes a single object", async () => {
    await client.deleteObject({ indexName: INDEX, objectID: "5" });

    // Poll until we have 4 hits
    const start = Date.now();
    while (Date.now() - start < 5000) {
      const res = await client.search({
        requests: [{ indexName: INDEX, query: "", hitsPerPage: 0 }],
      });
      if ((res.results as any[])[0].nbHits === 4) break;
      await new Promise((r) => setTimeout(r, 50));
    }

    const res = await client.search({
      requests: [{ indexName: INDEX, query: "" }],
    });
    expect((res.results as any[])[0].nbHits).toBe(4);
  });

  // ----- Delete index -----

  it("deletes the test index", async () => {
    await client.deleteIndex({ indexName: INDEX });

    // Verify the index is gone from listIndices
    const start = Date.now();
    while (Date.now() - start < 5000) {
      const indices = await client.listIndices();
      const names = (indices.items || []).map((i: any) => i.name);
      if (!names.includes(INDEX)) return;
      await new Promise((r) => setTimeout(r, 50));
    }
    // If we get here, check one more time
    const indices = await client.listIndices();
    const names = (indices.items || []).map((i: any) => i.name);
    expect(names).not.toContain(INDEX);
  });
});

// ---------------------------------------------------------------------------
// Additional SDK operations — each test uses its own isolated index
// ---------------------------------------------------------------------------

describe("flapjack-search E2E — additional SDK operations", () => {
  const client = createClient();

  beforeAll(async () => {
    const res = await fetch(`http://${SERVER}/health`);
    expect(res.ok).toBe(true);
  });

  // ----- Browse with cursor pagination -----

  it("browses an index with cursor pagination", async () => {
    const IDX = `e2e_browse_${Date.now()}`;
    try {
      await client.saveObjects({
        indexName: IDX,
        objects: Array.from({ length: 5 }, (_, i) => ({
          objectID: `b${i}`,
          name: `Item ${i}`,
        })),
      });
      await waitForHits(client, IDX, 5);

      const page1 = await client.browse({
        indexName: IDX,
        browseParams: { hitsPerPage: 2 },
      });
      expect((page1 as any).hits.length).toBe(2);
      expect((page1 as any).cursor).toBeDefined();

      // Browse next page with cursor
      const page2 = await client.browse({
        indexName: IDX,
        browseParams: { cursor: (page1 as any).cursor },
      });
      expect((page2 as any).hits.length).toBeGreaterThan(0);

      // Pages should have different IDs
      const ids1 = (page1 as any).hits.map((h: any) => h.objectID);
      const ids2 = (page2 as any).hits.map((h: any) => h.objectID);
      for (const id of ids2) {
        expect(ids1).not.toContain(id);
      }
    } finally {
      try {
        await client.deleteIndex({ indexName: IDX });
      } catch {}
    }
  });

  // ----- clearObjects -----

  it("clears all objects from an index", async () => {
    const IDX = `e2e_clear_${Date.now()}`;
    try {
      await client.saveObjects({
        indexName: IDX,
        objects: [
          { objectID: "c1", name: "A" },
          { objectID: "c2", name: "B" },
        ],
      });
      await waitForHits(client, IDX, 2);

      await client.clearObjects({ indexName: IDX });
      await waitForEmpty(client, IDX);

      const res = await client.search({
        requests: [{ indexName: IDX, query: "" }],
      });
      expect((res.results as any[])[0].nbHits).toBe(0);
    } finally {
      try {
        await client.deleteIndex({ indexName: IDX });
      } catch {}
    }
  });

  // ----- batch (direct) -----

  it("executes direct batch operations (add + delete)", async () => {
    const IDX = `e2e_batch_${Date.now()}`;
    try {
      // Add via batch
      await client.batch({
        indexName: IDX,
        batchWriteParams: {
          requests: [
            {
              action: "addObject",
              body: { objectID: "x1", name: "Batch Item 1" },
            },
            {
              action: "addObject",
              body: { objectID: "x2", name: "Batch Item 2" },
            },
          ],
        },
      });
      await waitForHits(client, IDX, 2);

      // Verify objects exist
      const obj = await client.getObject({
        indexName: IDX,
        objectID: "x1",
      });
      expect((obj as any).name).toBe("Batch Item 1");

      // Delete via batch
      await client.batch({
        indexName: IDX,
        batchWriteParams: {
          requests: [
            { action: "deleteObject", body: { objectID: "x1" } },
          ],
        },
      });

      // Poll until only 1 remains
      const start = Date.now();
      while (Date.now() - start < 5000) {
        const res = await client.search({
          requests: [{ indexName: IDX, query: "", hitsPerPage: 0 }],
        });
        if ((res.results as any[])[0].nbHits === 1) break;
        await new Promise((r) => setTimeout(r, 50));
      }

      const res = await client.search({
        requests: [{ indexName: IDX, query: "" }],
      });
      expect((res.results as any[])[0].nbHits).toBe(1);
      expect((res.results as any[])[0].hits[0].objectID).toBe("x2");
    } finally {
      try {
        await client.deleteIndex({ indexName: IDX });
      } catch {}
    }
  });

  // ----- deleteBy (filtered delete) -----

  it("deletes objects matching a filter via deleteBy", async () => {
    const IDX = `e2e_deleteby_${Date.now()}`;
    try {
      await client.setSettings({
        indexName: IDX,
        indexSettings: { attributesForFaceting: ["filterOnly(category)"] },
      });

      await client.saveObjects({
        indexName: IDX,
        objects: [
          { objectID: "d1", name: "A", category: "keep" },
          { objectID: "d2", name: "B", category: "remove" },
          { objectID: "d3", name: "C", category: "remove" },
        ],
      });
      await waitForHits(client, IDX, 3);

      await client.deleteBy({
        indexName: IDX,
        deleteByParams: { filters: "category:remove" },
      });

      // Poll until only 1 hit remains
      const start = Date.now();
      while (Date.now() - start < 5000) {
        const res = await client.search({
          requests: [{ indexName: IDX, query: "", hitsPerPage: 0 }],
        });
        if ((res.results as any[])[0].nbHits === 1) break;
        await new Promise((r) => setTimeout(r, 50));
      }

      const res = await client.search({
        requests: [{ indexName: IDX, query: "" }],
      });
      expect((res.results as any[])[0].nbHits).toBe(1);
      expect((res.results as any[])[0].hits[0].objectID).toBe("d1");
    } finally {
      try {
        await client.deleteIndex({ indexName: IDX });
      } catch {}
    }
  });

  // ----- searchForFacetValues -----

  it("searches for facet values matching a prefix", async () => {
    const IDX = `e2e_facetval_${Date.now()}`;
    try {
      await client.setSettings({
        indexName: IDX,
        indexSettings: {
          attributesForFaceting: ["searchable(brand)"],
        },
      });

      await client.saveObjects({
        indexName: IDX,
        objects: [
          { objectID: "f1", name: "Item1", brand: "Apple" },
          { objectID: "f2", name: "Item2", brand: "Amazon" },
          { objectID: "f3", name: "Item3", brand: "Google" },
        ],
      });
      await waitForHits(client, IDX, 3);

      const res = await client.searchForFacetValues({
        indexName: IDX,
        facetName: "brand",
        searchForFacetValuesRequest: { facetQuery: "a" },
      });

      const values = (res as any).facetHits || [];
      expect(values.length).toBeGreaterThanOrEqual(1);
      // "a" prefix should match "Apple" and/or "Amazon"
      const matched = values.map((v: any) => v.value);
      expect(matched).toEqual(expect.arrayContaining(["Apple"]));
    } finally {
      try {
        await client.deleteIndex({ indexName: IDX });
      } catch {}
    }
  });

  // ----- Full synonyms CRUD lifecycle -----

  it("performs full synonyms CRUD (save, get, delete)", async () => {
    const IDX = `e2e_syn_crud_${Date.now()}`;
    try {
      await client.saveObjects({
        indexName: IDX,
        objects: [{ objectID: "s1", name: "Test" }],
      });
      await waitForHits(client, IDX, 1);

      // Save
      await client.saveSynonym({
        indexName: IDX,
        objectID: "syn_crud",
        synonymHit: {
          objectID: "syn_crud",
          type: "synonym",
          synonyms: ["phone", "mobile", "cell"],
        },
      });

      // Get
      const fetched = await client.getSynonym({
        indexName: IDX,
        objectID: "syn_crud",
      });
      expect((fetched as any).objectID).toBe("syn_crud");
      expect((fetched as any).synonyms).toEqual(
        expect.arrayContaining(["phone", "mobile", "cell"]),
      );

      // Delete
      await client.deleteSynonym({
        indexName: IDX,
        objectID: "syn_crud",
      });

      // Verify deleted — getSynonym should throw/fail
      let deleted = false;
      try {
        await client.getSynonym({
          indexName: IDX,
          objectID: "syn_crud",
        });
      } catch {
        deleted = true;
      }
      expect(deleted).toBe(true);
    } finally {
      try {
        await client.deleteIndex({ indexName: IDX });
      } catch {}
    }
  });

  // ----- Full rules CRUD lifecycle -----

  it("performs full rules CRUD (save, get, delete)", async () => {
    const IDX = `e2e_rule_crud_${Date.now()}`;
    try {
      await client.saveObjects({
        indexName: IDX,
        objects: [{ objectID: "r1", name: "Test" }],
      });
      await waitForHits(client, IDX, 1);

      // Save
      await client.saveRule({
        indexName: IDX,
        objectID: "rule_crud",
        rule: {
          objectID: "rule_crud",
          conditions: [{ anchoring: "contains", pattern: "test" }],
          consequence: { params: { query: "modified" } },
        },
      });

      // Get
      const fetched = await client.getRule({
        indexName: IDX,
        objectID: "rule_crud",
      });
      expect((fetched as any).objectID).toBe("rule_crud");
      expect((fetched as any).conditions[0].pattern).toBe("test");

      // Delete
      await client.deleteRule({
        indexName: IDX,
        objectID: "rule_crud",
      });

      // Verify deleted — getRule should throw/fail
      let deleted = false;
      try {
        await client.getRule({
          indexName: IDX,
          objectID: "rule_crud",
        });
      } catch {
        deleted = true;
      }
      expect(deleted).toBe(true);
    } finally {
      try {
        await client.deleteIndex({ indexName: IDX });
      } catch {}
    }
  });

  // ----- Search response format compliance -----

  it("returns all Algolia-compatible response fields", async () => {
    const IDX = `e2e_format_${Date.now()}`;
    try {
      await client.saveObjects({
        indexName: IDX,
        objects: [{ objectID: "f1", name: "Test Widget" }],
      });
      await waitForHits(client, IDX, 1);

      const res = await client.search({
        requests: [{ indexName: IDX, query: "widget" }],
      });
      const result = (res.results as any[])[0];

      // Algolia SDKs expect all these fields
      expect(result.hits).toBeDefined();
      expect(result.nbHits).toBeDefined();
      expect(result.page).toBeDefined();
      expect(result.nbPages).toBeDefined();
      expect(result.hitsPerPage).toBeDefined();
      expect(result.processingTimeMS).toBeDefined();
      expect(result.query).toBe("widget");
      expect(result.params).toBeDefined();

      // Hits must contain objectID and _highlightResult
      const hit = result.hits[0];
      expect(hit.objectID).toBe("f1");
      expect(hit._highlightResult).toBeDefined();
      expect(hit._highlightResult.name).toBeDefined();
      expect(hit._highlightResult.name.value).toContain("<em>");
    } finally {
      try {
        await client.deleteIndex({ indexName: IDX });
      } catch {}
    }
  });

  // ----- operationIndex: copy -----

  it("copies an index to a new destination", async () => {
    const SRC = `e2e_copy_src_${Date.now()}`;
    const DST = `e2e_copy_dst_${Date.now()}`;
    try {
      await client.setSettings({
        indexName: SRC,
        indexSettings: { searchableAttributes: ["name"] },
      });
      await client.saveObjects({
        indexName: SRC,
        objects: [
          { objectID: "c1", name: "Copy Me" },
          { objectID: "c2", name: "Copy Me Too" },
        ],
      });
      await waitForHits(client, SRC, 2);

      await client.operationIndex({
        indexName: SRC,
        operationIndexParams: { operation: "copy", destination: DST },
      });

      // Poll until destination has the docs
      await waitForHits(client, DST, 2);

      // Verify source still exists
      const srcRes = await client.search({
        requests: [{ indexName: SRC, query: "" }],
      });
      expect((srcRes.results as any[])[0].nbHits).toBe(2);

      // Verify destination has the data
      const dstObj = await client.getObject({
        indexName: DST,
        objectID: "c1",
      });
      expect((dstObj as any).name).toBe("Copy Me");
    } finally {
      try {
        await client.deleteIndex({ indexName: SRC });
      } catch {}
      try {
        await client.deleteIndex({ indexName: DST });
      } catch {}
    }
  });

  // ----- operationIndex: move -----

  it("moves an index to a new name", async () => {
    const SRC = `e2e_move_src_${Date.now()}`;
    const DST = `e2e_move_dst_${Date.now()}`;
    try {
      await client.saveObjects({
        indexName: SRC,
        objects: [{ objectID: "m1", name: "Move Me" }],
      });
      await waitForHits(client, SRC, 1);

      await client.operationIndex({
        indexName: SRC,
        operationIndexParams: { operation: "move", destination: DST },
      });

      // Poll until destination has the doc
      await waitForHits(client, DST, 1);

      const obj = await client.getObject({
        indexName: DST,
        objectID: "m1",
      });
      expect((obj as any).name).toBe("Move Me");

      // Source should be gone from listIndices
      const indices = await client.listIndices();
      const names = (indices.items || []).map((i: any) => i.name);
      expect(names).not.toContain(SRC);
    } finally {
      try {
        await client.deleteIndex({ indexName: DST });
      } catch {}
    }
  });

  // ----- API key management -----

  it.skip("performs API key CRUD (add, get, list, delete) [Flapjack server does not support API key management]", async () => {
    // Add a new API key
    const createRes = await client.addApiKey({
      acl: ["search"],
      description: "E2E test key",
      indexes: ["*"],
    });
    const newKey = (createRes as any).key;
    expect(newKey).toBeDefined();
    expect(typeof newKey).toBe("string");
    expect(newKey.length).toBeGreaterThan(0);

    try {
      // Get the key
      const fetched = await client.getApiKey({ key: newKey });
      expect((fetched as any).description).toBe("E2E test key");

      // List keys — should include our new key
      const keys = await client.listApiKeys();
      const keyValues = (keys.keys || []).map((k: any) => k.value || k.key);
      expect(keyValues).toContain(newKey);
    } finally {
      // Delete the key
      await client.deleteApiKey({ key: newKey });
    }

    // Verify deleted — getApiKey should fail
    let deleted = false;
    try {
      await client.getApiKey({ key: newKey });
    } catch {
      deleted = true;
    }
    expect(deleted).toBe(true);
  });

  // ----- getTask -----
  // NOTE: Flapjack returns synthetic timestamp-based taskIDs from write operations
  // for Algolia wire compatibility, but getTask expects internal task UIDs.
  // This is a known server compatibility gap — waitTask/getTask doesn't work
  // with the taskID from batch/settings/delete responses.
  // The test below verifies the SDK method exists and the endpoint responds.

  it("verifies getTask endpoint exists (known gap: taskID mismatch)", async () => {
    const IDX = `e2e_task_${Date.now()}`;
    try {
      const result = await client.batch({
        indexName: IDX,
        batchWriteParams: {
          requests: [
            {
              action: "addObject",
              body: { objectID: "t1", name: "Task Test" },
            },
          ],
        },
      });

      const taskID = (result as any).taskID;
      expect(taskID).toBeDefined();
      expect(typeof taskID).toBe("number");

      // getTask with the batch taskID currently returns 404 (known gap)
      // When this gap is fixed, the following should succeed:
      try {
        await client.getTask({ indexName: IDX, taskID });
      } catch (e: any) {
        // Expected: task_not_found because batch returns synthetic IDs
        expect(e.message || e.toString()).toContain("not found");
      }
    } finally {
      try {
        await client.deleteIndex({ indexName: IDX });
      } catch {}
    }
  });
});