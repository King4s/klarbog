// Workspace company discovery for the cockpit backend (#256).
//
// A workspace is a directory holding one company subdirectory per `slug`
// (`<workspace>/<slug>/`), indexed by a `workspace.json` manifest. The manifest
// is the cockpit's source of truth for "which companies exist".
//
// But a company directory can land in the workspace WITHOUT being in the
// manifest: an owner who set up a company with the CLI's `--company <path>`
// flow (or copied a finished company directory in) has a fully populated
// `<workspace>/<slug>/data/ledger.sqlite` that the manifest never recorded.
// Before #256 the cockpit then showed "0 virksomheder" and — worse — letting
// the owner "create" that company minted a new, empty ledger over a blank
// slug, hiding the real data behind a blank screen.
//
// `discoverWorkspaceCompanies` closes that gap: before every company-list /
// portfolio read it scans the workspace for present-but-unlisted company
// directories and registers them into the manifest (the same adoption
// `registerCompanyDirIntoWorkspace` performs for `rentemester init`). The
// cockpit then shows the real company with its real data. It is deliberately
// forgiving — a non-company directory, an unreadable entry or a slug clash is
// skipped, never thrown — so a stray directory can never break the cockpit.

import { existsSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { companyPaths } from "../core/paths";
import {
  isValidSlug,
  listWorkspaceCompanies,
  registerCompanyDirIntoWorkspace,
  workspaceExists,
  type WorkspaceCompanyEntry,
} from "../core/workspace";

/**
 * Scans `workspaceRoot` for company directories that hold a ledger but are not
 * in the `workspace.json` manifest, and registers each one. Returns the
 * manifest's company entries AFTER the adoption — so a caller that lists the
 * result sees every real company, listed or freshly discovered.
 *
 * A workspace directory entry counts as a company when:
 *  - its name is a valid slug (a single safe path segment), and
 *  - it contains a `data/ledger.sqlite` ledger file.
 *
 * Anything else (the `workspace.json` file itself, a `.git` dir, a directory
 * with no ledger) is skipped. Adoption uses `registerCompanyDirIntoWorkspace`,
 * which reads the company name from the ledger and never throws.
 *
 * When the workspace root does not exist yet, this is a no-op returning an
 * empty list — there is nothing to discover.
 */
export function discoverWorkspaceCompanies(
  workspaceRoot: string,
): WorkspaceCompanyEntry[] {
  let dirEntries: string[];
  try {
    dirEntries = readdirSync(workspaceRoot, { withFileTypes: true })
      .filter((d) => d.isDirectory())
      .map((d) => d.name);
  } catch {
    // The workspace root does not exist (or is unreadable) — nothing to scan.
    return workspaceExists(workspaceRoot)
      ? listWorkspaceCompanies(workspaceRoot)
      : [];
  }

  const listed = new Set(
    workspaceExists(workspaceRoot)
      ? listWorkspaceCompanies(workspaceRoot).map((c) => c.slug)
      : [],
  );

  for (const name of dirEntries) {
    if (listed.has(name)) continue;
    if (!isValidSlug(name)) continue;
    const companyRoot = join(workspaceRoot, name);
    // Only a directory that actually carries a ledger is a company directory —
    // a bare folder is not adopted (and `registerCompanyDirIntoWorkspace`
    // would otherwise register it with no real data).
    if (!existsSync(companyPaths(companyRoot).db)) continue;
    // Forgiving by contract: a clash or any other condition is a silent no-op.
    registerCompanyDirIntoWorkspace(workspaceRoot, companyRoot);
  }

  return workspaceExists(workspaceRoot)
    ? listWorkspaceCompanies(workspaceRoot)
    : [];
}
