# Flapjack Dashboard — Multi-Tenancy UI Enhancement Plan

> **Status: COMPLETE** — All 6 phases implemented. 475 tests passing (35 new + 440 existing).

> **Goal:** Make tenant awareness best-in-class for an open-source search engine dashboard.
> Simple by default for single-index users, progressively detailed for multi-tenant deployments.

## Research Summary

### How Flapjack Multi-Tenancy Works
- **Index = Tenant.** Each index in Flapjack represents an isolated data boundary (tenant).
- **Scoped API keys** restrict access to specific indices, enabling per-tenant security.
- Flapjack supports 600+ indices per 4GB node — designed for dense multi-tenant deployments.
- No explicit "tenant" API entity exists — tenancy is implicit via index boundaries + key scoping.

### Industry Best Practices (from Algolia, Elasticsearch, Grafana, Stripe, Vercel)
1. **Progressive disclosure** — hide multi-tenant chrome when only 1 tenant exists
2. **Contextual tooltips** — explain technical concepts inline with `?` icons
3. **Tenant health at a glance** — colored status dots per tenant on overview pages
4. **Scoping visibility** — always show which tenant/index context you're operating in
5. **Educational onboarding** — dismissible banners for first-time multi-tenant users

---

## Implementation Checklist

### Phase 1: Foundation Components

- [x] **1.1 — Tooltip UI component**
  Add `@radix-ui/react-tooltip` and create `tooltip.tsx` in `components/ui/`.
  All new help icons will use this for consistent, accessible tooltips.

- [x] **1.2 — InfoTooltip helper component**
  Reusable `<InfoTooltip content="..." />` — renders a `HelpCircle` icon with tooltip.
  Use everywhere tenant concepts need explaining.

- [x] **1.3 — Multi-Tenant Info Banner component**
  Dismissible educational callout explaining Flapjack's index-as-tenant model.
  Shows when indices > 1. Remembers dismissal in localStorage.

### Phase 2: Overview Page Enhancements

- [x] **2.1 — Tenant Overview Cards**
  When indices > 1, show a "Tenant Overview" section with per-index cards showing:
  name, document count, storage, health status dot.

- [x] **2.2 — Stat card tooltips**
  Add InfoTooltip to "Indices" stat card explaining the index = tenant concept.
  Add tooltip to "Status" card explaining what healthy/degraded means.

### Phase 3: System Page Enhancements

- [x] **3.1 — Enhanced TenantHealthSummary**
  Add InfoTooltip explaining what "Tenant Health" means.
  Improve the health summary with clickable index names linking to index pages.

- [x] **3.2 — Indices Tab tenant context**
  Add InfoTooltip to the "Index Details" section explaining that each index
  is a tenant boundary with isolated data.

### Phase 4: API Keys Page Enhancements

- [x] **4.1 — Tenant scoping education**
  Add InfoTooltip to "Index Scope" section on key cards explaining how scoped
  keys enable multi-tenant security.

- [x] **4.2 — Filter bar help text**
  Add helper text to the index filter bar explaining its purpose.

- [x] **4.3 — CreateKeyDialog tooltip**
  Add InfoTooltip to the Index Scope section in the create dialog explaining
  tenant isolation via API keys.

### Phase 5: Sidebar Enhancement

- [x] **5.1 — Indices section tooltip**
  Add InfoTooltip next to "INDICES" header in sidebar explaining that indices
  serve as tenant boundaries.

### Phase 6: Tests

- [x] **6.1 — Tooltip component tests**
  Test InfoTooltip renders, shows content on hover.

- [x] **6.2 — Multi-tenant banner tests**
  Test banner shows when multiple indices, hides when single, dismissal persists.

- [x] **6.3 — Overview tenant cards tests**
  Test per-index tenant cards appear with correct data when indices > 1.

- [x] **6.4 — System page tooltip tests**
  Test tooltips on TenantHealthSummary, clickable index names.

- [x] **6.5 — API Keys tooltip tests**
  Test scoping tooltips and filter bar help text.

- [x] **6.6 — Sidebar tooltip tests**
  Test indices section has informational tooltip.

---

## Design Principles

1. **Invisible when irrelevant** — single-index users see zero multi-tenant UI
2. **Obvious when relevant** — multi-index users get clear tenant awareness
3. **Educational** — every tenant concept has a tooltip explaining it simply
4. **Non-blocking** — tooltips and banners never block workflows
5. **Consistent** — all help icons use the same `InfoTooltip` pattern
6. **Accessible** — tooltips triggered on hover AND focus, `aria-describedby` support

## Tooltip Content Standards

| Concept | Tooltip Text |
|---------|-------------|
| Indices (stat card) | "Each index is an isolated data container. In multi-tenant setups, each tenant typically gets its own index." |
| Status | "Shows the overall health of your Flapjack server. 'Healthy' means all systems are operational." |
| Tenant Health | "Shows the health status of each index (tenant). Green means healthy with no pending operations." |
| Index Scope (keys) | "Restricting a key to specific indices creates tenant-level security. The key can only access data in the selected indices." |
| Indices (sidebar) | "Each index is an isolated search collection. In multi-tenant deployments, indices serve as tenant boundaries." |
| Banner | "Flapjack uses indices as tenant boundaries. Each index has its own data, settings, and access controls — completely isolated from other indices." |

---

## Test Coverage Targets

| Feature | Test File | Tests |
|---------|-----------|-------|
| InfoTooltip | `tests/pages/multi-index-ui.spec.ts` | Renders help icon, shows tooltip on hover |
| Multi-tenant banner | `tests/pages/multi-index-ui.spec.ts` | Shows with 2+ indices, hidden with 1, dismiss persists |
| Tenant overview cards | `tests/pages/multi-index-ui.spec.ts` | Shows per-index cards, correct data, hidden with 1 index |
| System tooltips | `tests/pages/multi-index-ui.spec.ts` | Tooltip on tenant health, clickable index names |
| API Keys tooltips | `tests/pages/multi-index-ui.spec.ts` | Scope tooltip, filter help text |
| Sidebar tooltip | `tests/pages/multi-index-ui.spec.ts` | Indices section help icon |
| Progressive disclosure | `tests/pages/multi-index-ui.spec.ts` | Single-index hides MT features, tooltips still show |

---

## Files Changed

### New Files
| File | Purpose |
|------|---------|
| `src/components/ui/tooltip.tsx` | Radix UI Tooltip primitives (Provider, Root, Trigger, Content) |
| `src/components/ui/info-tooltip.tsx` | Reusable `<InfoTooltip>` component with HelpCircle icon |
| `src/components/MultiTenantBanner.tsx` | Dismissible educational banner for multi-tenant awareness |
| `tests/pages/multi-index-ui.spec.ts` | 35 Playwright tests covering all new UI features |

### Modified Files
| File | Changes |
|------|---------|
| `src/pages/Overview.tsx` | Added MultiTenantBanner, Tenant Overview Cards section, InfoTooltips on stat cards |
| `src/pages/System.tsx` | Added InfoTooltip to TenantHealthSummary, made index names clickable links, added tooltip to Index Details |
| `src/pages/ApiKeys.tsx` | Added InfoTooltip to Index Scope on key cards, added help text + tooltip to filter bar |
| `src/components/keys/CreateKeyDialog.tsx` | Added InfoTooltip to Index Scope section |
| `src/components/layout/Sidebar.tsx` | Added InfoTooltip to "INDICES" section header |
| `tests/pages/overview.spec.ts` | Fixed storage test selector to handle new tenant cards |
| `package.json` | Added `@radix-ui/react-tooltip` dependency |

### Dependencies Added
- `@radix-ui/react-tooltip` — accessible tooltip primitives

## Final Test Results

```
475 tests passed (0 failed)
- 35 new multi-tenancy UI tests
- 440 existing tests (zero regressions)
```
