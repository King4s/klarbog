import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { basename, join } from "node:path";

export type LegalSource = {
  id: string;
  title: string;
  authority: string;
  category: string;
  url: string;
  xmlUrl?: string;
  notes?: string;
};

export type DownloadedLegalSource = LegalSource & {
  downloadedAt: string;
  localPath: string;
  bytes: number;
  sha256: string;
};

export type DownloadLegalSourcesOptions = {
  rootDir: string;
  fetchImpl?: typeof fetch;
  now?: () => string;
  timeoutMs?: number;
};

export type DownloadLegalSourcesResult = {
  index: DownloadedLegalSource[];
  readme: string;
  errors: string[];
};

function sha256(data: string | ArrayBuffer) {
  return createHash("sha256").update(typeof data === "string" ? data : Buffer.from(data)).digest("hex");
}

function safeName(id: string, ext: string) {
  return `${id.replace(/[^A-Z0-9._-]/gi, "_")}.${ext}`;
}

function detectExtension(url: string, contentType: string | null) {
  const type = contentType?.split(";")[0].trim().toLowerCase() ?? "";
  if (type.includes("xml")) return "xml";
  if (type.includes("pdf")) return "pdf";
  if (type.includes("html")) return "html";
  if (url.endsWith("/xml")) return "xml";
  if (basename(new URL(url).pathname).toLowerCase().endsWith(".pdf")) return "pdf";
  return "html";
}

function validateBody(ext: string, body: string) {
  const trimmed = body.trimStart();
  if (ext === "xml") return trimmed.startsWith("<?xml") || trimmed.startsWith("<");
  if (ext === "html") return trimmed.startsWith("<!DOCTYPE html") || trimmed.startsWith("<html") || trimmed.startsWith("<HTML");
  if (ext === "pdf") return body.startsWith("%PDF-");
  return body.length > 0;
}

export async function downloadLegalSources(options: DownloadLegalSourcesOptions): Promise<DownloadLegalSourcesResult> {
  const fetchImpl = options.fetchImpl ?? fetch;
  const now = options.now ?? (() => new Date().toISOString());
  const timeoutMs = options.timeoutMs ?? 30_000;
  const manifestPath = join(options.rootDir, "sources", "legal-sources.json");
  const outDir = join(options.rootDir, "sources", "downloaded");
  const indexPath = join(outDir, "index.json");
  mkdirSync(outDir, { recursive: true });

  const sources = JSON.parse(readFileSync(manifestPath, "utf8")) as LegalSource[];
  const existingIndex = existsSync(indexPath)
    ? JSON.parse(readFileSync(indexPath, "utf8")) as DownloadedLegalSource[]
    : [];
  const existingById = new Map(existingIndex.map((entry) => [entry.id, entry]));
  const index: DownloadedLegalSource[] = [];
  const errors: string[] = [];

  for (const source of sources) {
    const url = source.xmlUrl ?? source.url;
    const existing = existingById.get(source.id);
    try {
      const res = await fetchImpl(url, {
        signal: AbortSignal.timeout(timeoutMs),
        headers: { "user-agent": "Rentemester legal source downloader/0.0.1" },
      });
      if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
      const body = await res.text();
      const ext = detectExtension(url, res.headers.get("content-type"));
      if (!validateBody(ext, body)) throw new Error(`unexpected ${ext} payload`);

      const filename = safeName(source.id, ext);
      const file = join(outDir, filename);
      const relativePath = `sources/downloaded/${filename}`;
      const hash = sha256(body);
      const downloadedAt = existing?.sha256 === hash ? existing.downloadedAt : now();

      writeFileSync(file, body);
      index.push({ ...source, downloadedAt, localPath: relativePath, bytes: Buffer.byteLength(body), sha256: hash });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      errors.push(`${source.id}: ${message}`);
      if (existing && existsSync(join(options.rootDir, existing.localPath))) {
        index.push(existing);
      }
    }
  }

  const readme = [
    "# Downloaded legal sources",
    "",
    `Source count: ${index.length}`,
    "",
    ...index.map((source) => `- ${source.id}: ${source.title} (${source.sha256})`),
    "",
  ].join("\n");

  return { index, readme, errors };
}
