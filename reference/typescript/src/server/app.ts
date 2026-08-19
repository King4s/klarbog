// Bun.serve wiring for the cockpit backend (#170).
//
// This is the only file that touches `Bun.serve`. All request logic lives in
// `router.ts` (a pure `(Request, config) => Promise<Response>`), so the server
// is trivially testable without binding a socket.

import type { ServerConfig } from "./config";
import { handleRequest } from "./router";

/** The concrete `Bun.serve` return type, without needing its generic param. */
type BunServer = ReturnType<typeof Bun.serve>;

export type CockpitServer = {
  server: BunServer;
  config: ServerConfig;
  /** Resolved `http://host:port` base URL. */
  url: string;
  stop: () => void;
};

/**
 * Starts the cockpit backend on the configured bind address.
 *
 * Binds `config.host` (default 127.0.0.1 — localhost-only) so Phase 1 is not
 * reachable off-box without an explicit config change.
 */
export function startCockpitServer(config: ServerConfig): CockpitServer {
  const server = Bun.serve({
    hostname: config.host,
    port: config.port,
    fetch(request) {
      return handleRequest(request, config);
    },
  });
  const host = config.host.includes(":") ? `[${config.host}]` : config.host;
  return {
    server,
    config,
    url: `http://${host}:${server.port}`,
    stop: () => server.stop(true),
  };
}
