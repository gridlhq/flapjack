import { describe, expect, it } from "vitest";
import { flapjackSearch } from "../builds/node";
import type { FlapjackSearch } from "../builds/node";
import { nodeEchoRequester } from "@flapjack-search/requester-testing";

function createTestClient(appId = "test-app-id", apiKey = "test-api-key"): FlapjackSearch {
  return flapjackSearch(appId, apiKey, {
    requester: nodeEchoRequester(),
  });
}

describe("flapjack-search", () => {
  // --- Factory & validation ---

  it("exports flapjackSearch function", () => {
    expect(typeof flapjackSearch).toBe("function");
  });

  it("throws when appId is missing", () => {
    expect(() => flapjackSearch("", "key")).toThrow("`appId` is missing.");
  });

  it("throws when apiKey is missing", () => {
    expect(() => flapjackSearch("appId", "")).toThrow("`apiKey` is missing.");
  });

  it("creates a client with search methods", () => {
    const client = createTestClient();
    expect(client).toBeDefined();
    expect(typeof client.search).toBe("function");
    expect(typeof client._ua).toBe("string");
  });

  // --- User agent (telemetry) ---

  it("_ua includes Flapjack client info", () => {
    const client = createTestClient();
    expect(client._ua).toContain("Flapjack");
  });

  it("_ua includes Node.js version", () => {
    const client = createTestClient();
    expect(client._ua).toContain("Node.js");
  });

  // --- Custom hosts (critical for self-hosted / migration) ---

  it("accepts custom hosts for self-hosted servers", async () => {
    const client = flapjackSearch("my-app", "my-key", {
      requester: nodeEchoRequester(),
      hosts: [{ url: "my-server.example.com", accept: "readWrite", protocol: "https" }],
    });
    const result = await client.search({ requests: [] });
    const echo = result as any;
    expect(echo.host).toBe("my-server.example.com");
  });

  // --- All critical search API methods exist ---

  it("exposes all Algolia-compatible search methods", () => {
    const client = createTestClient();
    const methods = [
      "search", "searchForFacetValues",
      "browse", "browseRules", "browseSynonyms",
      "getObject", "getObjects",
      "saveObjects", "deleteObjects", "partialUpdateObjects",
      "batch", "multipleBatch",
      "clearObjects",
      "getSettings", "setSettings",
      "listIndices", "deleteIndex",
      "saveSynonyms", "getSynonym", "deleteSynonym", "clearSynonyms", "searchSynonyms",
      "saveRules", "getRule", "deleteRule", "clearRules", "searchRules",
      "saveRule", "saveSynonym",
      "listApiKeys", "addApiKey", "getApiKey", "updateApiKey", "deleteApiKey",
      "operationIndex",
      "getTask",
      "deleteObject",
      "partialUpdateObject",
      "deleteBy",
      "replaceAllObjects",
      "indexExists",
    ];
    for (const method of methods) {
      expect(typeof (client as any)[method]).toBe("function");
    }
  });

  // --- Echo requester: verify correct API paths ---

  it("sends search request to correct path", async () => {
    const client = createTestClient();
    const result = await client.search({ requests: [{ indexName: "products" }] });
    const echo = result as any;
    expect(echo.path).toBe("/1/indexes/*/queries");
    expect(echo.data.requests[0].indexName).toBe("products");
  });

  it("sends getObject request to correct path", async () => {
    const client = createTestClient();
    const result = await client.getObject({ indexName: "products", objectID: "123" });
    const echo = result as any;
    expect(echo.path).toBe("/1/indexes/products/123");
    expect(echo.method).toBe("GET");
  });

  it("sends getSettings request to correct path", async () => {
    const client = createTestClient();
    const result = await client.getSettings({ indexName: "products" });
    const echo = result as any;
    expect(echo.path).toBe("/1/indexes/products/settings");
    expect(echo.method).toBe("GET");
  });

  it("sends setSettings request to correct path", async () => {
    const client = createTestClient();
    const result = await client.setSettings({
      indexName: "products",
      indexSettings: { searchableAttributes: ["name", "description"] },
    });
    const echo = result as any;
    expect(echo.path).toBe("/1/indexes/products/settings");
    expect(echo.method).toBe("PUT");
    expect(echo.data.searchableAttributes).toEqual(["name", "description"]);
  });

  it("sends listIndices request to correct path", async () => {
    const client = createTestClient();
    const result = await client.listIndices();
    const echo = result as any;
    expect(echo.path).toBe("/1/indexes");
    expect(echo.method).toBe("GET");
  });

  it("sends batch request to correct path", async () => {
    const client = createTestClient();
    const result = await client.batch({
      indexName: "products",
      batchWriteParams: {
        requests: [{ action: "addObject", body: { name: "Widget" } }],
      },
    });
    const echo = result as any;
    expect(echo.path).toBe("/1/indexes/products/batch");
    expect(echo.method).toBe("POST");
  });

  it("sends deleteIndex request to correct path", async () => {
    const client = createTestClient();
    const result = await client.deleteIndex({ indexName: "old-index" });
    const echo = result as any;
    expect(echo.path).toBe("/1/indexes/old-index");
    expect(echo.method).toBe("DELETE");
  });

  it("sends searchSynonyms request to correct path", async () => {
    const client = createTestClient();
    const result = await client.searchSynonyms({ indexName: "products" });
    const echo = result as any;
    expect(echo.path).toBe("/1/indexes/products/synonyms/search");
    expect(echo.method).toBe("POST");
  });

  it("sends searchRules request to correct path", async () => {
    const client = createTestClient();
    const result = await client.searchRules({ indexName: "products" });
    const echo = result as any;
    expect(echo.path).toBe("/1/indexes/products/rules/search");
    expect(echo.method).toBe("POST");
  });

  // --- Default hosts point to flapjack.io ---

  it("uses flapjack.io hosts by default", async () => {
    const client = flapjackSearch("my-app-id", "my-api-key", {
      requester: nodeEchoRequester(),
    });
    const result = await client.search({ requests: [] });
    const echo = result as any;
    expect(echo.host).toContain("flapjack.io");
  });

  // --- Algolia migration helper: generateSecuredApiKey ---

  it("generates a secured API key", () => {
    const client = createTestClient();
    const securedKey = client.generateSecuredApiKey({
      parentApiKey: "my-parent-key",
      restrictions: { validUntil: 9999999999 },
    });
    expect(typeof securedKey).toBe("string");
    expect(securedKey.length).toBeGreaterThan(0);
  });

  // --- Algolia migration: algoliasearch → flapjackSearch is a simple rename ---

  it("matches algoliasearch v5 constructor pattern", () => {
    // This is the exact pattern Algolia users use:
    //   const client = algoliasearch('APP_ID', 'API_KEY');
    //   client.search({ requests: [{ indexName: 'index', query: 'test' }] });
    //
    // For Flapjack, users just change:
    //   import { flapjackSearch } from 'flapjack-search';
    //   const client = flapjackSearch('APP_ID', 'API_KEY');
    const client = createTestClient("APP_ID", "API_KEY");
    expect(client).toBeDefined();
    expect(typeof client.search).toBe("function");
    expect(typeof client.saveObjects).toBe("function");
    expect(typeof client.getSettings).toBe("function");
  });
});
