/**
 * CLI for bilagsmail IMAP intake (#181).
 *
 *  - `imap-intake poll` — connects to a hosted IMAP mailbox, fetches new
 *    messages, and forwards each into the EXISTING #122 bilagsmail pipeline
 *    (attachment extraction, message-id + attachment-hash dedup, exception
 *    routing). Dedup is rerun-stable: re-polling an already-ingested message
 *    creates no duplicate documents.
 *
 * IMAP credentials are read from `--imap-*` flags and `RENTEMESTER_IMAP_*`
 * environment variables — never from the ledger DB. Provider OAuth and SMTP
 * sending are out of scope.
 */

import { readFileSync } from "node:fs";
import { openCommandDb } from "../cli-dispatch";
import { migrate } from "../core/db";
import {
  createImapClient,
  pollImapMailbox,
  resolveImapConfig,
  type ImapConfig,
  type PollImapOptions,
} from "../core/imap-intake";
import type { MailIntakeMetadataInput } from "../core/mail-intake";
import type { CommandDispatch } from "../cli-dispatch";

export function register(dispatch: CommandDispatch): void {
  dispatch.on("imap-intake", "poll", async (ctx) => {
    // Connection settings: explicit flags override RENTEMESTER_IMAP_* env.
    // The password is env-only by design so it never lands in a shell history.
    const partial: Partial<ImapConfig> = {};
    const host = ctx.arg("--imap-host");
    if (host) partial.host = host;
    const username = ctx.arg("--imap-username");
    if (username) partial.username = username;
    const mailbox = ctx.arg("--imap-mailbox");
    if (mailbox) partial.mailbox = mailbox;
    const portArg = ctx.parseOptionalNumber("--imap-port");
    if (!portArg.ok) {
      ctx.emitResult({ ok: false, errors: [portArg.error] });
      process.exit(1);
    }
    if (portArg.value !== undefined) partial.port = portArg.value;

    const config = resolveImapConfig(partial);
    if (!config.ok) {
      ctx.emitResult({ ok: false, errors: config.errors });
      process.exit(1);
    }

    const options: PollImapOptions = {
      ingestOptions: { forceDuplicateLogicalIdentity: ctx.hasFlag("--force") },
    };
    const sinceUid = ctx.parseOptionalNumber("--since-uid");
    if (!sinceUid.ok) {
      ctx.emitResult({ ok: false, errors: [sinceUid.error] });
      process.exit(1);
    }
    if (sinceUid.value !== undefined) options.sinceUid = sinceUid.value;

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
        process.exit(1);
      }
      // A bare metadata object applies to every attachment; an object keyed
      // on message-id applies per-message (same heuristic as mail-intake).
      if (raw && typeof raw === "object" && isMetadataByMessageId(raw)) {
        options.metadataPerMessage = raw as Record<string, MailIntakeMetadataInput>;
      } else {
        options.metadata = raw as MailIntakeMetadataInput;
      }
    }

    const root = ctx.companyRoot();
    const db = openCommandDb(ctx);
    migrate(db);
    try {
      const client = createImapClient(config.config);
      const result = await pollImapMailbox(db, root, client, options);
      ctx.emitResult(result as unknown as Record<string, unknown>);
      if (!result.ok) process.exit(1);
    } finally {
      db.close();
    }
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
