import { migrate } from "../core/db";
import {
  createCustomer,
  listCustomers,
  customerInputFromCvr,
  type CreateCustomerInput,
} from "../core/master-data";
import { validateVatAgainstVies } from "../core/vies";
import { lookupCvrCompany } from "../core/cvr";
import { openCommandDb } from "../cli-dispatch";
import { renderContactList } from "../cli-format";
import type { CommandDispatch } from "../cli-dispatch";

export function register(dispatch: CommandDispatch): void {
  dispatch.on("customer", "create", async (ctx) => {
    const db = openCommandDb(ctx);
    migrate(db);
    const paymentTermsRaw = ctx.arg("--payment-terms");
    const paymentTerms = paymentTermsRaw === undefined ? undefined : Number(paymentTermsRaw);
    const base: CreateCustomerInput = {
      name: ctx.arg("--name") ?? "",
      address: ctx.arg("--address") ?? undefined,
      vatOrCvr: ctx.arg("--cvr") ?? undefined,
      email: ctx.arg("--email") ?? undefined,
      eanNumber: ctx.arg("--ean") ?? undefined,
      paymentTermsDays: Number.isFinite(paymentTerms) ? paymentTerms : undefined,
      defaultCurrency: ctx.arg("--currency") ?? undefined,
      notes: ctx.arg("--notes") ?? undefined,
    };

    // --from-cvr fills any field the caller left unset from the CVR register.
    let input = base;
    const fromCvr = ctx.arg("--from-cvr");
    if (fromCvr) {
      const resolved = await customerInputFromCvr(db, fromCvr, base);
      if (!resolved.ok) {
        ctx.emitResult({ ok: false, errors: resolved.errors });
        db.close();
        process.exit(1);
      }
      input = resolved.input;
    }

    const result = createCustomer(db, input);
    ctx.emitResult(result as Record<string, unknown>);
    db.close();
    if (!result.ok) process.exit(1);
  });

  dispatch.on("customer", "list", (ctx) => {
    const db = openCommandDb(ctx);
    migrate(db);
    const result = listCustomers(db, { archived: ctx.hasFlag("--archived") });
    if (ctx.outputFormat === "json") {
      ctx.emitResult(result as Record<string, unknown>);
    } else {
      console.log(
        renderContactList(result as Record<string, unknown>, "Kunder", "Ingen kunder registreret."),
      );
    }
    db.close();
  });

  dispatch.on("customer", "validate-vat", async (ctx) => {
    const cvr = ctx.arg("--cvr");
    if (!cvr) {
      console.error("Missing required --cvr <EU-VAT>");
      process.exit(2);
    }
    const db = openCommandDb(ctx);
    migrate(db);
    const result = await validateVatAgainstVies(db, cvr);
    ctx.emitResult(result as Record<string, unknown>);
    db.close();
    if (!result.ok) process.exit(1);
  });

  dispatch.on("customer", "cvr-lookup", async (ctx) => {
    const cvr = ctx.arg("--cvr");
    if (!cvr) {
      console.error("Missing required --cvr <CVR-nummer>");
      process.exit(2);
    }
    const db = openCommandDb(ctx);
    migrate(db);
    const result = await lookupCvrCompany(db, cvr);
    ctx.emitResult(result as unknown as Record<string, unknown>);
    db.close();
    if (!result.ok) process.exit(1);
  });
}
