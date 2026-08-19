// Cockpit backend configuration (#170).
//
// The bind address is config-driven so the local-only Phase 1 default can be
// changed without touching code. Everything is resolved from the environment
// here, in one place, so the rest of the server is pure.

import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { resolveConfiguredWorkspaceRoot } from "../core/workspace";

export const DEFAULT_APP_HOST = "127.0.0.1";
export const DEFAULT_APP_PORT = 4319;

/**
 * Absolute path of the built cockpit SPA. The repo layout is `<root>/app/dist`
 * and this file lives at `<root>/src/server/config.ts`, so the dist directory
 * is two levels up plus `app/dist`. Overridable via `RENTEMESTER_APP_STATIC`.
 */
function resolveStaticRoot(env: Record<string, string | undefined>): string {
  const override = env.RENTEMESTER_APP_STATIC?.trim();
  if (override) return override;
  const here = dirname(fileURLToPath(import.meta.url));
  return join(here, "..", "..", "app", "dist");
}

export type ServerConfig = {
  /** Interface to bind. Defaults to 127.0.0.1 (localhost-only). */
  host: string;
  /** TCP port to listen on. */
  port: number;
  /** Workspace root the API serves. */
  workspaceRoot: string;
  /**
   * When true, the auth middleware enforces a shared-secret check via the
   * `RENTEMESTER_APP_TOKEN` env var. Phase 1 leaves this off (localhost-trusted)
   * — it exists so the seam can be exercised by tests and flipped on later.
   */
  authRequired: boolean;
  /** Optional shared secret consulted only when `authRequired` is true. */
  authToken: string | null;
  /**
   * Absolute path to the built cockpit SPA (`app/dist`). When the directory
   * exists, the server serves it for every non-`/api` route. Resolved here so
   * the rest of the server stays pure. Optional: when absent the server is a
   * pure JSON API (the shape used by API-only tests).
   */
  staticRoot?: string;
};

function parsePort(raw: string | undefined, fallback: number): number {
  if (raw === undefined || raw.trim().length === 0) return fallback;
  const port = Number(raw);
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    throw new Error(
      `RENTEMESTER_APP_PORT must be an integer between 1 and 65535, got: ${raw}`,
    );
  }
  return port;
}

export type ResolveServerConfigOptions = {
  /** Explicit overrides (e.g. from CLI flags) take precedence over env. */
  host?: string;
  port?: number;
  workspaceRoot?: string;
  /** Read environment from here instead of `process.env` (testability). */
  env?: Record<string, string | undefined>;
};

/**
 * Resolves the server configuration from CLI overrides + environment.
 *
 * A workspace root is required: the API is workspace-scoped, so it must know
 * which workspace to serve. Throws a clear error when none is configured.
 */
export function resolveServerConfig(
  options: ResolveServerConfigOptions = {},
): ServerConfig {
  const env = options.env ?? process.env;

  const host =
    options.host?.trim() ||
    (env.RENTEMESTER_APP_HOST?.trim() ?? "") ||
    DEFAULT_APP_HOST;

  const port =
    options.port ?? parsePort(env.RENTEMESTER_APP_PORT, DEFAULT_APP_PORT);

  let workspaceRoot = options.workspaceRoot?.trim() || null;
  if (!workspaceRoot) {
    // resolveConfiguredWorkspaceRoot reads RENTEMESTER_WORKSPACE from
    // process.env; honour an injected env map for testability.
    const fromEnv = env.RENTEMESTER_WORKSPACE;
    if (typeof fromEnv === "string" && fromEnv.trim().length > 0) {
      const prev = process.env.RENTEMESTER_WORKSPACE;
      process.env.RENTEMESTER_WORKSPACE = fromEnv;
      try {
        workspaceRoot = resolveConfiguredWorkspaceRoot();
      } finally {
        if (prev === undefined) delete process.env.RENTEMESTER_WORKSPACE;
        else process.env.RENTEMESTER_WORKSPACE = prev;
      }
    } else {
      workspaceRoot = resolveConfiguredWorkspaceRoot();
    }
  }
  if (!workspaceRoot) {
    throw new Error(
      "no workspace configured: set RENTEMESTER_WORKSPACE or pass --workspace <dir>",
    );
  }

  const authToken = env.RENTEMESTER_APP_TOKEN?.trim() || null;
  const authRequired =
    (env.RENTEMESTER_APP_AUTH?.trim().toLowerCase() ?? "") === "required";

  return {
    host,
    port,
    workspaceRoot,
    authRequired,
    authToken,
    staticRoot: resolveStaticRoot(env),
  };
}
