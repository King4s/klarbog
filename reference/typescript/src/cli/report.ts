// Financial statements (#176): the `report` CLI command group.
//
// `report trial-balance|profit-loss --company <c> --from --to` and
// `report balance --company <c> --as-of` emit the deterministic JSON produced
// by src/core/financial-statements.ts. The same core functions feed the
// dashboard and the cockpit API; this command is a thin CLI wrapper.

import { migrate } from "../core/db";
import {
  buildBalanceSheet,
  buildProfitAndLoss,
  buildTrialBalance,
} from "../core/financial-statements";
import { openCommandDb } from "../cli-dispatch";
import type { CommandDispatch } from "../cli-dispatch";
import { emitHumanReport } from "../cli-format";

export function register(dispatch: CommandDispatch): void {
  dispatch.on("report", "trial-balance", (ctx) => {
    const from = ctx.arg("--from");
    const to = ctx.arg("--to");
    if (!from || !to) {
      console.error("Missing required --from <YYYY-MM-DD> or --to <YYYY-MM-DD>");
      process.exit(2);
    }
    const db = openCommandDb(ctx);
    migrate(db);
    const result = buildTrialBalance(db, from, to);
    emitHumanReport("report-trial-balance", result as unknown as Record<string, unknown>, ctx.outputFormat);
    db.close();
  });

  dispatch.on("report", "profit-loss", (ctx) => {
    const from = ctx.arg("--from");
    const to = ctx.arg("--to");
    if (!from || !to) {
      console.error("Missing required --from <YYYY-MM-DD> or --to <YYYY-MM-DD>");
      process.exit(2);
    }
    const db = openCommandDb(ctx);
    migrate(db);
    const result = buildProfitAndLoss(db, from, to);
    emitHumanReport("report-profit-loss", result as unknown as Record<string, unknown>, ctx.outputFormat);
    db.close();
  });

  dispatch.on("report", "balance", (ctx) => {
    const asOf = ctx.arg("--as-of");
    if (!asOf) {
      console.error("Missing required --as-of <YYYY-MM-DD>");
      process.exit(2);
    }
    const db = openCommandDb(ctx);
    migrate(db);
    const result = buildBalanceSheet(db, asOf);
    emitHumanReport("report-balance", result as unknown as Record<string, unknown>, ctx.outputFormat);
    db.close();
  });
}
