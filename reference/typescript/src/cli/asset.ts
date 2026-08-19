import { migrate } from "../core/db";
import {
  registerAsset,
  postDepreciationPeriod,
  postImmediateWriteOff,
  buildAssetRegisterReport,
} from "../core/assets";
import { openCommandDb } from "../cli-dispatch";
import type { CommandDispatch } from "../cli-dispatch";

export function register(dispatch: CommandDispatch): void {
  dispatch.on("asset", "register", (ctx) => {
    const name = ctx.arg("--name");
    const category = ctx.arg("--category");
    const acquisitionDate = ctx.arg("--acquisition-date");
    const cost = Number(ctx.arg("--cost"));
    const usefulLifeMonths = Number(ctx.arg("--useful-life-months"));
    const purchaseDocumentId = Number(ctx.arg("--document-id"));
    if (
      !name ||
      !category ||
      !acquisitionDate ||
      !Number.isFinite(cost) ||
      cost <= 0 ||
      !Number.isInteger(usefulLifeMonths) ||
      usefulLifeMonths <= 0 ||
      !Number.isInteger(purchaseDocumentId) ||
      purchaseDocumentId <= 0
    ) {
      console.error(
        "Missing required --name <text> --category <text> --acquisition-date <YYYY-MM-DD> --cost <n> --useful-life-months <n> --document-id <n>",
      );
      process.exit(2);
    }
    const db = openCommandDb(ctx);
    migrate(db);
    const result = registerAsset(db, {
      name,
      category,
      acquisitionDate,
      cost,
      usefulLifeMonths,
      purchaseDocumentId,
      assetAccountNo: ctx.arg("--asset-account") ?? undefined,
      depreciationExpenseAccountNo: ctx.arg("--depreciation-account") ?? undefined,
      accumulatedDepreciationAccountNo: ctx.arg("--accumulated-account") ?? undefined,
      note: ctx.arg("--note") ?? undefined,
    });
    ctx.emitResult(result as Record<string, unknown>);
    db.close();
    if (!result.ok) process.exit(1);
  });

  dispatch.on("asset", "depreciate", (ctx) => {
    const assetId = Number(ctx.arg("--asset-id"));
    const periodIndex = Number(ctx.arg("--period"));
    const transactionDate = ctx.arg("--date");
    if (
      !Number.isInteger(assetId) ||
      assetId <= 0 ||
      !Number.isInteger(periodIndex) ||
      periodIndex <= 0 ||
      !transactionDate
    ) {
      console.error("Missing required --asset-id <n> --period <n> --date <YYYY-MM-DD>");
      process.exit(2);
    }
    const db = openCommandDb(ctx);
    migrate(db);
    const result = postDepreciationPeriod(db, { assetId, periodIndex, transactionDate });
    ctx.emitResult(result as Record<string, unknown>);
    db.close();
    if (!result.ok) process.exit(1);
  });

  dispatch.on("asset", "write-off", (ctx) => {
    const name = ctx.arg("--name");
    const category = ctx.arg("--category");
    const acquisitionDate = ctx.arg("--acquisition-date");
    const cost = Number(ctx.arg("--cost"));
    const purchaseDocumentId = Number(ctx.arg("--document-id"));
    const expenseAccountNo = ctx.arg("--expense-account");
    const transactionDate = ctx.arg("--date");
    const thresholdRuleSource = ctx.arg("--threshold-source");
    if (
      !name ||
      !category ||
      !acquisitionDate ||
      !Number.isFinite(cost) ||
      cost <= 0 ||
      !Number.isInteger(purchaseDocumentId) ||
      purchaseDocumentId <= 0 ||
      !expenseAccountNo ||
      !transactionDate ||
      !thresholdRuleSource
    ) {
      console.error(
        "Missing required --name <text> --category <text> --acquisition-date <YYYY-MM-DD> --cost <n> --document-id <n> --expense-account <konto> --date <YYYY-MM-DD> --threshold-source <text>",
      );
      process.exit(2);
    }
    // `--confirm` is a valued flag (--confirm yes) rather than a bare boolean:
    // the shared cli-args BOOLEAN_FLAGS set is append-only and must not be
    // modified. Only the literal "yes" confirms the straksafskrivning.
    const confirmValue = (ctx.arg("--confirm") ?? "").trim().toLowerCase();
    const db = openCommandDb(ctx);
    migrate(db);
    const result = postImmediateWriteOff(db, {
      name,
      category,
      acquisitionDate,
      cost,
      purchaseDocumentId,
      expenseAccountNo,
      transactionDate,
      confirmImmediateWriteOff: confirmValue === "yes",
      thresholdRuleSource,
      paymentAccountNo: ctx.arg("--payment-account") ?? undefined,
      note: ctx.arg("--note") ?? undefined,
    });
    ctx.emitResult(result as Record<string, unknown>);
    db.close();
    if (!result.ok) process.exit(1);
  });

  dispatch.on("asset", "register-report", (ctx) => {
    const db = openCommandDb(ctx);
    migrate(db);
    const result = buildAssetRegisterReport(db);
    ctx.emitResult(result as Record<string, unknown>);
    db.close();
  });
}
