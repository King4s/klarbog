import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import {
  compareParagrafIds,
  findProvision,
  loadAllProvisions,
  parseProvisionRef,
  type Provision,
} from "./legal-provisions";
import { readRuleMetadata, type RuleMetadata } from "./rules-metadata";

// Regulatory coverage — like code coverage, but it measures how much of the
// cited Danish legislation is traceably implemented in code. See
// docs/regulatory-coverage.md. The result is pure and deterministic.

export type ClosureError = {
  ruleId: string;
  sourceId: string;
  ref: string;
  reason: "unresolved" | "cross_source";
  resolvedSourceId?: string;
};

export type DriftError = {
  ruleId: string;
  sourceId: string;
  ref: string;
  citedTextHash: string;
  currentTextHash: string;
};

export type SourceCoverage = {
  sourceId: string;
  operativeCount: number;
  citedCount: number;
  inScopeOperativeCount: number;
  inScopeCitedCount: number;
  coveredRefs: string[];
  uncoveredRefs: string[];
};

// A declared in-scope range: a single paragraf (low === high) or an inclusive
// span. Paragraf identifiers may carry a letter suffix ("9b", "138a").
export type ScopeRange = { low: string; high: string };

export type SourceScope =
  | { kind: "all" }
  | { kind: "ranges"; ranges: ScopeRange[] };

export type ScopeManifest = {
  version: string;
  sources: Map<string, SourceScope>;
};

export type ScopeError =
  | { kind: "missing_source"; sourceId: string }
  | { kind: "bad_endpoint"; sourceId: string; paragraf: string }
  | { kind: "citation_out_of_scope"; ruleId: string; sourceId: string; ref: string };

export type UncoveredProvision = {
  sourceId: string;
  ref: string;
};

export type ReverseMapEntry = {
  ruleId: string;
  codeFiles: string[];
  testFiles: string[];
};

export type ReverseMapRecord = {
  sourceId: string;
  ref: string;
  rules: ReverseMapEntry[];
};

export type RegulatoryCoverage = {
  overall: {
    operativeCount: number;
    citedCount: number;
    inScopeOperativeCount: number;
    inScopeCitedCount: number;
  };
  perSource: SourceCoverage[];
  closureErrors: ClosureError[];
  driftErrors: DriftError[];
  scopeErrors: ScopeError[];
  uncitedRules: string[];
  uncoveredProvisions: UncoveredProvision[];
  reverseMap: ReverseMapRecord[];
};

function repoRoot(): string {
  return fileURLToPath(new URL("../../", import.meta.url));
}

function compareStrings(a: string, b: string): number {
  return a < b ? -1 : a > b ? 1 : 0;
}

// Extracts the bare paragraf identifier ("9b") from a range endpoint token
// ("§ 9b"). Returns undefined for anything that is not a `§ <id>` token.
function parseScopeParagraf(token: string): string | undefined {
  const match = token.trim().match(/^§\s*(\d+[a-zæøå]*)$/i);
  return match ? match[1].toLowerCase() : undefined;
}

// Hand-parses sources/scope.yaml — the repo hand-parses YAML (see
// rules-metadata.ts); this follows that pattern. The grammar is fixed and
// shallow: a `version:` line, then a `sources:` map of source id -> either
// `in_scope: all` or a list of `- "§ ..."` range strings. A malformed manifest
// throws rather than parsing partially — a silently dropped range would make
// coverage look better than it really is.
export function parseScopeManifest(text: string): ScopeManifest {
  const lines = text.split(/\r?\n/);
  let version: string | undefined;
  const sources = new Map<string, SourceScope>();

  let inSources = false;
  let currentSource: string | null = null;
  let currentRanges: ScopeRange[] | null = null;
  let currentKind: "all" | "ranges" | null = null;

  const unquote = (value: string): string => {
    const t = value.trim();
    if (
      t.length >= 2 &&
      ((t.startsWith('"') && t.endsWith('"')) || (t.startsWith("'") && t.endsWith("'")))
    ) {
      return t.slice(1, -1);
    }
    return t;
  };

  const flushSource = () => {
    if (currentSource === null) return;
    if (sources.has(currentSource)) {
      throw new Error(`scope.yaml: source ${currentSource} is declared more than once`);
    }
    if (currentKind === "all") {
      sources.set(currentSource, { kind: "all" });
    } else if (currentKind === "ranges") {
      if ((currentRanges ?? []).length === 0) {
        throw new Error(
          `scope.yaml: source ${currentSource} declares in_scope with no ranges`,
        );
      }
      sources.set(currentSource, { kind: "ranges", ranges: currentRanges! });
    } else {
      throw new Error(
        `scope.yaml: source ${currentSource} has no in_scope declaration`,
      );
    }
    currentSource = null;
    currentRanges = null;
    currentKind = null;
  };

  for (const raw of lines) {
    if (raw.trim().length === 0) continue;
    if (raw.includes("\t")) {
      throw new Error("scope.yaml: tab indentation is not allowed");
    }
    if (/^\s*#/.test(raw)) continue;
    const indent = raw.length - raw.trimStart().length;
    const line = raw.trim();

    if (indent === 0) {
      flushSource();
      const versionMatch = line.match(/^version:\s*(\S+)$/);
      if (versionMatch) {
        version = versionMatch[1];
        inSources = false;
        continue;
      }
      if (line === "sources:") {
        inSources = true;
        continue;
      }
      throw new Error(`scope.yaml: unexpected top-level line: ${line}`);
    }

    if (!inSources) {
      throw new Error(`scope.yaml: line outside the sources: block: ${line}`);
    }

    // A 2-space-indented `<source-id>:` opens a source entry.
    if (indent === 2) {
      flushSource();
      const sourceMatch = line.match(/^(DK-[A-Z0-9-]+):$/);
      if (!sourceMatch) {
        throw new Error(`scope.yaml: expected a \`DK-...:\` source key, got: ${line}`);
      }
      currentSource = sourceMatch[1];
      continue;
    }

    if (currentSource === null) {
      throw new Error(`scope.yaml: orphan line with no source: ${line}`);
    }

    // The `in_scope:` key of a source — either `all` inline or a list below.
    if (indent === 4) {
      const inScopeAll = line.match(/^in_scope:\s*all$/);
      if (inScopeAll) {
        currentKind = "all";
        continue;
      }
      if (line === "in_scope:") {
        currentKind = "ranges";
        currentRanges = [];
        continue;
      }
      throw new Error(
        `scope.yaml: source ${currentSource} expected \`in_scope: all\` or ` +
          `\`in_scope:\`, got: ${line}`,
      );
    }

    // A 6-space-indented `- "..."` is one range entry.
    if (indent === 6) {
      if (currentKind !== "ranges") {
        throw new Error(
          `scope.yaml: source ${currentSource} has a range entry without \`in_scope:\``,
        );
      }
      const itemMatch = line.match(/^-\s*(.+)$/);
      if (!itemMatch) {
        throw new Error(
          `scope.yaml: source ${currentSource} range entry must be \`- "§ ..."\`, got: ${line}`,
        );
      }
      const value = unquote(itemMatch[1]);
      // Split only on a hyphen that introduces the second `§` endpoint, so a
      // hyphen inside an identifier token cannot mis-split the range.
      const parts = value.split(/\s*-\s*(?=§)/).map((part) => part.trim());
      let range: ScopeRange;
      if (parts.length === 1) {
        const id = parseScopeParagraf(parts[0]);
        if (!id) {
          throw new Error(`scope.yaml: source ${currentSource} bad range token: ${value}`);
        }
        range = { low: id, high: id };
      } else if (parts.length === 2) {
        const low = parseScopeParagraf(parts[0]);
        const high = parseScopeParagraf(parts[1]);
        if (!low || !high) {
          throw new Error(`scope.yaml: source ${currentSource} bad range: ${value}`);
        }
        if (compareParagrafIds(low, high) > 0) {
          throw new Error(`scope.yaml: source ${currentSource} range is inverted: ${value}`);
        }
        range = { low, high };
      } else {
        throw new Error(`scope.yaml: source ${currentSource} malformed range: ${value}`);
      }
      if (currentRanges!.some((r) => r.low === range.low && r.high === range.high)) {
        throw new Error(`scope.yaml: source ${currentSource} has a duplicate range: ${value}`);
      }
      currentRanges!.push(range);
      continue;
    }

    throw new Error(`scope.yaml: unexpected indentation in line: ${line}`);
  }
  flushSource();

  if (version === undefined) {
    throw new Error("scope.yaml: missing version");
  }
  if (sources.size === 0) {
    throw new Error("scope.yaml: no sources declared");
  }
  return { version, sources };
}

function loadScopeManifest(rootDir: string): ScopeManifest {
  return parseScopeManifest(readFileSync(join(rootDir, "sources", "scope.yaml"), "utf8"));
}

// A provision is in scope if its source is `in_scope: all` or its paragraf
// falls within a declared range. Range membership is numeric-aware so that
// "9b" lies between "9" and "10".
function paragrafInScope(scope: SourceScope | undefined, paragraf: string): boolean {
  if (!scope) return false;
  if (scope.kind === "all") return true;
  for (const range of scope.ranges) {
    if (
      compareParagrafIds(paragraf, range.low) >= 0 &&
      compareParagrafIds(paragraf, range.high) <= 0
    ) {
      return true;
    }
  }
  return false;
}

function refParagraf(ref: string): string | undefined {
  const path = parseProvisionRef(ref);
  return path ? path[0] : undefined;
}

// The three scope hard-error checks, as a pure function of the inputs so they
// are testable against synthetic manifests without a fixture filesystem:
//  (a) every downloaded source must appear in the scope manifest;
//  (b) every range endpoint must reference a paragraf that exists in its source;
//  (c) every cited operative provision must fall inside the declared scope.
export function evaluateScope(
  provisionsBySource: Map<string, Provision[]>,
  scope: ScopeManifest,
  rules: RuleMetadata[],
): ScopeError[] {
  const errors: ScopeError[] = [];

  // (a)
  for (const sourceId of [...provisionsBySource.keys()].sort(compareStrings)) {
    if (!scope.sources.has(sourceId)) {
      errors.push({ kind: "missing_source", sourceId });
    }
  }

  // (b)
  for (const [sourceId, sourceScope] of [...scope.sources.entries()].sort((a, b) =>
    compareStrings(a[0], b[0]),
  )) {
    if (sourceScope.kind !== "ranges") continue;
    const provisions = provisionsBySource.get(sourceId);
    if (!provisions) continue;
    const existing = new Set<string>();
    for (const provision of provisions) existing.add(provision.path[0]);
    for (const range of sourceScope.ranges) {
      for (const endpoint of [...new Set([range.low, range.high])].sort(compareParagrafIds)) {
        if (!existing.has(endpoint)) {
          errors.push({ kind: "bad_endpoint", sourceId, paragraf: endpoint });
        }
      }
    }
  }

  // (c)
  for (const rule of rules) {
    const sourceProvisions = provisionsBySource.get(rule.sourceId) ?? [];
    for (const citation of rule.provisions) {
      const resolved = findProvision(sourceProvisions, citation.ref);
      if (!resolved || resolved.kind !== "operative") continue;
      const paragraf = refParagraf(citation.ref);
      if (
        paragraf !== undefined &&
        !paragrafInScope(scope.sources.get(rule.sourceId), paragraf)
      ) {
        errors.push({
          kind: "citation_out_of_scope",
          ruleId: rule.ruleId,
          sourceId: rule.sourceId,
          ref: citation.ref,
        });
      }
    }
  }

  const scopeErrorKey = (error: ScopeError): string => {
    if (error.kind === "missing_source") return `0|${error.sourceId}`;
    if (error.kind === "bad_endpoint") return `1|${error.sourceId}|${error.paragraf}`;
    return `2|${error.sourceId}|${error.ruleId}|${error.ref}`;
  };
  errors.sort((a, b) => compareStrings(scopeErrorKey(a), scopeErrorKey(b)));
  return errors;
}

// Directory reads are sorted so the walk order is stable across filesystems.
function collectTsFiles(rootDir: string, dir: string): string[] {
  const out: string[] = [];
  let entries: string[];
  try {
    entries = readdirSync(dir).sort();
  } catch {
    return out;
  }
  for (const entry of entries) {
    const full = join(dir, entry);
    let isDir = false;
    try {
      isDir = statSync(full).isDirectory();
    } catch {
      continue;
    }
    if (isDir) {
      out.push(...collectTsFiles(rootDir, full));
    } else if (entry.endsWith(".ts")) {
      out.push(full.slice(rootDir.length).replace(/\\/g, "/").replace(/^\/+/, ""));
    }
  }
  return out;
}

type ScanIndex = { codeFiles: string[]; testFiles: string[] };

// Matches a rule-id-shaped token as a whole word, so DK-FOO-001 is not found
// inside DK-FOO-0011 or a longer identifier.
const RULE_ID_TOKEN_RE = /(?<![A-Za-z0-9])DK-[A-Z0-9-]+-\d{3}(?![0-9])/g;

// For every rule id, the sorted lists of files under src/ and tests/ that
// reference it. Each file is scanned once for all rule-id tokens.
function buildScanIndex(rootDir: string, ruleIds: string[]): Map<string, ScanIndex> {
  const index = new Map<string, ScanIndex>();
  for (const ruleId of ruleIds) index.set(ruleId, { codeFiles: [], testFiles: [] });
  const known = new Set(ruleIds);

  const srcFiles = collectTsFiles(rootDir, join(rootDir, "src"));
  const testFiles = collectTsFiles(rootDir, join(rootDir, "tests"));

  const scan = (relPaths: string[], target: "codeFiles" | "testFiles") => {
    for (const rel of relPaths) {
      let text: string;
      try {
        text = readFileSync(join(rootDir, rel), "utf8");
      } catch {
        continue;
      }
      const seen = new Set<string>();
      for (const match of text.matchAll(RULE_ID_TOKEN_RE)) {
        const token = match[0];
        if (known.has(token) && !seen.has(token)) {
          seen.add(token);
          index.get(token)![target].push(rel);
        }
      }
    }
  };
  scan(srcFiles, "codeFiles");
  scan(testFiles, "testFiles");
  return index;
}

export function computeRegulatoryCoverage(rootDir: string = repoRoot()): RegulatoryCoverage {
  const provisionsBySource = loadAllProvisions(rootDir);
  const scope = loadScopeManifest(rootDir);
  const rules = readRuleMetadata();
  const ruleIds = [...new Set(rules.map((rule) => rule.ruleId))].sort(compareStrings);
  const scanIndex = buildScanIndex(rootDir, ruleIds);

  const closureErrors: ClosureError[] = [];
  const driftErrors: DriftError[] = [];
  // (a)(b)(c) — the three scope hard-error checks. See evaluateScope.
  const scopeErrors = evaluateScope(provisionsBySource, scope, rules);
  const uncitedRules: string[] = [];

  // (sourceId -> (ref -> set of rule ids)) for citations that resolve cleanly.
  const citationsBySource = new Map<string, Map<string, Set<string>>>();
  const addCitation = (sourceId: string, ref: string, ruleId: string) => {
    let byRef = citationsBySource.get(sourceId);
    if (!byRef) {
      byRef = new Map();
      citationsBySource.set(sourceId, byRef);
    }
    let ruleSet = byRef.get(ref);
    if (!ruleSet) {
      ruleSet = new Set();
      byRef.set(ref, ruleSet);
    }
    ruleSet.add(ruleId);
  };

  for (const rule of rules) {
    if (rule.provisions.length === 0) {
      uncitedRules.push(rule.ruleId);
      continue;
    }
    const sourceProvisions = provisionsBySource.get(rule.sourceId) ?? [];
    for (const citation of rule.provisions) {
      const resolved = findProvision(sourceProvisions, citation.ref);
      if (!resolved) {
        // Not in the rule's declared source — check whether it resolves
        // anywhere else, which is the more specific (cross-source) error.
        let crossSource: string | undefined;
        for (const [otherSourceId, provs] of [...provisionsBySource.entries()].sort((a, b) =>
          compareStrings(a[0], b[0]),
        )) {
          if (otherSourceId === rule.sourceId) continue;
          if (findProvision(provs, citation.ref)) {
            crossSource = otherSourceId;
            break;
          }
        }
        closureErrors.push(
          crossSource
            ? {
                ruleId: rule.ruleId,
                sourceId: rule.sourceId,
                ref: citation.ref,
                reason: "cross_source",
                resolvedSourceId: crossSource,
              }
            : {
                ruleId: rule.ruleId,
                sourceId: rule.sourceId,
                ref: citation.ref,
                reason: "unresolved",
              },
        );
        continue;
      }
      if (resolved.textHash !== citation.textHash) {
        driftErrors.push({
          ruleId: rule.ruleId,
          sourceId: rule.sourceId,
          ref: citation.ref,
          citedTextHash: citation.textHash,
          currentTextHash: resolved.textHash,
        });
      }
      // A citation contributes to coverage once it resolves to the rule's
      // declared source, independent of drift — drift is reported separately.
      addCitation(rule.sourceId, citation.ref, rule.ruleId);
    }
  }
  uncitedRules.sort(compareStrings);
  closureErrors.sort(
    (a, b) =>
      compareStrings(a.ruleId, b.ruleId) ||
      compareStrings(a.sourceId, b.sourceId) ||
      compareStrings(a.ref, b.ref),
  );
  driftErrors.sort(
    (a, b) =>
      compareStrings(a.ruleId, b.ruleId) ||
      compareStrings(a.sourceId, b.sourceId) ||
      compareStrings(a.ref, b.ref),
  );

  const perSource: SourceCoverage[] = [];
  const uncoveredProvisions: UncoveredProvision[] = [];
  const reverseMap: ReverseMapRecord[] = [];
  let overallOperative = 0;
  let overallCited = 0;
  let overallInScopeOperative = 0;
  let overallInScopeCited = 0;

  const buildReverseEntries = (ruleSet: Set<string>): ReverseMapEntry[] =>
    [...ruleSet].sort(compareStrings).map((ruleId) => {
      const scan = scanIndex.get(ruleId) ?? { codeFiles: [], testFiles: [] };
      return {
        ruleId,
        codeFiles: [...scan.codeFiles].sort(compareStrings),
        testFiles: [...scan.testFiles].sort(compareStrings),
      };
    });

  for (const [sourceId, provisions] of [...provisionsBySource.entries()].sort((a, b) =>
    compareStrings(a[0], b[0]),
  )) {
    const operative = provisions.filter((p: Provision) => p.kind === "operative");
    const byRef = citationsBySource.get(sourceId) ?? new Map<string, Set<string>>();
    const sourceScope = scope.sources.get(sourceId);
    const coveredRefs: string[] = [];
    const uncoveredRefs: string[] = [];
    let inScopeOperative = 0;
    let inScopeCited = 0;
    for (const provision of operative) {
      const inScope = paragrafInScope(sourceScope, provision.path[0]);
      if (inScope) inScopeOperative += 1;
      if (byRef.has(provision.ref)) {
        coveredRefs.push(provision.ref);
        if (inScope) inScopeCited += 1;
      } else {
        uncoveredRefs.push(provision.ref);
        uncoveredProvisions.push({ sourceId, ref: provision.ref });
      }
    }
    coveredRefs.sort(compareStrings);
    uncoveredRefs.sort(compareStrings);
    perSource.push({
      sourceId,
      operativeCount: operative.length,
      citedCount: coveredRefs.length,
      inScopeOperativeCount: inScopeOperative,
      inScopeCitedCount: inScopeCited,
      coveredRefs,
      uncoveredRefs,
    });
    overallOperative += operative.length;
    overallCited += coveredRefs.length;
    overallInScopeOperative += inScopeOperative;
    overallInScopeCited += inScopeCited;

    for (const [ref, ruleSet] of [...byRef.entries()].sort((a, b) =>
      compareStrings(a[0], b[0]),
    )) {
      reverseMap.push({ sourceId, ref, rules: buildReverseEntries(ruleSet) });
    }
  }

  reverseMap.sort(
    (a, b) => compareStrings(a.sourceId, b.sourceId) || compareStrings(a.ref, b.ref),
  );

  return {
    overall: {
      operativeCount: overallOperative,
      citedCount: overallCited,
      inScopeOperativeCount: overallInScopeOperative,
      inScopeCitedCount: overallInScopeCited,
    },
    perSource,
    closureErrors,
    driftErrors,
    scopeErrors,
    uncitedRules,
    uncoveredProvisions,
    reverseMap,
  };
}

function fraction(numerator: number, denominator: number): string {
  if (denominator === 0) return "n/a (0 operative provisions)";
  return `${numerator}/${denominator}`;
}

// Deterministic Markdown report — a pure function of repo state, no timestamps,
// so it can be committed and diffed.
export function renderRegulatoryCoverageReport(coverage: RegulatoryCoverage): string {
  const lines: string[] = [];
  lines.push("# Regulatory coverage");
  lines.push("");
  lines.push(
    "Generated by `rentemester reg coverage`. This report is a deterministic " +
      "function of the rule citations and the legal-source corpus — no timestamps.",
  );
  lines.push("");
  const outOfScopeOperative =
    coverage.overall.operativeCount - coverage.overall.inScopeOperativeCount;
  lines.push(
    `In-scope operative-provision coverage: ${fraction(
      coverage.overall.inScopeCitedCount,
      coverage.overall.inScopeOperativeCount,
    )}`,
  );
  lines.push("");
  lines.push(
    "**The headline metric is a self-attestation.** Its denominator is the set " +
      "of provisions declared in scope in `sources/scope.yaml` — narrowing that " +
      "scope raises the percentage. Read it together with the raw corpus-wide " +
      `figure (${fraction(
        coverage.overall.citedCount,
        coverage.overall.operativeCount,
      )} operative provisions, ${outOfScopeOperative} out of scope) and review ` +
      "`sources/scope.yaml` itself; the closure checks only guarantee that no " +
      "cited provision falls outside the declared scope.",
  );
  lines.push("");
  lines.push(`Closure errors: ${coverage.closureErrors.length}`);
  lines.push(`Drift errors: ${coverage.driftErrors.length}`);
  lines.push(`Scope errors: ${coverage.scopeErrors.length}`);
  lines.push(`Uncited rules: ${coverage.uncitedRules.length}`);
  lines.push("");

  lines.push("## Coverage per source");
  lines.push("");
  lines.push(
    "| Source | In-scope operative | In-scope cited | In-scope coverage | " +
      "All operative | All cited |",
  );
  lines.push("| --- | --- | --- | --- | --- | --- |");
  for (const source of coverage.perSource) {
    lines.push(
      `| ${source.sourceId} | ${source.inScopeOperativeCount} | ` +
        `${source.inScopeCitedCount} | ${fraction(
          source.inScopeCitedCount,
          source.inScopeOperativeCount,
        )} | ${source.operativeCount} | ${source.citedCount} |`,
    );
  }
  lines.push("");

  if (coverage.closureErrors.length > 0) {
    lines.push("## Closure errors");
    lines.push("");
    for (const error of coverage.closureErrors) {
      const detail =
        error.reason === "cross_source"
          ? `resolves in ${error.resolvedSourceId}, not the rule's source`
          : "does not resolve to any provision";
      lines.push(`- ${error.ruleId} (${error.sourceId}) — \`${error.ref}\`: ${detail}`);
    }
    lines.push("");
  }

  if (coverage.driftErrors.length > 0) {
    lines.push("## Drift errors");
    lines.push("");
    for (const error of coverage.driftErrors) {
      lines.push(
        `- ${error.ruleId} (${error.sourceId}) — \`${error.ref}\`: cited ` +
          `${error.citedTextHash} but current is ${error.currentTextHash}`,
      );
    }
    lines.push("");
  }

  if (coverage.scopeErrors.length > 0) {
    lines.push("## Scope errors");
    lines.push("");
    for (const error of coverage.scopeErrors) {
      if (error.kind === "missing_source") {
        lines.push(`- ${error.sourceId}: downloaded source missing from scope.yaml`);
      } else if (error.kind === "bad_endpoint") {
        lines.push(
          `- ${error.sourceId}: range endpoint \`§ ${error.paragraf}\` is not a ` +
            "paragraf in this source",
        );
      } else {
        lines.push(
          `- ${error.ruleId} (${error.sourceId}) — \`${error.ref}\`: cited ` +
            "provision is outside the declared scope",
        );
      }
    }
    lines.push("");
  }

  lines.push("## Covered and uncovered provisions per source");
  lines.push("");
  const rulesByKey = new Map<string, string[]>();
  for (const record of coverage.reverseMap) {
    rulesByKey.set(
      `${record.sourceId} ${record.ref}`,
      record.rules.map((entry) => entry.ruleId).sort(compareStrings),
    );
  }
  for (const source of coverage.perSource) {
    lines.push(`### ${source.sourceId}`);
    lines.push("");
    if (source.coveredRefs.length === 0) {
      lines.push("_No operative provisions cited yet._");
    } else {
      lines.push("Covered:");
      for (const ref of source.coveredRefs) {
        const ruleIds = rulesByKey.get(`${source.sourceId} ${ref}`) ?? [];
        lines.push(`- \`${ref}\` — ${ruleIds.join(", ")}`);
      }
    }
    lines.push("");
    if (source.uncoveredRefs.length > 0) {
      lines.push(`Uncovered operative provisions (${source.uncoveredRefs.length}):`);
      for (const ref of source.uncoveredRefs) lines.push(`- \`${ref}\``);
      lines.push("");
    }
  }

  if (coverage.reverseMap.length > 0) {
    lines.push("## Traceability (provision → rule → code → tests)");
    lines.push("");
    let currentSource = "";
    for (const record of coverage.reverseMap) {
      if (record.sourceId !== currentSource) {
        currentSource = record.sourceId;
        lines.push(`### ${currentSource}`);
        lines.push("");
      }
      lines.push(`- \`${record.ref}\``);
      for (const entry of record.rules) {
        const code = entry.codeFiles.length > 0 ? entry.codeFiles.join(", ") : "—";
        const tests = entry.testFiles.length > 0 ? entry.testFiles.join(", ") : "—";
        lines.push(`  - ${entry.ruleId} — code: ${code} — tests: ${tests}`);
      }
    }
    lines.push("");
  }

  return lines.join("\n").replace(/\n+$/, "\n");
}

// Deterministic Markdown review aid for `rentemester reg citations`. For every
// rule that carries citations, it prints the rule, its name and explanation,
// and the verbatim statutory text of each cited provision — so the project
// owner can check by eye that a rule is mapped to the right paragraph. A pure
// function of repo state, no timestamps.
export function renderRegulatoryCitationsReview(
  rootDir: string = repoRoot(),
): string {
  const provisionsBySource = loadAllProvisions(rootDir);
  const rules = readRuleMetadata()
    .filter((rule) => rule.provisions.length > 0)
    .sort((a, b) => compareStrings(a.ruleId, b.ruleId));

  const lines: string[] = [];
  lines.push("# Regulatory citation review");
  lines.push("");
  lines.push(
    "Generated by `rentemester reg citations`. For every rule that cites a " +
      "statutory provision, this lists the rule and the verbatim legislation " +
      "text it is mapped to, so the mapping can be reviewed by eye. " +
      "Deterministic — no timestamps.",
  );
  lines.push("");

  for (const rule of rules) {
    lines.push(`## ${rule.ruleId}`);
    lines.push("");
    lines.push(`- Source: ${rule.sourceId}`);
    if (rule.name) lines.push(`- Name: ${rule.name}`);
    if (rule.explanation) lines.push(`- Explanation: ${rule.explanation}`);
    lines.push("");
    const sourceProvisions = provisionsBySource.get(rule.sourceId) ?? [];
    for (const citation of rule.provisions) {
      lines.push(`### \`${citation.ref}\``);
      lines.push("");
      const resolved = findProvision(sourceProvisions, citation.ref);
      if (!resolved) {
        lines.push("_Does not resolve in the declared source (closure error)._");
      } else {
        lines.push("> " + (resolved.text.length > 0 ? resolved.text : "(empty)"));
      }
      lines.push("");
    }
  }

  return lines.join("\n").replace(/\n+$/, "\n");
}
