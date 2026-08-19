// Tests: src/cli-args.ts
import { describe, expect, test } from "bun:test";
import { parseCliArgs } from "../../src/cli-args";

describe("CLI arg parser", () => {
  test("keeps repeated raw values attached to their own flags", () => {
    const parsed = parseCliArgs([
      "bun", "src/cli.ts",
      "invoice", "issue",
      "--company", "/tmp/smoke",
      "--input", "/tmp/smoke",
    ]);

    expect(parsed.errors).toEqual([]);
    expect(parsed.positionals).toEqual(["invoice", "issue"]);
    expect(parsed.flags.get("--company")).toBe("/tmp/smoke");
    expect(parsed.flags.get("--input")).toBe("/tmp/smoke");
  });

  test("reports missing flag values instead of silently consuming the next flag", () => {
    const parsed = parseCliArgs([
      "bun", "src/cli.ts",
      "bank", "import",
      "--company",
      "--file", "examples/bank-transactions.csv",
    ]);

    expect(parsed.errors).toContain("Flag --company requires a value");
    expect(parsed.flags.get("--file")).toBe("examples/bank-transactions.csv");
  });

  test("accepts --flag=value form so values may begin with dashes", () => {
    const parsed = parseCliArgs([
      "bun", "src/cli.ts",
      "journal", "post",
      "--text=--weird-text",
      "--actor=user:alice",
    ]);

    expect(parsed.errors).toEqual([]);
    expect(parsed.flags.get("--text")).toBe("--weird-text");
    expect(parsed.flags.get("--actor")).toBe("user:alice");
  });

  test("accepts --flag=value with an empty value", () => {
    const parsed = parseCliArgs([
      "bun", "src/cli.ts",
      "journal", "post",
      "--text=",
    ]);

    expect(parsed.errors).toEqual([]);
    expect(parsed.flags.get("--text")).toBe("");
  });

  test("treats tokens after -- as positionals even if they start with dashes", () => {
    const parsed = parseCliArgs([
      "bun", "src/cli.ts",
      "journal", "post",
      "--company", "/tmp/smoke",
      "--",
      "--not-a-flag",
      "plain",
    ]);

    expect(parsed.errors).toEqual([]);
    expect(parsed.flags.get("--company")).toBe("/tmp/smoke");
    expect(parsed.positionals).toEqual(["journal", "post", "--not-a-flag", "plain"]);
  });

  test("a non-boolean flag value may begin with -- after the terminator is not needed via =form", () => {
    const parsed = parseCliArgs([
      "bun", "src/cli.ts",
      "invoice", "validate",
      "--input=--leading-dash-path",
    ]);

    expect(parsed.errors).toEqual([]);
    expect(parsed.flags.get("--input")).toBe("--leading-dash-path");
  });
});
