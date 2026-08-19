import { migrate } from "../core/db";
import {
  buildMileagePeriodReport,
  createMileageEntry,
  exportMileageLog,
  listMileageEntries,
} from "../core/mileage";
import { openCommandDb } from "../cli-dispatch";
import type { CommandDispatch } from "../cli-dispatch";

/**
 * `mileage` CLI — kørselsregnskab register + period report (#123).
 *
 * Missing/invalid flags are NOT pre-checked here: each handler forwards the
 * raw arguments to the core, which validates them and returns a structured
 * `{ ok: false, errors }` result. That keeps the failure mode deterministic
 * (exit code 1 + machine-readable errors) instead of a bare exit 2.
 */
export function register(dispatch: CommandDispatch): void {
  dispatch.on("mileage", "log", (ctx) => {
    const db = openCommandDb(ctx);
    migrate(db);
    const kmRaw = ctx.arg("--km");
    const rateRaw = ctx.arg("--rate-per-km");
    const result = createMileageEntry(db, {
      tripDate: ctx.arg("--date") ?? "",
      purpose: ctx.arg("--purpose") ?? "",
      fromLocation: ctx.arg("--from") ?? "",
      toLocation: ctx.arg("--to") ?? "",
      kilometers: kmRaw === undefined ? Number.NaN : Number(kmRaw),
      vehicle: ctx.arg("--vehicle") ?? "",
      driver: ctx.arg("--driver") ?? "",
      ratePerKm: rateRaw === undefined ? Number.NaN : Number(rateRaw),
      rateBasis: ctx.arg("--rate-basis") ?? "",
      rateSource: ctx.arg("--rate-source") ?? undefined,
      notes: ctx.arg("--notes") ?? undefined,
    });
    ctx.emitResult(result as Record<string, unknown>);
    db.close();
    if (!result.ok) process.exit(1);
  });

  dispatch.on("mileage", "list", (ctx) => {
    const db = openCommandDb(ctx);
    migrate(db);
    const result = listMileageEntries(db);
    ctx.emitResult(result as Record<string, unknown>);
    db.close();
  });

  dispatch.on("mileage", "report", (ctx) => {
    const db = openCommandDb(ctx);
    migrate(db);
    const result = buildMileagePeriodReport(db, {
      from: ctx.arg("--from") ?? "",
      to: ctx.arg("--to") ?? "",
    });
    ctx.emitResult(result as Record<string, unknown>);
    db.close();
    if (!result.ok) process.exit(1);
  });

  dispatch.on("mileage", "export", (ctx) => {
    const db = openCommandDb(ctx);
    migrate(db);
    const result = exportMileageLog(db, {
      from: ctx.arg("--from") ?? "",
      to: ctx.arg("--to") ?? "",
      outputDir: ctx.arg("--out") ?? "",
    });
    ctx.emitResult(result as Record<string, unknown>);
    db.close();
    if (!result.ok) process.exit(1);
  });
}
