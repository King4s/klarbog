import { writeFileSync } from "node:fs";
import {
  computeRegulatoryCoverage,
  renderRegulatoryCitationsReview,
  renderRegulatoryCoverageReport,
} from "../core/regulatory-coverage";
import type { CommandDispatch } from "../cli-dispatch";

// `reg coverage` — regulatory coverage; repo-static, needs no --company.
export function register(dispatch: CommandDispatch): void {
  dispatch.on("reg", "coverage", (ctx) => {
    const coverage = computeRegulatoryCoverage();
    const out = ctx.trimToNull(ctx.arg("--out"));
    if (out) {
      writeFileSync(out, renderRegulatoryCoverageReport(coverage), "utf8");
    }

    const result: Record<string, unknown> = {
      ok:
        coverage.closureErrors.length === 0 &&
        coverage.driftErrors.length === 0 &&
        coverage.scopeErrors.length === 0,
      inScopeOperativeProvisions: coverage.overall.inScopeOperativeCount,
      inScopeCitedProvisions: coverage.overall.inScopeCitedCount,
      operativeProvisions: coverage.overall.operativeCount,
      citedProvisions: coverage.overall.citedCount,
      closureErrors: coverage.closureErrors.length,
      driftErrors: coverage.driftErrors.length,
      scopeErrors: coverage.scopeErrors.length,
      uncitedRules: coverage.uncitedRules.length,
      perSource: coverage.perSource.map((source) => ({
        sourceId: source.sourceId,
        inScopeOperativeProvisions: source.inScopeOperativeCount,
        inScopeCitedProvisions: source.inScopeCitedCount,
        operativeProvisions: source.operativeCount,
        citedProvisions: source.citedCount,
      })),
    };
    if (out) result.reportPath = out;
    if (
      coverage.closureErrors.length > 0 ||
      coverage.driftErrors.length > 0 ||
      coverage.scopeErrors.length > 0
    ) {
      result.errors = [
        ...coverage.closureErrors.map(
          (error) =>
            `closure: ${error.ruleId} (${error.sourceId}) cites unresolved ${error.ref}`,
        ),
        ...coverage.driftErrors.map(
          (error) =>
            `drift: ${error.ruleId} (${error.sourceId}) cites stale ${error.ref}`,
        ),
        ...coverage.scopeErrors.map((error) => {
          if (error.kind === "missing_source") {
            return `scope: ${error.sourceId} missing from scope.yaml`;
          }
          if (error.kind === "bad_endpoint") {
            return `scope: ${error.sourceId} range endpoint § ${error.paragraf} does not exist`;
          }
          return `scope: ${error.ruleId} (${error.sourceId}) cites out-of-scope ${error.ref}`;
        }),
      ];
    }

    if (ctx.outputFormat === "json") {
      ctx.emitResult(result);
      return;
    }

    const ok = result.ok === true;
    const lines: string[] = [];
    lines.push(`${ok ? "✔" : "✘"} reg coverage`);
    lines.push(
      `  In-scope provisions cited: ${coverage.overall.inScopeCitedCount}/` +
        `${coverage.overall.inScopeOperativeCount}`,
    );
    lines.push(
      `  Corpus-wide (incl. out of scope): ${coverage.overall.citedCount}/` +
        `${coverage.overall.operativeCount}`,
    );
    lines.push(`  Closure errors: ${coverage.closureErrors.length}`);
    lines.push(`  Drift errors: ${coverage.driftErrors.length}`);
    lines.push(`  Scope errors: ${coverage.scopeErrors.length}`);
    lines.push(`  Uncited rules: ${coverage.uncitedRules.length}`);
    if (out) lines.push(`  Report: ${out}`);
    lines.push("  Per source (in-scope cited / in-scope operative):");
    for (const source of coverage.perSource) {
      lines.push(
        `    ${source.sourceId}: ${source.inScopeCitedCount}/` +
          `${source.inScopeOperativeCount}`,
      );
    }
    for (const error of (result.errors as string[] | undefined) ?? []) {
      lines.push(`  → ${error}`);
    }
    const text = lines.join("\n");
    if (ok) console.log(text);
    else {
      console.error(text);
      process.exitCode = 1;
    }
  });

  // `reg citations` — a deterministic Markdown review aid mapping every cited
  // rule to the verbatim statutory text of its provisions; repo-static.
  dispatch.on("reg", "citations", (ctx) => {
    const review = renderRegulatoryCitationsReview();
    const out = ctx.trimToNull(ctx.arg("--out"));
    if (out) {
      writeFileSync(out, review, "utf8");
      if (ctx.outputFormat === "json") {
        ctx.emitResult({ ok: true, reportPath: out });
      } else {
        console.log(`✔ reg citations\n  Review: ${out}`);
      }
      return;
    }
    if (ctx.outputFormat === "json") {
      ctx.emitResult({ ok: true, review });
      return;
    }
    console.log(review);
  });
}
