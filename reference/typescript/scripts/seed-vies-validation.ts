#!/usr/bin/env bun
import { openDb, migrate } from "../src/core/db";
import { companyPaths } from "../src/core/paths";
import { storeViesValidation } from "../src/core/vies";

const [, , companyRoot, vatOrCvr] = Bun.argv;
if (!companyRoot || !vatOrCvr) {
  console.error("Usage: bun run scripts/seed-vies-validation.ts <company-root> <EU-VAT>");
  process.exit(2);
}

const db = openDb(companyPaths(companyRoot).db);
migrate(db);
const validation = storeViesValidation(db, {
  vatOrCvr,
  valid: true,
  validatedAt: "2026-05-16T00:00:00.000Z",
  expiresAt: "2026-08-16T00:00:00.000Z",
  rawResponse: JSON.stringify({ valid: true, source: "smoke-seed" }),
});
db.close();
console.log(JSON.stringify({ ok: true, validation }, null, 2));
