import { migrate } from "../core/db";
import { listExceptions, resolveException } from "../core/exceptions";
import { openCommandDb } from "../cli-dispatch";
import type { CommandDispatch } from "../cli-dispatch";
import { renderHumanReport } from "../cli-format";

export function register(dispatch: CommandDispatch): void {
  dispatch.on("exceptions", "list", (ctx) => {
    const db = openCommandDb(ctx);
    migrate(db);
    const result = listExceptions(db, {
      status: (ctx.arg("--status") as any) ?? undefined,
      includeArchived: ctx.hasFlag("--include-archived"),
    });
    if (ctx.outputFormat === "json") {
      ctx.emitResult(result as Record<string, unknown>);
    } else if (result.ok) {
      const human = renderHumanReport("exceptions-list", result as Record<string, unknown>);
      console.log(human ?? "");
    } else {
      console.error(result.errors.join("\n"));
    }
    db.close();
    if (!result.ok) process.exit(1);
  });

  dispatch.on("exceptions", "resolve", (ctx) => {
    const id = Number(ctx.arg("--id"));
    if (!Number.isInteger(id) || id <= 0) {
      console.error("Missing required --id <n>");
      process.exit(2);
    }
    const db = openCommandDb(ctx);
    migrate(db);
    const result = resolveException(db, {
      id,
      note: ctx.arg("--note") ?? undefined,
      resolvedBy:
        ctx.cliActor ?? process.env.RENTEMESTER_ACTOR ?? ctx.inferredMutationActor() ?? null,
    });
    ctx.emitResult(result as Record<string, unknown>);
    db.close();
    if (!result.ok) process.exit(1);
  });
}
