import { migrate } from "../core/db";
import { verifyAuditChain } from "../core/ledger";
import { openCommandDb } from "../cli-dispatch";
import type { CommandDispatch } from "../cli-dispatch";

export function register(dispatch: CommandDispatch): void {
  dispatch.on("audit", "verify", (ctx) => {
    const db = openCommandDb(ctx);
    migrate(db);
    const result = verifyAuditChain(db);
    ctx.emitResult(result as Record<string, unknown>);
    db.close();
  });
}
