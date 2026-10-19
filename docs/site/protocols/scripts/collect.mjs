// Reads every protocol document under the repository's protocols/ directory (the files named
// <major>.yaml; a protocol directory holds other files too) and writes them,
// parsed but otherwise unchanged, to src/protocols.json. The protocol files are the source of
// truth: the viewer shows what they say and nothing else, and a rebuild picks up every change.
import { readdirSync, readFileSync, writeFileSync, statSync } from "node:fs";
import { join, relative, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { parse } from "yaml";

const here = dirname(fileURLToPath(import.meta.url));
const repo = join(here, "..", "..", "..", "..");
const root = join(repo, "protocols");

const files = [];
for (const dir of readdirSync(root).sort()) {
  const path = join(root, dir);
  if (!statSync(path).isDirectory()) continue;
  for (const file of readdirSync(path).sort()) {
    if (/^\d+\.yaml$/.test(file)) files.push(join(path, file));
  }
}

const protocols = files.map((file) => {
  const text = readFileSync(file, "utf8");
  const doc = parse(text);
  if (doc?.format !== "protocol/1") {
    throw new Error(`${relative(repo, file)}: format is ${doc?.format}, not protocol/1`);
  }
  return { source: relative(repo, file), doc };
});

writeFileSync(join(here, "..", "src", "protocols.json"), JSON.stringify(protocols, null, 2) + "\n");
console.log(`collected ${protocols.length} protocol(s): ${protocols.map((p) => p.source).join(", ")}`);
