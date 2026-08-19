// Tests: src/core/dashboard.ts (dashboard design tokens)
import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const DESIGN_PATH = join(process.cwd(), "DESIGN.md");
const HEADING_ORDER = [
  "Overview",
  "Colors",
  "Typography",
  "Layout",
  "Elevation & Depth",
  "Shapes",
  "Components",
  "Do's and Don'ts",
];

type FrontMatter = Record<string, any>;

describe("DESIGN.md token contract", () => {
  test("front matter has required sections and valid hex colors", () => {
    const { frontMatter } = parseDesign(readFileSync(DESIGN_PATH, "utf8"));
    expect(frontMatter.name).toBeString();
    expect(frontMatter.colors).toBeObject();
    expect(frontMatter.typography).toBeObject();
    expect(frontMatter.spacing).toBeObject();
    expect(frontMatter.rounded).toBeObject();
    expect(frontMatter.components).toBeObject();

    for (const [path, value] of flatten(frontMatter.colors)) {
      if (typeof value !== "string") continue;
      expect(value, `invalid hex at ${path}`).toMatch(/^#[0-9A-F]{6}$/i);
    }
  });

  test("component token references are not dangling", () => {
    const { frontMatter } = parseDesign(readFileSync(DESIGN_PATH, "utf8"));
    for (const [path, value] of flatten(frontMatter.components, "components")) {
      if (typeof value !== "string") continue;
      const match = /^\{(.+)\}$/.exec(value.trim());
      if (!match) continue;
      const resolved = resolveToken(frontMatter, match[1]!);
      expect(resolved, `dangling reference: ${path} -> ${value}`).not.toBeUndefined();
    }
  });

  test("core contrast pairs meet WCAG thresholds", () => {
    const { frontMatter } = parseDesign(readFileSync(DESIGN_PATH, "utf8"));
    const colors = frontMatter.colors;
    expect(contrast(colors.ink, colors.paper), "ink on paper must be AAA").toBeGreaterThanOrEqual(7);
    expect(contrast(colors.inkMuted, colors.paper), "ink-muted on paper must be AA").toBeGreaterThanOrEqual(4.5);
    expect(contrast(colors.onAccent, colors.accent), "on-accent on accent must be AA").toBeGreaterThanOrEqual(4.5);
    expect(contrast(colors.danger, colors.paper), "danger on paper must be AA").toBeGreaterThanOrEqual(4.5);

    for (const [name, value] of Object.entries(colors)) {
      if (!name.endsWith("Soft") || typeof value !== "string") continue;
      expect(contrast(colors.ink, value), `ink on ${name} must be AA`).toBeGreaterThanOrEqual(4.5);
    }
  });

  test("markdown sections follow canonical order", () => {
    const { body } = parseDesign(readFileSync(DESIGN_PATH, "utf8"));
    const headings = Array.from(body.matchAll(/^##\s+(.+)$/gm)).map((match) => match[1]!.trim());
    expect(headings).toEqual(HEADING_ORDER);
  });
});

function parseDesign(markdown: string) {
  const match = /^---\n([\s\S]*?)\n---\n([\s\S]*)$/m.exec(markdown);
  if (!match) throw new Error("DESIGN.md missing YAML front matter");
  return {
    frontMatter: parseYaml(match[1]!),
    body: match[2]!,
  };
}

function parseYaml(yaml: string) {
  const root: FrontMatter = {};
  const stack: Array<{ indent: number; value: FrontMatter }> = [{ indent: -1, value: root }];

  for (const rawLine of yaml.split(/\r?\n/)) {
    if (!rawLine.trim()) continue;
    const indent = rawLine.match(/^\s*/)?.[0].length ?? 0;
    const line = rawLine.trim();
    const keyMatch = /^([^:]+):(.*)$/.exec(line);
    if (!keyMatch) throw new Error(`Unsupported YAML line: ${rawLine}`);
    const key = toCamel(keyMatch[1]!.trim());
    const rest = keyMatch[2]!.trim();

    while (stack.length > 1 && indent <= stack[stack.length - 1]!.indent) stack.pop();
    const parent = stack[stack.length - 1]!.value;

    if (!rest) {
      const child: FrontMatter = {};
      parent[key] = child;
      stack.push({ indent, value: child });
      continue;
    }

    parent[key] = parseScalar(rest);
  }

  return root;
}

function toCamel(input: string) {
  return input.replace(/-([a-z])/g, (_, c) => c.toUpperCase());
}

function parseScalar(value: string) {
  const trimmed = value.trim();
  if ((trimmed.startsWith('"') && trimmed.endsWith('"')) || (trimmed.startsWith("'") && trimmed.endsWith("'"))) {
    return trimmed.slice(1, -1);
  }
  if (/^-?\d+(\.\d+)?$/.test(trimmed)) return Number(trimmed);
  return trimmed;
}

function flatten(value: unknown, prefix = "colors"): Array<[string, unknown]> {
  if (!value || typeof value !== "object" || Array.isArray(value)) return [[prefix, value]];
  const entries: Array<[string, unknown]> = [];
  for (const [key, child] of Object.entries(value as Record<string, unknown>)) {
    entries.push(...flatten(child, `${prefix}.${key}`));
  }
  return entries;
}

function resolveToken(root: FrontMatter, path: string) {
  return path.split(".").reduce<any>((current, part) => current?.[toCamel(part)], root);
}

function contrast(foreground: string, background: string) {
  const fg = relativeLuminance(foreground);
  const bg = relativeLuminance(background);
  const lighter = Math.max(fg, bg);
  const darker = Math.min(fg, bg);
  return (lighter + 0.05) / (darker + 0.05);
}

function relativeLuminance(hex: string) {
  const channels = hex.replace("#", "").match(/.{2}/g)?.map((chunk) => parseInt(chunk, 16) / 255) ?? [];
  const [r, g, b] = channels.map((value) => value <= 0.03928 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4);
  return 0.2126 * r! + 0.7152 * g! + 0.0722 * b!;
}
