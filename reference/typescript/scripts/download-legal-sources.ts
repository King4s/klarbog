import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { downloadLegalSources } from "../src/legal-sources";

const root = new URL("..", import.meta.url).pathname;
const outDir = join(root, "sources", "downloaded");

const result = await downloadLegalSources({ rootDir: root });
writeFileSync(join(outDir, "index.json"), JSON.stringify(result.index, null, 2) + "\n");
writeFileSync(join(outDir, "README.md"), result.readme);

for (const source of result.index) {
  console.log(`ready ${source.id} (${source.bytes} bytes)`);
}

if (result.errors.length > 0) {
  console.error(result.errors.join("\n"));
  process.exitCode = 1;
}
