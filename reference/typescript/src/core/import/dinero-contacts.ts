/**
 * Dinero "Kontakter" CSV import.
 *
 * Dinero exports its contact book as a semicolon-delimited CSV (UTF-8 with a
 * BOM). This module parses that file and lands each contact in Rentemester's
 * `customers` / `vendors` master data, optionally enriched from the CVR
 * register for Danish companies.
 *
 * Design:
 *  - A contact becomes a customer when it has sales history, a vendor when it
 *    has purchase history, and both when it has both. A contact with neither
 *    falls back to `defaultRole`.
 *  - CSV values always win; CVR enrichment only fills fields the CSV left
 *    empty (same rule as `customer create --from-cvr`).
 *  - Re-import is idempotent: a contact that already exists (matched on the
 *    (vat_or_cvr, name) natural key) is skipped, never duplicated.
 *  - The plain import is deterministic and offline. CVR enrichment is opt-in
 *    and degrades gracefully — a failed lookup leaves the CSV row intact.
 */

import type { Database } from "bun:sqlite";
import type { CvrLookupOptions } from "../cvr";
import {
  createCustomer,
  createVendor,
  customerInputFromCvr,
  vendorInputFromCvr,
  findCustomerByKey,
  findVendorByKey,
  type CreateCustomerInput,
  type CreateVendorInput,
} from "../master-data";

export type ContactRole = "customer" | "vendor";

/** One normalised contact parsed from a Dinero Kontakter.csv row. */
export type DineroContact = {
  name: string;
  /** One-line postal address composed from street + postcode + city. */
  address: string | null;
  /** Normalised tax id: DK######## for Danish CVR, raw EU VAT otherwise. */
  vatOrCvr: string | null;
  countryCode: string | null;
  email: string | null;
  phone: string | null;
  website: string | null;
  eanNumber: string | null;
  attentionPerson: string | null;
  paymentTermsDays: number | null;
  totalSales: number;
  totalPurchases: number;
  contactType: string | null;
  /** Bare 8-digit CVR when the contact is a Danish company — else null. */
  danishCvr: string | null;
};

export type ParseContactsResult =
  | { ok: true; contacts: DineroContact[]; errors: string[] }
  | { ok: false; contacts: []; errors: string[] };

export type ContactImportSummary = {
  parsed: number;
  customersCreated: number;
  vendorsCreated: number;
  /** Contact roles skipped because the record already existed. */
  skipped: number;
  /** Contacts whose fields were enriched from a successful CVR lookup. */
  enriched: number;
  /** Contacts where CVR enrichment was attempted but failed. */
  enrichmentFailures: number;
};

export type ContactImportResult = {
  ok: boolean;
  summary: ContactImportSummary;
  errors: string[];
};

export type ImportContactsOptions = CvrLookupOptions & {
  /** When true, enrich Danish-CVR contacts from the CVR register. */
  enrichCvr?: boolean;
  /** Role for contacts with neither sales nor purchase history. Default vendor. */
  defaultRole?: ContactRole;
};

// ---------------------------------------------------------------------------
// CSV parsing
// ---------------------------------------------------------------------------

/** Split one CSV line into fields — semicolon-delimited, RFC4180 quoting. */
function splitCsvLine(line: string): string[] {
  const fields: string[] = [];
  let field = "";
  let inQuotes = false;
  for (let i = 0; i < line.length; i += 1) {
    const ch = line[i]!;
    if (inQuotes) {
      if (ch === '"') {
        if (line[i + 1] === '"') {
          field += '"';
          i += 1;
        } else {
          inQuotes = false;
        }
      } else {
        field += ch;
      }
    } else if (ch === '"') {
      inQuotes = true;
    } else if (ch === ";") {
      fields.push(field);
      field = "";
    } else {
      field += ch;
    }
  }
  fields.push(field);
  return fields;
}

/** Maps a Dinero CSV header label to the field it populates. */
const HEADER_FIELDS: Record<string, string> = {
  "kontaktnavn": "name",
  "adresse": "address",
  "postnummer": "postalCode",
  "by": "city",
  "landekode": "countryCode",
  "cvr-nummer": "cvr",
  "ean-nummer": "ean",
  "telefon": "phone",
  "e-mail": "email",
  "att. person": "attentionPerson",
  "hjemmeside": "website",
  "betalingsfrist i dage": "paymentTermsDays",
  "total salg": "totalSales",
  "total køb": "totalPurchases",
  "kontakttype": "contactType",
};

function cellOrNull(value: string | undefined): string | null {
  const trimmed = (value ?? "").trim();
  return trimmed.length > 0 ? trimmed : null;
}

/** Compose a one-line address from the street, postcode and city columns. */
function composeAddress(street: string | null, postalCode: string | null, city: string | null): string | null {
  // Dinero leaves a trailing ", " on some street values — drop it.
  const cleanStreet = street ? street.replace(/[,\s]+$/, "") : null;
  const cityLine = [postalCode, city].filter(Boolean).join(" ");
  const full = [cleanStreet, cityLine].filter((part) => part && part.length > 0).join(", ");
  return full.length > 0 ? full : null;
}

/**
 * Normalise the Dinero "CVR-nummer" cell, which holds a Danish CVR for DK
 * contacts and an EU VAT number for foreign ones. Returns the value to store
 * in `vat_or_cvr` plus the bare 8-digit CVR when the contact is CVR-lookup-able.
 */
function normalizeTaxId(
  raw: string | null,
  countryCode: string | null,
): { vatOrCvr: string | null; danishCvr: string | null } {
  if (!raw) return { vatOrCvr: null, danishCvr: null };
  const compact = raw.replace(/\s+/g, "").toUpperCase();
  if (compact.length === 0) return { vatOrCvr: null, danishCvr: null };
  const isDanish = (countryCode ?? "").toUpperCase() === "DK";
  if (isDanish && /^(DK)?\d{8}$/.test(compact)) {
    const digits = compact.replace(/^DK/, "");
    return { vatOrCvr: `DK${digits}`, danishCvr: digits };
  }
  return { vatOrCvr: compact, danishCvr: null };
}

function parseNumber(value: string | null): number {
  if (!value) return 0;
  const n = Number(value.replace(/\s/g, ""));
  return Number.isFinite(n) ? n : 0;
}

/** Parse a Dinero Kontakter.csv export into normalised contacts. */
export function parseDineroContactsCsv(raw: string): ParseContactsResult {
  if (typeof raw !== "string" || raw.trim().length === 0) {
    return { ok: false, contacts: [], errors: ["kontakt-CSV'en er tom"] };
  }

  // Strip a UTF-8 BOM, then split into non-empty lines.
  const lines = raw.replace(/^﻿/, "").split(/\r?\n/).filter((line) => line.trim().length > 0);
  if (lines.length === 0) {
    return { ok: false, contacts: [], errors: ["kontakt-CSV'en har ingen rækker"] };
  }

  const headerCells = splitCsvLine(lines[0]!).map((cell) => cell.trim().toLowerCase());
  const columnOf: Record<string, number> = {};
  headerCells.forEach((label, index) => {
    const field = HEADER_FIELDS[label];
    if (field) columnOf[field] = index;
  });
  if (columnOf.name === undefined) {
    return {
      ok: false,
      contacts: [],
      errors: ["kontakt-CSV'en mangler kolonnen 'Kontaktnavn' — er det en Dinero kontakt-eksport?"],
    };
  }

  const contacts: DineroContact[] = [];
  const errors: string[] = [];
  for (let i = 1; i < lines.length; i += 1) {
    const cells = splitCsvLine(lines[i]!);
    const get = (field: string): string | null =>
      columnOf[field] === undefined ? null : cellOrNull(cells[columnOf[field]!]);

    const name = get("name");
    if (!name) {
      errors.push(`linje ${i + 1}: kontakt uden navn — sprunget over`);
      continue;
    }

    const countryCode = get("countryCode");
    const { vatOrCvr, danishCvr } = normalizeTaxId(get("cvr"), countryCode);
    const paymentTermsRaw = get("paymentTermsDays");
    const paymentTermsDays =
      paymentTermsRaw && /^\d+$/.test(paymentTermsRaw) ? Number(paymentTermsRaw) : null;

    contacts.push({
      name,
      address: composeAddress(get("address"), get("postalCode"), get("city")),
      vatOrCvr,
      countryCode,
      email: get("email"),
      phone: get("phone"),
      website: get("website"),
      eanNumber: get("ean"),
      attentionPerson: get("attentionPerson"),
      paymentTermsDays,
      totalSales: parseNumber(get("totalSales")),
      totalPurchases: parseNumber(get("totalPurchases")),
      contactType: get("contactType"),
      danishCvr,
    });
  }

  return { ok: true, contacts, errors };
}

// ---------------------------------------------------------------------------
// Import
// ---------------------------------------------------------------------------

/** Decide which master-data table(s) a contact belongs in. */
export function classifyContactRoles(
  contact: DineroContact,
  defaultRole: ContactRole,
): ContactRole[] {
  const roles: ContactRole[] = [];
  if (contact.totalSales !== 0) roles.push("customer");
  if (contact.totalPurchases !== 0) roles.push("vendor");
  if (roles.length === 0) roles.push(defaultRole);
  return roles;
}

function baseCustomerInput(contact: DineroContact): CreateCustomerInput {
  return {
    name: contact.name,
    address: contact.address ?? undefined,
    vatOrCvr: contact.vatOrCvr ?? undefined,
    email: contact.email ?? undefined,
    phone: contact.phone ?? undefined,
    website: contact.website ?? undefined,
    eanNumber: contact.eanNumber ?? undefined,
    paymentTermsDays: contact.paymentTermsDays ?? undefined,
    notes: contact.attentionPerson ? `Att.: ${contact.attentionPerson}` : undefined,
  };
}

function baseVendorInput(contact: DineroContact): CreateVendorInput {
  return {
    name: contact.name,
    address: contact.address ?? undefined,
    vatOrCvr: contact.vatOrCvr ?? undefined,
    email: contact.email ?? undefined,
    phone: contact.phone ?? undefined,
    website: contact.website ?? undefined,
    notes: contact.attentionPerson ? `Att.: ${contact.attentionPerson}` : undefined,
  };
}

const EMPTY_SUMMARY: ContactImportSummary = {
  parsed: 0,
  customersCreated: 0,
  vendorsCreated: 0,
  skipped: 0,
  enriched: 0,
  enrichmentFailures: 0,
};

/**
 * Import a Dinero Kontakter.csv export into the company's master data.
 * Idempotent: a contact already present (by vat_or_cvr + name) is skipped.
 */
export async function importDineroContacts(
  db: Database,
  csvRaw: string,
  options: ImportContactsOptions = {},
): Promise<ContactImportResult> {
  const parsed = parseDineroContactsCsv(csvRaw);
  if (!parsed.ok) {
    return { ok: false, summary: { ...EMPTY_SUMMARY }, errors: parsed.errors };
  }

  const defaultRole: ContactRole = options.defaultRole ?? "vendor";
  const summary: ContactImportSummary = { ...EMPTY_SUMMARY, parsed: parsed.contacts.length };
  const errors: string[] = [...parsed.errors];

  for (const contact of parsed.contacts) {
    let contactEnriched = false;
    for (const role of classifyContactRoles(contact, defaultRole)) {
      if (role === "customer") {
        let input = baseCustomerInput(contact);
        // Existence is checked before enrichment so a re-import never spends a
        // CVR lookup on a contact it will only skip. The (vat_or_cvr, name) key
        // is unaffected by enrichment, so the base input is a safe probe.
        if (findCustomerByKey(db, input.vatOrCvr ?? null, input.name)) {
          summary.skipped += 1;
          continue;
        }
        if (options.enrichCvr && contact.danishCvr) {
          const enriched = await customerInputFromCvr(db, contact.danishCvr, input, options);
          if (enriched.ok) {
            input = enriched.input;
            contactEnriched = true;
          } else {
            summary.enrichmentFailures += 1;
            errors.push(`CVR-berigelse af '${contact.name}' fejlede: ${enriched.errors[0] ?? "ukendt fejl"}`);
          }
        }
        const created = createCustomer(db, input);
        if (created.ok) summary.customersCreated += 1;
        else errors.push(`Kunde '${contact.name}' kunne ikke oprettes: ${created.errors.join(", ")}`);
      } else {
        let input = baseVendorInput(contact);
        if (findVendorByKey(db, input.vatOrCvr ?? null, input.name)) {
          summary.skipped += 1;
          continue;
        }
        if (options.enrichCvr && contact.danishCvr) {
          const enriched = await vendorInputFromCvr(db, contact.danishCvr, input, options);
          if (enriched.ok) {
            input = enriched.input;
            contactEnriched = true;
          } else {
            summary.enrichmentFailures += 1;
            errors.push(`CVR-berigelse af '${contact.name}' fejlede: ${enriched.errors[0] ?? "ukendt fejl"}`);
          }
        }
        const created = createVendor(db, input);
        if (created.ok) summary.vendorsCreated += 1;
        else errors.push(`Leverandør '${contact.name}' kunne ikke oprettes: ${created.errors.join(", ")}`);
      }
    }
    if (contactEnriched) summary.enriched += 1;
  }

  return { ok: true, summary, errors };
}
