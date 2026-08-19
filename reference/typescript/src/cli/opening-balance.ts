import { readFileSync } from "node:fs";
import { migrate } from "../core/db";
import { postOpeningBalance } from "../core/opening-balance";
import { openCommandDb } from "../cli-dispatch";
import type { CommandDispatch } from "../cli-dispatch";

// `opening-balance post` — establishes a company's opening balance
// (primobalance) from a JSON payload. See src/core/opening-balance.ts (#179).
export function register(dispatch: CommandDispatch): void {
  dispatch.on("opening-balance", "post", (ctx) => {
    const input = ctx.arg("--input");
    if (!input) {
      console.error("Missing required --input <file.json>");
      process.exit(2);
    }
    const db = openCommandDb(ctx);
    migrate(db);
    const payload = JSON.parse(readFileSync(input, "utf8"));
    const result = postOpeningBalance(db, payload);
    ctx.emitResult(result as Record<string, unknown>);
    db.close();
    if (!result.ok) process.exit(1);
  });
}
