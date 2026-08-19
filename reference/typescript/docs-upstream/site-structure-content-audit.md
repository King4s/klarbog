# Site Structure and Content Audit

Date: 2026-05-21

Scope: `www/src/pages`, shared layout/components, footer/header structure, Graphify-backed content coverage, and high-level content quality.

## Current State

- The site has 73 Astro pages.
- All 73 pages use `BaseLayout`.
- `BaseLayout` renders the shared `Header` and `Footer`, so the header/footer are already shared components.
- 69 pages use `PageHero`.
- 63 pages use `SeoEvidence`.
- All `/viden/**` pages currently use `SeoEvidence`.
- `bun run content:coverage` reports all 19 tracked Graphify/content areas as covered.
- `bun run build` passes.
- `www/src/site-map.ts` now centralizes main navigation, footer links, videnshub sections, and route labels.

## Fixed During Audit

`www/src/layouts/BaseLayout.astro` had five stray breadcrumb label lines after `</html>`:

```txt
kontoplan
kreditnota
koerselsgodtgorelse
mobilepay
mcp
```

These could render as loose text after the page footer. They were removed.

The first structural cleanup was also applied:

- `Header.astro` still gets `NAV` via `consts.ts`, but the data now comes from `site-map.ts`.
- `Footer.astro` renders project, knowledge, and legal links from shared data.
- `/viden` renders its section cards from the same shared knowledge sections used by the footer.
- `BaseLayout.astro` uses `labelForPath()` from `site-map.ts` for breadcrumb labels instead of owning large label maps.
- `/viden/sikkerhed/signeret-backup` is now linked from the videnshub.
- `/funktioner` now includes concise content for anlægsaktiver/afskrivning and portfolio/cockpit, so the Graphify coverage check has no thin areas.

## Structural Findings

### 1. Header/footer were shared, but navigation data was split

Status: fixed in first pass.

`Header.astro`, `Footer.astro`, `/viden`, and breadcrumb labels now read from shared route/link data in `www/src/site-map.ts`.

The previous state created drift:

- new Graphify-backed pages are not automatically available in the footer
- page labels exist in several places
- old/new pages can feel inconsistent even though they share the same footer component

Remaining recommendation:

- connect `llms.txt` generation or validation to the same registry
- keep future page additions in `site-map.ts` as part of the page checklist

### 2. Breadcrumb labels were embedded in `BaseLayout`

Status: fixed in first pass.

`BaseLayout.astro` now calls `labelForPath()` from the shared site map. It no longer contains the large `fullPathLabelMap` and `labelMap`.

Remaining recommendation:

- evolve the current link registry into a richer page registry:

```ts
{
  path: "/viden/myndigheder/saft",
  title: "SAF-T eksport",
  section: "Myndigheder",
  footerGroup: "Viden",
  graphCoverage: ["saft"],
}
```

Then metadata, graph coverage, page type, footer grouping, and generated files can all use the same source.

### 3. Videnssider repeat the same page template manually

Most knowledge pages repeat:

- imports for `BaseLayout`, `PageHero`, `SeoEvidence`, `SITE`
- `BaseLayout` metadata props
- `PageHero`
- `<article class="max-w-3xl ... prose-da">`
- `SeoEvidence`

Recommendation:

- create `www/src/layouts/ArticleLayout.astro` or `www/src/components/KnowledgeArticle.astro`
- let individual pages provide metadata, hero copy, evidence, related links, and body slot
- keep custom pages like `/`, `/funktioner`, `/vaerktoej/momsberegner` bespoke

### 4. Page types are mixed but not explicitly encoded

The site currently has several implicit page types:

- landing page: `/`
- product/head-term pages: `/regnskabsprogram`, `/bogfoeringsprogram`, `/ai-bogholder`
- target pages: `/bogfoering-for-freelancere`, `/bogfoering-for-aps`
- knowledge pages: `/viden/**`
- tools: `/vaerktoej/**`
- trust/legal: `/sikkerhed`, `/privatlivspolitik`, `/about`, `/om`
- documentation: `/docs/installation`

Recommendation:

- encode page type in the page registry
- use different shared templates per page type instead of ad hoc variation

## Content Findings

### 1. Graphify coverage is broad, but many pages are thin

Coverage is now good, but many pages are short. Several knowledge pages are around 90-150 words of unique body copy. That is enough to avoid total gaps, but not enough for strong standalone content.

Priority pages to expand first:

- `/viden/myndigheder/saft`
- `/viden/fakturering/e-faktura-offentlig`
- `/viden/bilag/bilagsmail`
- `/viden/import/dinero`
- `/viden/regnskab/aarsrapport`
- `/viden/anlaegsaktiver/afskrivning`
- `/viden/stamdata/cvr`
- `/viden/regnskab/aabningsbalance`

Recommended target for important knowledge pages:

- 400-700 words
- concrete bookkeeping example
- “Sådan håndterer Rentemester det”
- “Hvad er ikke understøttet endnu”
- links to related pages
- repo/test/rule evidence

### 2. Some top-level pages are intentionally bespoke, but should still share blocks

`/`, `/funktioner`, `/saadan-virker-det`, `/kontakt`, and `/vaerktoej/momsberegner` use bespoke section layouts.

That is acceptable, but repeated visual primitives should become components:

- feature card
- related link grid
- roadmap/status item
- trust/evidence block
- CTA strip

### 3. English pages visually diverge

`/about` and `/privacy-policy` do not use `PageHero`. That may be intentional, but it makes them feel older than the Danish pages.

Recommendation:

- either explicitly keep them as minimal machine-readable pages
- or normalize them with `PageHero` and same article spacing

### 4. Claims are mostly conservative

The reviewed copy generally avoids the forbidden claims:

- does not call Rentemester a finished SaaS
- does not call it a registered standard bookkeeping system
- does not claim full SAF-T compliance
- separates local handoff from hosted reviewer access
- separates OIOUBL handoff from PEPPOL transport

Keep this discipline. The best pages are the ones that say exactly what is code-backed and exactly where the boundary is.

## Recommended Refactor Plan

### Phase 1: Registry and Navigation

Status: partially implemented.

`www/src/site-map.ts` now centralizes:

- main navigation
- footer link groups
- videnshub link groups
- breadcrumb labels

Next, extend it or replace it with a richer page registry for:

- `llms.txt` generation or at least validation
- coverage mapping

This reduces drift across the site.

### Phase 2: Shared Article Template

Add a shared article/knowledge-page wrapper:

- metadata props
- `PageHero`
- byline/date handling
- standard article width/spacing
- optional `SeoEvidence`
- optional FAQ JSON-LD
- related links

Then convert `/viden/**` pages gradually.

### Phase 3: Content Deepening

Expand the important Graphify-backed pages from “coverage stubs” into durable pages:

1. SAF-T
2. Public e-invoice/OIOUBL/PEPPOL
3. Bilagsmail/IMAP
4. Dinero import
5. Annual report/iXBRL
6. Fixed assets/depreciation
7. CVR master data
8. Opening balance

Each page should include at least one practical example and one explicit limitation section.

### Phase 4: Automated QA

Extend `bun run content:coverage` into a broader site QA command:

- pages missing registry entry
- registry links pointing to missing files
- footer links pointing to missing pages
- `BaseLayout` pages without date metadata where required
- knowledge pages below a minimum content threshold
- pages with no `SeoEvidence`
- Graphify areas with no dedicated page
