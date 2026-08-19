import { existsSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import {
  FOOTER_KNOWLEDGE_LINKS,
  FOOTER_LEGAL_LINKS,
  FOOTER_PROJECT_LINKS,
  MAIN_NAV,
  VIDEN_SECTIONS,
  type LinkItem,
} from "../src/site-map";

const repoRoot = join(import.meta.dir, "../..");
const pagesRoot = join(repoRoot, "www/src/pages");
const publicRoot = join(repoRoot, "www/public");
const generatedRoutes = new Set(["/sitemap-index.xml"]);

function walk(dir: string): string[] {
  return readdirSync(dir).flatMap((entry) => {
    const path = join(dir, entry);
    return statSync(path).isDirectory() ? walk(path) : [path];
  });
}

function routeFromPage(path: string): string {
  const route = "/" + relative(pagesRoot, path)
    .replace(/\\/g, "/")
    .replace(/\/index\.astro$/, "")
    .replace(/\.astro$/, "");
  return route === "" ? "/" : route;
}

function uniqueLinks(links: LinkItem[]): LinkItem[] {
  return [...new Map(links.map((link) => [link.href, link])).values()];
}

const pageRoutes = new Set(walk(pagesRoot).filter((path) => path.endsWith(".astro")).map(routeFromPage));
const allSiteMapLinks = uniqueLinks([
  ...MAIN_NAV,
  ...FOOTER_PROJECT_LINKS,
  ...FOOTER_KNOWLEDGE_LINKS,
  ...FOOTER_LEGAL_LINKS,
  ...VIDEN_SECTIONS.flatMap((section) => section.links),
]);

const internalLinks = allSiteMapLinks.filter((link) => link.href.startsWith("/"));
const missingLinks = internalLinks.filter((link) => {
  if (pageRoutes.has(link.href) || generatedRoutes.has(link.href)) return false;
  return !existsSync(join(publicRoot, link.href.replace(/^\//, "")));
});

const videnRoutes = [...pageRoutes].filter((route) => route.startsWith("/viden/"));
const videnLinks = new Set(VIDEN_SECTIONS.flatMap((section) => section.links).map((link) => link.href));
const missingFromVidenHub = videnRoutes.filter((route) => !videnLinks.has(route));
const deadVidenHubLinks = [...videnLinks].filter((href) => href.startsWith("/viden/") && !pageRoutes.has(href));

console.log("# Site map QA\n");
console.log(`Pages: ${pageRoutes.size}`);
console.log(`Site map links: ${allSiteMapLinks.length}`);
console.log(`Viden pages: ${videnRoutes.length}`);
console.log(`Viden links: ${[...videnLinks].filter((href) => href.startsWith("/viden/")).length}\n`);

if (missingLinks.length > 0) {
  console.log("Missing internal links:");
  for (const link of missingLinks) console.log(`- ${link.href} (${link.label})`);
}

if (missingFromVidenHub.length > 0) {
  console.log("\nViden pages missing from VIDEN_SECTIONS:");
  for (const route of missingFromVidenHub) console.log(`- ${route}`);
}

if (deadVidenHubLinks.length > 0) {
  console.log("\nDead VIDEN_SECTIONS links:");
  for (const route of deadVidenHubLinks) console.log(`- ${route}`);
}

if (missingLinks.length || missingFromVidenHub.length || deadVidenHubLinks.length) {
  process.exit(1);
}

console.log("All site-map links are valid.");
