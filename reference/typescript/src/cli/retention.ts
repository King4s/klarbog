import { migrate } from "../core/db";
import { buildRetentionStatusReport } from "../core/retention";
import { openCommandDb } from "../cli-dispatch";
import type { CommandDispatch } from "../cli-dispatch";
import { emitHumanReport } from "../cli-format";

export function register(dispatch: CommandDispatch): void {
  dispatch.on("retention", "status", (ctx) => {
    const db = openCommandDb(ctx);
    migrate(db);
    const result = buildRetentionStatusReport(db, ctx.arg("--as-of"));
    emitHumanReport("retention-status", result as Record<string, unknown>, ctx.outputFormat);
    db.close();
  });
}
