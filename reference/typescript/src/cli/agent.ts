/**
 * CLI for the runtime bookkeeper agent (#183).
 *
 *  - `agent run` — runs one deterministic, replayable bookkeeping loop for a
 *    company: ingest bilag → book the unambiguous → route the uncertain to
 *    the exception queue → reconcile bank → check VAT / year-end deadlines →
 *    print an end-of-run report.
 *
 * The agent never overrules the ledger or the rules; anything uncertain
 * becomes an exception, not a guess. See docs/runtime-agent-contract.md.
 */

import { resolve } from "node:path";
import { runAgentLoop } from "../agent/loop";
import { formatRunReport } from "../agent/run";
import type { CommandDispatch } from "../cli-dispatch";

export function register(dispatch: CommandDispatch): void {
  dispatch.on("agent", "run", (ctx) => {
    const asOf = ctx.arg("--as-of");
    if (!asOf) {
      console.error("Missing required --as-of <YYYY-MM-DD>");
      process.exit(2);
    }
    const inboxArg = ctx.arg("--inbox");
    const metadataArg = ctx.arg("--metadata-dir");
    const bankArg = ctx.arg("--bank-csv");

    const report = runAgentLoop({
      companyRoot: ctx.companyRoot(),
      asOf,
      inboxDir: inboxArg ? resolve(inboxArg) : undefined,
      metadataDir: metadataArg ? resolve(metadataArg) : undefined,
      bankCsvPath: bankArg ? resolve(bankArg) : undefined,
    });

    if (ctx.outputFormat === "json") {
      ctx.emitResult(report as unknown as Record<string, unknown>);
    } else {
      console.log(formatRunReport(report));
      if (!report.ok) process.exitCode = 1;
    }
  });
}
