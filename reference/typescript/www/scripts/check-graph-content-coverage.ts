import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";

type GraphNode = {
  label?: string;
  source_file?: string;
};

type Capability = {
  id: string;
  title: string;
  graphTerms: string[];
  contentTerms: string[];
  target?: string;
};

const repoRoot = join(import.meta.dir, "../..");
const graphPath = join(repoRoot, "graphify-out/graph.json");
const pagesRoot = join(repoRoot, "www/src/pages");

const capabilities: Capability[] = [
  {
    id: "invoice",
    title: "Faktura, kreditnota, betaling og debitoropfoelgning",
    graphTerms: ["invoice", "credit-note", "reminder", "bad-debt", "refund"],
    contentTerms: ["faktura", "kreditnota", "rykker", "morarente", "debitor", "refundering"],
  },
  {
    id: "public-einvoice",
    title: "Offentlig e-faktura, EAN/GLN, OIOUBL og PEPPOL",
    graphTerms: ["public-einvoice", "peppol", "oioubl", "ean", "gln"],
    contentTerms: ["OIOUBL", "PEPPOL", "EAN", "GLN", "offentlig e-faktura"],
    target: "/viden/fakturering/e-faktura-offentlig",
  },
  {
    id: "recurring-invoice",
    title: "Gentagne fakturaer",
    graphTerms: ["recurring-invoice", "recurring-invoices", "DK-RECURRING-INVOICE"],
    contentTerms: ["gentagne fakturaer", "gentagende faktura", "recurring"],
    target: "/viden/fakturering/gentagne-fakturaer",
  },
  {
    id: "documents",
    title: "Bilag, kvitteringer og dokumentintegritet",
    graphTerms: ["documents", "DocumentMetadata", "ingestDocument", "receipt"],
    contentTerms: ["bilag", "kvittering", "kassebon", "dokumentintegritet"],
  },
  {
    id: "mail-intake",
    title: "Bilagsmail, maildrop, EML og IMAP intake",
    graphTerms: ["mail-intake", "imap-intake", "parseEml", "ingestMailDrop"],
    contentTerms: ["bilagsmail", "maildrop", "EML", "IMAP"],
    target: "/viden/bilag/bilagsmail",
  },
  {
    id: "bank",
    title: "Bankimport, bankmatch og bankafstemning",
    graphTerms: ["bank", "reconciliation", "suggestBankMatches"],
    contentTerms: ["bankimport", "banktransaktion", "bankafstemning", "bankmatch"],
  },
  {
    id: "vat",
    title: "Momsrapport, momsangivelse, reverse charge og VIES",
    graphTerms: ["vat", "vies", "reverse-charge", "momsangivelse"],
    contentTerms: ["momsrapport", "momsindberetning", "reverse charge", "VIES", "momssatser"],
  },
  {
    id: "cvr-master-data",
    title: "CVR-opslag og kunde-/leverandoerstamdata",
    graphTerms: ["cvr", "master-data", "customer", "vendor"],
    contentTerms: ["CVR", "stamdata", "kunde", "leverandør"],
    target: "/viden/stamdata/cvr",
  },
  {
    id: "import-dinero",
    title: "Dinero-import, aabningsbalance og arkiv",
    graphTerms: ["dinero", "import", "opening-balance", "archive"],
    contentTerms: ["Dinero", "import", "åbningsbalance", "arkiv"],
    target: "/viden/import/dinero",
  },
  {
    id: "opening-balance",
    title: "Aabningsbalance og primo",
    graphTerms: ["opening-balance", "postOpeningBalance", "primobalance"],
    contentTerms: ["åbningsbalance", "primobalance", "primo"],
    target: "/viden/regnskab/aabningsbalance",
  },
  {
    id: "assets",
    title: "Anlaegsaktiver, afskrivning og straksafskrivning",
    graphTerms: ["assets", "depreciation", "asset register", "write-off"],
    contentTerms: ["anlægsaktiv", "afskrivning", "aktivregister", "straksafskrivning"],
    target: "/viden/anlaegsaktiver/afskrivning",
  },
  {
    id: "ledger-audit",
    title: "Append-only ledger, audit chain og tilbagefoersler",
    graphTerms: ["ledger", "audit", "reverseJournalEntry", "hashEntry"],
    contentTerms: ["append-only", "audit trail", "tilbageføring", "hash"],
  },
  {
    id: "backup-restore",
    title: "Backup, restore og signering",
    graphTerms: ["system-backups", "system-restore", "ed25519", "signature"],
    contentTerms: ["backup", "restore", "signeret", "ed25519"],
  },
  {
    id: "saft",
    title: "SAF-T eksport",
    graphTerms: ["saft", "exportSaftPackage", "DK-BOOKKEEPING-SAFT"],
    contentTerms: ["SAF-T", "Standard Audit File", "saft.xml"],
    target: "/viden/myndigheder/saft",
  },
  {
    id: "authority-export",
    title: "Myndighedseksport og revisor-handoff",
    graphTerms: ["authority-export", "export-accountant", "handoff"],
    contentTerms: ["eksport af regnskab", "revisor-handoff", "myndighedseksport"],
  },
  {
    id: "gdpr",
    title: "GDPR-indsigt og sletning under opbevaringspligt",
    graphTerms: ["gdpr", "eraseGdprSubject", "buildGdprSubjectExport"],
    contentTerms: ["GDPR", "persondata", "indsigt", "sletning"],
    target: "/viden/sikkerhed/gdpr",
  },
  {
    id: "annual-report",
    title: "Aarsrapport og iXBRL",
    graphTerms: ["annual-report", "ixbrl", "buildAnnualReport", "generateIxbrl"],
    contentTerms: ["årsrapport", "iXBRL", "regnskabsklasse B"],
    target: "/viden/regnskab/aarsrapport",
  },
  {
    id: "mcp-agent",
    title: "MCP-vaerktoejer og agent-workflow",
    graphTerms: ["mcp", "agent", "contract"],
    contentTerms: ["MCP", "agent", "AI-bogføring med kontrol"],
  },
  {
    id: "workspace-portfolio",
    title: "Multi-company workspace, portfolio og cockpit",
    graphTerms: ["workspace", "portfolio", "server", "cockpit"],
    contentTerms: ["portfolio", "flere virksomheder", "kontrolpanel", "cockpit"],
    target: "/viden/regnskab/flere-virksomheder",
  },
];

function walk(dir: string): string[] {
  return readdirSync(dir).flatMap((entry) => {
    const path = join(dir, entry);
    return statSync(path).isDirectory() ? walk(path) : [path];
  });
}

function normalize(value: string) {
  return value.toLocaleLowerCase("da-DK");
}

function countMatches(haystack: string, terms: string[]) {
  const text = normalize(haystack);
  return terms.reduce((count, term) => count + (text.includes(normalize(term)) ? 1 : 0), 0);
}

if (!existsSync(graphPath)) {
  throw new Error(`Missing Graphify graph: ${relative(repoRoot, graphPath)}`);
}

const graph = JSON.parse(readFileSync(graphPath, "utf8")) as { nodes?: GraphNode[] };
const nodes = graph.nodes ?? [];
const graphCorpus = nodes.map((node) => `${node.source_file ?? ""} ${node.label ?? ""}`).join("\n");

const pageFiles = walk(pagesRoot).filter((path) => path.endsWith(".astro"));
const pages = pageFiles.map((path) => ({
  path,
  route: "/" + relative(pagesRoot, path).replace(/\\/g, "/").replace(/\/index\.astro$/, "").replace(/\.astro$/, ""),
  text: readFileSync(path, "utf8"),
}));

const rows = capabilities.map((capability) => {
  const graphHits = countMatches(graphCorpus, capability.graphTerms);
  const pageHits = pages
    .map((page) => ({ route: page.route === "/" ? "/" : page.route, score: countMatches(page.text, capability.contentTerms) }))
    .filter((page) => page.score > 0)
    .sort((a, b) => b.score - a.score);
  const targetCovered = capability.target ? pages.some((page) => page.route === capability.target) : true;
  const status = graphHits === 0
    ? "no-graph-signal"
    : pageHits.length === 0 || !targetCovered
      ? "missing"
      : pageHits.length < 2
        ? "thin"
        : "covered";

  return { capability, graphHits, pageHits, status };
});

console.log("# Graphify content coverage\n");
console.log(`Graph nodes: ${nodes.length}`);
console.log(`Content pages: ${pages.length}\n`);
for (const row of rows) {
  const target = row.capability.target ? ` target=${row.capability.target}` : "";
  console.log(`- ${row.status.padEnd(15)} ${row.capability.title} graph=${row.graphHits} pages=${row.pageHits.length}${target}`);
  for (const page of row.pageHits.slice(0, 3)) {
    console.log(`  - ${page.route} (${page.score})`);
  }
}

const missing = rows.filter((row) => row.status === "missing");
if (missing.length > 0) {
  console.log("\nMissing targets:");
  for (const row of missing) {
    console.log(`- ${row.capability.title}${row.capability.target ? ` -> ${row.capability.target}` : ""}`);
  }
}
