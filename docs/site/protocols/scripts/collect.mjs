// Reads every protocol document `takt protocols` lists, the engineering protocols' built-ins,
// and writes them, parsed but otherwise unchanged, to src/protocols.json. They are read from
// the source of the `b10x-canon-engineering` that takt's library depends on (`cargo metadata
// --locked`), where the registry itself embeds them from: `protocols/<name>/<major>.yaml` two
// directories above the crate's manifest. The documents are the source of truth: the viewer shows what they say and
// nothing else, and a rebuild after the pin moves picks up every change.
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { parse } from "yaml";

const here = dirname(fileURLToPath(import.meta.url));
const repo = join(here, "..", "..", "..", "..");
const PROTOCOLS = [
  "software-change/1",
  "story-delivery/1",
  "eval-gated-change/1",
  "incident-response/1",
  "ml-experiment/1",
  "support-triage/1",
  "self-improvement/1",
  "loop-improvement/1",
];

const metadata = JSON.parse(
  execFileSync("cargo", ["metadata", "--locked", "--format-version", "1"], {
    cwd: repo,
    encoding: "utf8",
    maxBuffer: 256 * 1024 * 1024,
  }),
);
const takt = metadata.packages.find((p) => p.name === "b10x-takt" && metadata.workspace_members.includes(p.id));
if (!takt) throw new Error("cargo metadata: no workspace package b10x-takt");
const node = metadata.resolve.nodes.find((n) => n.id === takt.id);
const dep = node?.deps.find((d) => d.name === "canon_engineering");
const crate = metadata.packages.find((p) => p.id === dep?.pkg);
if (!crate) throw new Error("cargo metadata: b10x-takt depends on no b10x-canon-engineering");
const root = join(dirname(crate.manifest_path), "..", "..", "protocols");

const protocols = PROTOCOLS.map((id) => {
  const file = join(root, `${id}.yaml`);
  const doc = parse(readFileSync(file, "utf8"));
  const source = `b10x-canon-engineering ${crate.version}, protocols/${id}.yaml`;
  if (doc?.format !== "protocol/1") {
    throw new Error(`${source}: format is ${doc?.format}, not protocol/1`);
  }
  return { source, doc };
});

writeFileSync(join(here, "..", "src", "protocols.json"), JSON.stringify(protocols, null, 2) + "\n");
console.log(`collected ${protocols.length} protocol(s): ${protocols.map((p) => p.source).join(", ")}`);
