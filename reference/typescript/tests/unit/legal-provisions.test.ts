// Tests: src/core/legal-provisions.ts
import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import {
  compareParagrafIds,
  extractProvisions,
  findProvision,
  loadAllProvisions,
  parseProvisionRef,
} from "../../src/core/legal-provisions";

const downloadedDir = join(import.meta.dir, "../../sources/downloaded");

function readSource(sourceId: string): string {
  return readFileSync(join(downloadedDir, `${sourceId}.xml`), "utf8");
}

describe("extractProvisions structural anchors", () => {
  test("renteloven exposes paragraf/stk anchors and the 3a sub-paragraf", () => {
    const sourceId = "DK-RENTELOVEN-2014-459";
    const provisions = extractProvisions(readSource(sourceId), sourceId);

    const paragrafIds = [...new Set(provisions.map((p) => p.path[0]))];
    expect(paragrafIds.length).toBe(18);
    expect(paragrafIds).toContain("3a");

    const par1Stk = provisions.filter((p) => p.path[0] === "1" && p.path.length === 2);
    expect(par1Stk.map((p) => p.path[1])).toEqual(["1", "2", "3", "4", "5"]);

    const par3Stk = provisions.filter((p) => p.path[0] === "3" && p.path.length === 2);
    expect(par3Stk.length).toBe(5);

    expect(findProvision(provisions, "§ 1, stk. 1")?.path).toEqual(["1", "1"]);
  });

  test("opbevaringsbekendtgørelse splits stk and nummer provisions with correct text", () => {
    const sourceId = "DK-BILAG-OPBEVARING-2023-1383";
    const provisions = extractProvisions(readSource(sourceId), sourceId);

    const par1Stk = provisions.filter((p) => p.path[0] === "1" && p.path.length === 2);
    expect(par1Stk.length).toBe(3);

    const par1Stk1Nr = provisions.filter(
      (p) => p.path[0] === "1" && p.path[1] === "1" && p.path.length === 3,
    );
    expect(par1Stk1Nr.length).toBe(6);
    expect(par1Stk1Nr[0].text).toBe("Udstedelsesdato.");
    expect(par1Stk1Nr[0].ref).toBe("§ 1, stk. 1, nr. 1");
    expect(par1Stk1Nr[3].text).toBe(
      "Afsender og modtager, herunder navn, adresse, samt CVR-nummer eller SE-nummer.",
    );

    const stk1 = findProvision(provisions, "§ 1, stk. 1");
    expect(stk1?.text.startsWith("Virksomheder omfattet af")).toBe(true);
    expect(stk1?.text.includes("Udstedelsesdato")).toBe(false);

    const par2Stk1 = findProvision(provisions, "§ 2, stk. 1");
    expect(par2Stk1?.text).toBe("Bekendtgørelsen træder i kraft den 1. juli 2024.");
    expect(par2Stk1?.kind).toBe("commencement");
  });

  test("amendment document yields amendment-kinded provisions", () => {
    const sourceId = "DK-BILAG-OPBEVARING-AMEND-2025-302";
    const provisions = extractProvisions(readSource(sourceId), sourceId);

    const par1 = findProvision(provisions, "§ 1, stk. 1");
    expect(par1?.kind).toBe("amendment");

    const par2 = findProvision(provisions, "§ 2, stk. 1");
    expect(par2?.kind).toBe("commencement");
  });
});

describe("commencement classification", () => {
  test("commencement is told apart from prose that merely mentions it", () => {
    const sourceId = "DK-BOGFORINGSLOVEN-2022-700";
    const provisions = extractProvisions(readSource(sourceId), sourceId);

    // A genuine commencement provision, even one led by a paragraf enumeration
    // ("§§ 1-14, 18 … træder i kraft …") rather than the phrase itself.
    expect(findProvision(provisions, "§ 34, stk. 1")?.kind).toBe("commencement");

    // A delegation clause that mentions commencement timing inside real prose
    // is not a commencement provision.
    expect(findProvision(provisions, "§ 34, stk. 2")?.kind).toBe("operative");

    // A genuine commencement provision in a bekendtgørelse is still detected.
    const bilag = "DK-BILAG-OPBEVARING-2023-1383";
    const bilagProvisions = extractProvisions(readSource(bilag), bilag);
    expect(findProvision(bilagProvisions, "§ 2, stk. 1")?.kind).toBe("commencement");
  });
});

describe("determinism", () => {
  test("extractProvisions is byte-stable across repeated runs", () => {
    const sourceId = "DK-BILAG-OPBEVARING-2023-1383";
    const xml = readSource(sourceId);
    const first = extractProvisions(xml, sourceId);
    const second = extractProvisions(xml, sourceId);
    expect(second).toEqual(first);
    expect(second.map((p) => p.textHash)).toEqual(first.map((p) => p.textHash));
  });

  test("textHash is a sha256 of the normalized text", () => {
    const sourceId = "DK-BILAG-OPBEVARING-2023-1383";
    const provisions = extractProvisions(readSource(sourceId), sourceId);
    for (const provision of provisions) {
      expect(provision.textHash).toMatch(/^sha256:[0-9a-f]{64}$/);
    }
  });
});

describe("ref parsing and round-trip", () => {
  test("parseProvisionRef decodes paragraf, stk, and nr levels", () => {
    expect(parseProvisionRef("§ 3, stk. 2, nr. 4")).toEqual(["3", "2", "4"]);
    expect(parseProvisionRef("§ 1, stk. 1")).toEqual(["1", "1"]);
    expect(parseProvisionRef("§ 3a")).toEqual(["3a"]);
    expect(parseProvisionRef("§ 2, stk. 1, litra b")).toEqual(["2", "1", "b"]);
    expect(parseProvisionRef("not a ref")).toBeUndefined();
  });

  test("parseProvisionRef rejects malformed refs instead of resolving them wrong", () => {
    // "§ 3 a" must not silently resolve to § 3; a dangling "og 7" must not be
    // silently dropped — both surface as a loud closure error instead.
    expect(parseProvisionRef("§ 3 a")).toBeUndefined();
    expect(parseProvisionRef("§ 58, stk. 1, nr. 6 og 7")).toBeUndefined();
    expect(parseProvisionRef("§ 3, stk. 2 extra")).toBeUndefined();
    expect(parseProvisionRef("§ 3, stk.")).toBeUndefined();
    expect(parseProvisionRef("")).toBeUndefined();
  });

  test("findProvision round-trips every extracted provision", () => {
    const sourceId = "DK-RENTELOVEN-2014-459";
    const provisions = extractProvisions(readSource(sourceId), sourceId);
    expect(provisions.length).toBeGreaterThan(0);
    for (const provision of provisions) {
      expect(findProvision(provisions, provision.ref)).toBe(provision);
    }
  });
});

describe("compareParagrafIds", () => {
  test("orders letter-suffixed paragraf identifiers numerically then by suffix", () => {
    // This ordering is what scope-range membership ("§ 1-§ 9b" must contain
    // "9" and "9b" but not "9c") depends on.
    expect(compareParagrafIds("3", "3a")).toBeLessThan(0);
    expect(compareParagrafIds("3a", "3")).toBeGreaterThan(0);
    expect(compareParagrafIds("3a", "4")).toBeLessThan(0);
    expect(compareParagrafIds("9", "9b")).toBeLessThan(0);
    expect(compareParagrafIds("9a", "9b")).toBeLessThan(0);
    expect(compareParagrafIds("9b", "9c")).toBeLessThan(0);
    expect(compareParagrafIds("9b", "10")).toBeLessThan(0);
    expect(compareParagrafIds("9b", "9b")).toBe(0);
  });
});

describe("loadAllProvisions", () => {
  test("loads every downloaded source with a non-trivial provision count", () => {
    const all = loadAllProvisions();
    const expectedIds = [
      "DK-BILAG-OPBEVARING-2023-1383",
      "DK-BILAG-OPBEVARING-AMEND-2025-302",
      "DK-BOGFORINGSLOVEN-2022-700",
      "DK-DIGITAL-BOGFORING-NONREGISTERED-2024-205",
      "DK-DIGITAL-STANDARD-BOGFORING-2023-97",
      "DK-MOMSBEKENDTGORELSEN-2023-1435",
      "DK-MOMSLOVEN-2024-209",
      "DK-RENTELOVEN-2014-459",
      "DK-UDENRETLIGE-INDDRIVELSESOMKOSTNINGER-AMEND-2013-105",
    ];
    expect([...all.keys()].sort()).toEqual(expectedIds);
    for (const id of expectedIds) {
      expect(all.get(id)!.length).toBeGreaterThan(0);
    }
    expect(all.get("DK-MOMSLOVEN-2024-209")!.length).toBeGreaterThan(100);
  });
});
