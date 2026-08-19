// Tests: src/cli.ts, src/cli-args.ts (CLI input boundary errors)
import { describe, expect, test } from "bun:test";

describe("CLI input boundary errors", () => {
  test("fails fast when a required flag value is missing", async () => {
    const proc = Bun.spawn([
      "bun", "run", "src/cli.ts",
      "bank", "import",
      "--company",
      "--file", "examples/bank-transactions.csv",
    ], {
      cwd: process.cwd(),
      stdout: "pipe",
      stderr: "pipe",
    });

    const stdout = await new Response(proc.stdout).text();
    const stderr = await new Response(proc.stderr).text();
    const exitCode = await proc.exited;

    expect(exitCode).toBe(2);
    expect(stdout).toBe("");
    expect(stderr).toContain("Flag --company requires a value");
  });

  test("rejects a --company path containing parent-directory traversal", async () => {
    const proc = Bun.spawn([
      "bun", "run", "src/cli.ts",
      "journal", "post",
      "--company", "/tmp/../tmp/evil",
      "--input", "examples/journal-entry.expense.json",
    ], {
      cwd: process.cwd(),
      stdout: "pipe",
      stderr: "pipe",
      env: { ...process.env, RENTEMESTER_COMPANY: "" },
    });

    const stderr = await new Response(proc.stderr).text();
    const exitCode = await proc.exited;

    expect(exitCode).toBe(2);
    expect(stderr.toLowerCase()).toContain("company");
  });

  test("fails with a clear error when a company-bound command has no --company", async () => {
    const proc = Bun.spawn([
      "bun", "run", "src/cli.ts",
      "journal", "post",
      "--input", "examples/journal-entry.expense.json",
    ], {
      cwd: process.cwd(),
      stdout: "pipe",
      stderr: "pipe",
      env: { ...process.env, RENTEMESTER_COMPANY: "" },
    });

    const stderr = await new Response(proc.stderr).text();
    const exitCode = await proc.exited;

    expect(exitCode).toBe(2);
    expect(stderr).toContain("--company");
    expect(stderr).not.toContain("/company");
  });

  test("invoice validate still works without --company", async () => {
    const proc = Bun.spawn([
      "bun", "run", "src/cli.ts",
      "invoice", "validate",
      "--input", "examples/full-invoice.dk.json",
    ], {
      cwd: process.cwd(),
      stdout: "pipe",
      stderr: "pipe",
      env: { ...process.env, RENTEMESTER_COMPANY: "" },
    });

    const stderr = await new Response(proc.stderr).text();
    const exitCode = await proc.exited;

    // validate needs no company; it must not fail on a missing --company.
    expect(stderr).not.toContain("--company is required");
    expect([0, 1]).toContain(exitCode);
  });

  test("prints a useful error for unknown commands", async () => {
    const proc = Bun.spawn(["bun", "run", "src/cli.ts", "nonsense", "command"], {
      cwd: process.cwd(),
      stdout: "pipe",
      stderr: "pipe",
    });

    const stdout = await new Response(proc.stdout).text();
    const stderr = await new Response(proc.stderr).text();
    const exitCode = await proc.exited;

    expect(exitCode).toBe(2);
    expect(stderr).toContain("Unknown command: nonsense command");
    // The global usage lists commands grouped by read vs write, with the
    // actor contract among the global flags. (#231)
    expect(stdout).toContain("Læsekommandoer");
    expect(stdout).toContain("Skrivekommandoer");
    expect(stdout).toContain("--actor");
  });
});
