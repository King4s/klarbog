/**
 * CLI for bilagsmail intake (#122).
 *
 *  - `mail-intake ingest` — accepts a local `.eml` file or maildrop
 *    directory, forwards attachments into the document-ingest pipeline,
 *    and routes no-attachment / ambiguous messages to the exception queue.
 *
 * IMAP / hosted-mailbox sync is deliberately not implemented here — this
 * is the first deterministic local-transport slice only.
 */

import { readFileSync } from "node:fs";
import { companyPaths } from "../core/paths";
import { openDb, migrate } from "../core/db";
import {
  ingestMailDrop,
  type IngestMailDropOptions,
  type MailIntakeMetadataInput,
} from "../core/mail-intake";
import type { CommandDispatch } from "../cli-dispatch";

export function register(dispatch: CommandDispatch): void {
  dispatch.on("mail-intake", "ingest", (ctx) => {
    const source = ctx.arg("--source");
    if (!source) {
      console.error("Missing required --source <eml-file-or-maildrop-dir>");
      process.exit(2);
    }

    const root = ctx.companyRoot();
    const db = openDb(companyPaths(root).db);
    migrate(db);

    const options: IngestMailDropOptions = {
      ingestOptions: { forceDuplicateLogicalIdentity: ctx.hasFlag("--force") },
    };

    const metadataFile = ctx.arg("--metadata");
    if (metadataFile) {
      let raw: unknown;
      try {
        raw = JSON.parse(readFileSync(metadataFile, "utf8"));
      } catch (error) {
        ctx.emitResult({
          ok: false,
          errors: [`could not read --metadata ${metadataFile}: ${error instanceof Error ? error.message : String(error)}`],
        });
        db.close();
        process.exit(1);
      }
      // A bare metadata object applies to every attachment; an object
      // keyed on message-id applies per-message.
      if (raw && typeof raw === "object" && isMetadataByMessageId(raw)) {
        options.metadataPerMessage = raw as Record<string, MailIntakeMetadataInput>;
      } else {
        options.metadata = raw as MailIntakeMetadataInput;
      }
    }

    const result = ingestMailDrop(db, root, source, options);
    ctx.emitResult(result as unknown as Record<string, unknown>);
    db.close();
  });
}

/**
 * Heuristic: a `--metadata` payload is treated as per-message when every
 * top-level key looks like an RFC-822 Message-ID (`<...>`).
 */
function isMetadataByMessageId(value: object): boolean {
  const keys = Object.keys(value);
  return keys.length > 0 && keys.every((key) => key.startsWith("<") && key.endsWith(">"));
}
